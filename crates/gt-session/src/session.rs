//! An open project: the tree in memory, the person index and the settings.

use crate::dto::{
    FamilyViewDto, HitKindDto, LocationKind, LocationWarningDto, Page, PersonDetail, PersonQuery,
    PersonRef, PersonRow, ProjectInfo, SearchHitDto, Settings, SortKey,
};
use crate::error::{ApiError, ErrorCode};
use crate::index::{Links, PersonIndex};
use crate::query::MAX_SEARCH;
use crate::settings::{self, SettingsPatch};
use gt_core::living::{LivingRule, compute_living, compute_living_ignoring_overrides};
use gt_core::model::{LivingOverride, ParticipantRef};
use gt_core::tree::Tree;
use gt_store::{ChangeSet, HitKind, LocationWarning, Project, Record, RecordKind};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use uuid::Uuid;

/// Change sets with more records than this refresh by full `rebuild`.
const FULL_REBUILD_ABOVE: usize = 2_000;

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
    /// Undo and redo stacks and the last citation (see `undo.rs`).
    pub(crate) edit: crate::undo::EditState,
    /// Image sizes read so far (see `media.rs`).
    pub(crate) media_dims: crate::media::DimsCache,
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
            edit: Default::default(),
            media_dims: Default::default(),
        };
        s.rebuild();
        Ok(s)
    }

    /// Releases the project (closes the database).
    pub fn close(self) {
        drop(self);
    }

    /// The living flags from the tree: `(effective with manual overrides, the rule alone)`.
    pub(crate) fn living_maps(&self) -> (HashMap<Uuid, bool>, HashMap<Uuid, bool>) {
        let (today, _) = (self.clock.0)();
        let rule = LivingRule {
            max_age_years: self.settings.living_max_age,
        };
        let computed = compute_living_ignoring_overrides(&self.tree, &rule, today);
        let mut effective = computed.clone();
        for (id, p) in &self.tree.persons {
            match p.living_override {
                Some(LivingOverride::Living) => effective.insert(*id, true),
                Some(LivingOverride::Deceased) => effective.insert(*id, false),
                None => None,
            };
        }
        (effective, computed)
    }

    /// Recomputes links, living flags and the index from the tree (open, import, settings that
    /// change every row). Edits use `refresh_after` instead.
    pub fn rebuild(&mut self) {
        self.charts.bump_revision();
        self.links = Links::build(&self.tree);
        (self.living, self.living_computed) = self.living_maps();
        self.index = PersonIndex::build(
            &self.tree,
            &self.links,
            &self.living,
            self.settings.name_order,
        );
        self.place_index = crate::lookup::PlaceIndex::build(&self.tree);
    }

    /// Brings every derived structure in line after `cs` was applied to the tree, touching only
    /// what `cs` can have changed: the rows of the persons it names (or whose events or living
    /// flag it changes), the links of the records it names, and the place use counts. Equal to
    /// `rebuild` (see `verify_derived_state`). `effective` and `computed` are the living maps of
    /// the new tree and `living_changed` the flags that differ from `self.living`.
    pub(crate) fn refresh_after(
        &mut self,
        cs: &ChangeSet,
        effective: HashMap<Uuid, bool>,
        computed: HashMap<Uuid, bool>,
        living_changed: &[(Uuid, bool)],
    ) {
        if cs.changes.len() > FULL_REBUILD_ABOVE {
            self.rebuild();
            return;
        }
        self.charts.bump_revision();
        let mut dirty: HashSet<Uuid> = HashSet::new();
        let mut structural = false;
        let mut places_changed = false;
        for ch in &cs.changes {
            match ch.kind {
                RecordKind::Person => {
                    dirty.insert(ch.id);
                    structural |= ch.before.is_none() != ch.after.is_none();
                }
                RecordKind::Place => places_changed = true,
                _ => {}
            }
            for rec in [&ch.before, &ch.after].into_iter().flatten() {
                if let Record::Event(e) = rec {
                    for p in &e.participants {
                        if let ParticipantRef::Person(id) = p.target {
                            dirty.insert(id);
                        }
                    }
                }
            }
        }
        dirty.extend(living_changed.iter().map(|(id, _)| *id));
        self.living = effective;
        self.living_computed = computed;
        self.links.apply(&self.tree, cs);
        self.index.refresh(
            &self.tree,
            &self.links,
            &self.living,
            self.settings.name_order,
            &dirty,
            structural,
        );
        if places_changed {
            self.place_index = crate::lookup::PlaceIndex::build(&self.tree);
        } else {
            for ch in &cs.changes {
                let place_of = |r: &Option<Record>| match r {
                    Some(Record::Event(e)) => e.place,
                    _ => None,
                };
                if ch.kind == RecordKind::Event {
                    self.place_index.move_event(
                        &self.tree,
                        place_of(&ch.before),
                        place_of(&ch.after),
                    );
                }
            }
        }
    }

    /// Checks that the incrementally refreshed state equals a fresh `rebuild`; the first
    /// difference is returned. For tests.
    #[doc(hidden)]
    pub fn verify_derived_state(&self) -> Result<(), String> {
        let (living, computed) = self.living_maps();
        let rule = LivingRule {
            max_age_years: self.settings.living_max_age,
        };
        let (today, _) = (self.clock.0)();
        if living != compute_living(&self.tree, &rule, today) {
            return Err("effective living differs from compute_living".into());
        }
        if living != self.living {
            return Err("living map differs".into());
        }
        if computed != self.living_computed {
            return Err("living_computed map differs".into());
        }
        let links = Links::build(&self.tree);
        if links != self.links {
            return Err("links differ".into());
        }
        let index = PersonIndex::build(&self.tree, &links, &living, self.settings.name_order);
        if index.summaries.len() != self.index.summaries.len() {
            return Err("index row count differs".into());
        }
        for (i, (a, b)) in index
            .summaries
            .iter()
            .zip(&self.index.summaries)
            .enumerate()
        {
            if a != b {
                return Err(format!("index row {i} differs: {a:?} vs {b:?}"));
            }
            if self.index.slot(a.id) != index.slot(a.id) {
                return Err(format!("slot of row {i} differs"));
            }
        }
        for sort in [
            SortKey::Surname,
            SortKey::Given,
            SortKey::Birth,
            SortKey::Changed,
        ] {
            for desc in [false, true] {
                if index.order(sort, desc) != self.index.order(sort, desc) {
                    return Err(format!("sort order {sort:?} desc={desc} differs"));
                }
            }
        }
        if crate::lookup::PlaceIndex::build(&self.tree) != self.place_index {
            return Err("place index differs".into());
        }
        Ok(())
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
        let mut living_rule_changed = false;
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
            living_rule_changed = v != self.settings.living_max_age;
            rebuild |= living_rule_changed;
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
        if living_rule_changed {
            // The stored flag follows the new rule for the whole tree.
            self.project.set_living_cached_all(&self.living)?;
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

    pub fn get_family(&self, id: Uuid) -> Result<crate::dto::FamilyDto, ApiError> {
        crate::family_view::family(self, id)
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
