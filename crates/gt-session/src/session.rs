//! An open project: the tree in memory, the person index and the settings.

use crate::dto::{
    FamilyViewDto, HitKindDto, LocationKind, LocationWarningDto, Page, PersonDetail, PersonQuery,
    PersonRef, PersonRow, ProjectInfo, SearchHitDto, Settings,
};
use crate::error::{ApiError, ErrorCode};
use crate::index::{Links, PersonIndex};
use crate::query::MAX_SEARCH;
use crate::settings::{self, SettingsPatch};
use gt_core::living::{LivingRule, compute_living};
use gt_core::model::LivingOverride;
use gt_core::tree::Tree;
use gt_store::{HitKind, LocationWarning, Project};
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;

/// `(today as a Julian day number, now in Unix milliseconds)`. Injected so tests pin the date.
pub struct SessionClock(pub Box<dyn Fn() -> (i32, i64) + Send + Sync>);

impl SessionClock {
    pub fn system() -> SessionClock {
        SessionClock(Box::new(|| {
            let ms = gt_core::ids::now_ms();
            let days = ms.div_euclid(86_400_000);
            // JDN of 1970-01-01 is 2440588.
            (i32::try_from(days + 2_440_588).unwrap_or(i32::MAX), ms)
        }))
    }

    pub fn fixed(today_jdn: i32, now_ms: i64) -> SessionClock {
        SessionClock(Box::new(move || (today_jdn, now_ms)))
    }
}

pub struct Session {
    pub(crate) project: Project,
    pub(crate) tree: Tree,
    pub(crate) links: Links,
    pub(crate) index: PersonIndex,
    /// Effective living flag (override applied).
    pub(crate) living: HashMap<Uuid, bool>,
    /// The core rule alone, ignoring manual overrides.
    pub(crate) living_computed: HashMap<Uuid, bool>,
    pub(crate) settings: Settings,
    pub(crate) clock: SessionClock,
    pub(crate) place_index: crate::lookup::PlaceIndex,
    pub(crate) charts: crate::chart::Charts,
}

impl Session {
    pub fn open(dir: &Path, clock: SessionClock) -> Result<Session, ApiError> {
        let project = Project::open(dir)?;
        let tree = project.load_tree()?;
        let settings = settings::load(&project)?;
        let mut s = Session {
            project,
            tree,
            links: Links::default(),
            index: PersonIndex::default(),
            living: HashMap::new(),
            living_computed: HashMap::new(),
            settings,
            clock,
            place_index: Default::default(),
            charts: crate::chart::Charts::default(),
        };
        s.rebuild();
        Ok(s)
    }

    /// Releases the project (closes the database).
    pub fn close(self) {
        drop(self);
    }

    /// Recomputes links, living flags and the index from the tree.
    pub fn rebuild(&mut self) {
        self.charts.bump_revision();
        let (today, _) = (self.clock.0)();
        let rule = LivingRule {
            max_age_years: self.settings.living_max_age,
        };
        self.links = Links::build(&self.tree);
        self.living = compute_living(&self.tree, &rule, today);
        self.living_computed = if self
            .tree
            .persons
            .values()
            .any(|p| p.living_override.is_some())
        {
            let mut plain = self.tree.clone();
            for p in plain.persons.values_mut() {
                p.living_override = None::<LivingOverride>;
            }
            compute_living(&plain, &rule, today)
        } else {
            self.living.clone()
        };
        self.index = PersonIndex::build(
            &self.tree,
            &self.links,
            &self.living,
            self.settings.name_order,
        );
        self.place_index = crate::lookup::PlaceIndex::build(&self.tree);
    }

    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Applies a patch; project values go to the `meta` table, and display or rule changes
    /// rebuild the index.
    pub fn set_settings(&mut self, patch: SettingsPatch) -> Result<&Settings, ApiError> {
        let mut rebuild = false;
        if let Some(v) = patch.day_order {
            self.settings.day_order = v;
        }
        if let Some(v) = patch.theme {
            self.settings.theme = v;
        }
        if let Some(v) = patch.backups_to_keep {
            self.settings.backups_to_keep = v;
        }
        if let Some(v) = patch.name_order {
            rebuild |= v != self.settings.name_order;
            self.settings.name_order = v;
        }
        if let Some(v) = patch.living_max_age {
            if !(1..=150).contains(&v) {
                return Err(ApiError::new(
                    ErrorCode::Invalid,
                    "Enter a maximum age between 1 and 150 years.",
                ));
            }
            self.project
                .meta_set(settings::KEY_LIVING_MAX_AGE, &v.to_string())?;
            rebuild |= v != self.settings.living_max_age;
            self.settings.living_max_age = v;
        }
        if let Some(home) = patch.home_person {
            if let Some(id) = home
                && !self.tree.persons.contains_key(&id)
            {
                return Err(ApiError::not_found("person", id));
            }
            self.project.meta_set(
                settings::KEY_HOME_PERSON,
                &home.map(|h| h.to_string()).unwrap_or_default(),
            )?;
            self.settings.home_person = home;
        }
        if rebuild {
            self.rebuild();
        }
        Ok(&self.settings)
    }

    pub fn info(&self) -> ProjectInfo {
        let dir = self.project.dir.clone();
        let name = self
            .project
            .meta_get("project_name")
            .ok()
            .flatten()
            .filter(|n| !n.trim().is_empty())
            .or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        ProjectInfo {
            dir: dir.to_string_lossy().into_owned(),
            name,
            person_count: u32::try_from(self.tree.persons.len()).unwrap_or(u32::MAX),
            home_person: self
                .settings
                .home_person
                .filter(|id| self.tree.persons.contains_key(id)),
            source_version: self.tree.header.source_version.clone(),
            location_warning: gt_store::check_location(&dir).map(|w| match w {
                LocationWarning::SyncedFolder(p) => LocationWarningDto {
                    kind: LocationKind::SyncedFolder,
                    provider: Some(p.to_string()),
                },
                LocationWarning::NetworkShare => LocationWarningDto {
                    kind: LocationKind::NetworkShare,
                    provider: None,
                },
            }),
        }
    }

    /// `limit` is clamped to 200.
    pub fn list_persons(&self, q: &PersonQuery, offset: usize, limit: usize) -> Page<PersonRow> {
        self.index.list(q, offset, limit)
    }

    pub fn person_ref(&self, id: Uuid) -> Option<PersonRef> {
        self.index.get(id).map(|s| s.person_ref())
    }

    pub fn get_person(&self, id: Uuid) -> Result<PersonDetail, ApiError> {
        crate::person_detail::person_detail(self, id)
    }

    pub fn family_view(&self, id: Uuid) -> Result<FamilyViewDto, ApiError> {
        crate::family_view::family_view(self, id)
    }

    /// Full-text search; `limit` is clamped to 50.
    pub fn search(&self, q: &str, limit: usize) -> Result<Vec<SearchHitDto>, ApiError> {
        let hits = self.project.search(q, limit.min(MAX_SEARCH))?;
        Ok(hits
            .into_iter()
            .map(|h| SearchHitDto {
                kind: match h.kind {
                    HitKind::Person => HitKindDto::Person,
                    HitKind::Place => HitKindDto::Place,
                    HitKind::Source => HitKindDto::Source,
                    HitKind::Note => HitKindDto::Note,
                },
                person: (h.kind == HitKind::Person)
                    .then(|| self.person_ref(h.id))
                    .flatten(),
                id: h.id,
                label: h.label,
                snippet: h.snippet,
            })
            .collect())
    }
}
