//! What the Map view needs, in one payload: the places (with their points, when they have one), the people in
//! the chosen scope with their generation, one row per event a person lived at a place, and the parent to child
//! links. The view filters and draws from it without more calls.

use crate::dto::NameOrder;
use crate::names::{display_name, preferred_name, surname_of};
use crate::person_detail::event_kind_id;
use crate::session::Session;
use gt_core::date::{Date, DateValue, GenDate};
use gt_core::model::{ParticipantRef, Role};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use ts_rs::TS;
use uuid::Uuid;

/// The most generations a scope walks (a guard; a family tree this deep is a loop).
const MAX_GENERATIONS: u32 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MapScopeKind {
    All,
    Ancestors,
    Descendants,
    Person,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MapScope {
    pub kind: MapScopeKind,
    /// The person for every kind but `all`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub person: Option<Uuid>,
    /// How many generations to walk for `ancestors` and `descendants`; absent means all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub generations: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MapPlaceDto {
    pub id: Uuid,
    /// "Village, District, Oblast".
    pub label: String,
    /// The leaf name alone.
    pub name: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MapPersonDto {
    pub id: Uuid,
    pub name: String,
    pub surname: String,
    /// 0 for the person the scope is about, 1 for parents or children, and so on.
    pub generation: i32,
    pub living: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MapEventDto {
    pub id: Uuid,
    pub kind: String,
    /// The first year the date states; absent for an undated event.
    pub year: Option<i32>,
    pub place: Uuid,
    pub person: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MapLinkDto {
    pub parent: Uuid,
    pub child: Uuid,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MapDataDto {
    /// Every place an event of the scope happened at, with or without a point.
    pub places: Vec<MapPlaceDto>,
    pub people: Vec<MapPersonDto>,
    pub events: Vec<MapEventDto>,
    pub links: Vec<MapLinkDto>,
    /// Events of the scope with no place at all.
    pub events_without_place: u32,
    /// The old maps laid on the Map, the lowest first (every one of the tree, whatever the scope).
    pub layers: Vec<crate::map_layers::MapLayerDto>,
}

/// The first year a date states (negative for BCE).
pub fn year_of(d: &GenDate) -> Option<i32> {
    use DateValue::*;
    let y = |x: &Date| if x.bce { -x.year } else { x.year };
    match &d.value {
        Exact(x) | Interpreted(x) | About(x) | Calculated(x) | Estimated(x) | Before(x) | To(x)
        | After(x) | From(x) => Some(y(x)),
        Between(a, _) | FromTo(a, _) => Some(y(a)),
        PhraseOnly | Unparsed => None,
    }
}

impl Session {
    /// People of the scope with their generation, nearest first. Private people are never in it.
    fn map_scope_people(&self, scope: &MapScope) -> Vec<(Uuid, i32)> {
        let visible = |id: &Uuid| {
            self.tree
                .persons
                .get(id)
                .is_some_and(|p| !p.common.private && !p.sibling_marker)
        };
        let limit = scope
            .generations
            .unwrap_or(MAX_GENERATIONS)
            .min(MAX_GENERATIONS);
        match scope.kind {
            MapScopeKind::All => self
                .tree
                .persons
                .keys()
                .filter(|id| visible(id))
                .map(|id| (*id, 0))
                .collect(),
            MapScopeKind::Person => scope
                .person
                .filter(visible)
                .map(|id| vec![(id, 0)])
                .unwrap_or_default(),
            MapScopeKind::Ancestors | MapScopeKind::Descendants => {
                let Some(root) = scope.person.filter(visible) else {
                    return Vec::new();
                };
                let up = scope.kind == MapScopeKind::Ancestors;
                let mut seen: HashSet<Uuid> = HashSet::from([root]);
                let mut out = vec![(root, 0)];
                let mut queue = VecDeque::from([(root, 0u32)]);
                while let Some((p, gen_)) = queue.pop_front() {
                    if gen_ >= limit {
                        continue;
                    }
                    let next: Vec<Uuid> = if up {
                        self.parents_of(p)
                    } else {
                        self.children_of(p)
                    };
                    for n in next {
                        if visible(&n) && seen.insert(n) {
                            out.push((n, (gen_ + 1) as i32));
                            queue.push_back((n, gen_ + 1));
                        }
                    }
                }
                out
            }
        }
    }

    fn parents_of(&self, person: Uuid) -> Vec<Uuid> {
        let mut out = Vec::new();
        for (fid, _) in self.links.child_in.get(&person).into_iter().flatten() {
            if let Some(f) = self.tree.families.get(fid) {
                out.extend([f.partner1, f.partner2].into_iter().flatten());
            }
        }
        out
    }

    fn children_of(&self, person: Uuid) -> Vec<Uuid> {
        let mut out = Vec::new();
        for fid in self.links.partner_in.get(&person).into_iter().flatten() {
            if let Some(f) = self.tree.families.get(fid) {
                out.extend(f.children.iter().map(|c| c.person));
            }
        }
        out
    }

    /// The map payload for `scope`. Records marked private are left out; living people are in it (with a flag) as in
    /// every other view of the owner's tree, and the picture export leaves them out.
    pub fn map_data(&self, scope: &MapScope) -> MapDataDto {
        let scoped = self.map_scope_people(scope);
        let generation: HashMap<Uuid, i32> = scoped.iter().copied().collect();
        let mut people = Vec::with_capacity(scoped.len());
        for (id, gen_) in &scoped {
            let Some(p) = self.tree.persons.get(id) else {
                continue;
            };
            people.push(MapPersonDto {
                id: *id,
                name: display_name(p, NameOrder::GivenFirst),
                surname: preferred_name(p).map(surname_of).unwrap_or_default(),
                generation: *gen_,
                living: self.living.get(id).copied().unwrap_or(true),
            });
        }
        let mut events: Vec<MapEventDto> = Vec::new();
        let mut used: HashSet<Uuid> = HashSet::new();
        let mut without_place = 0u32;
        let mut seen: HashSet<(Uuid, Uuid)> = HashSet::new();
        for (id, _) in &scoped {
            let mut event_ids: Vec<Uuid> =
                self.links.events_of.get(id).cloned().unwrap_or_default();
            for fid in self.links.partner_in.get(id).into_iter().flatten() {
                event_ids.extend(
                    self.links
                        .events_of_family
                        .get(fid)
                        .into_iter()
                        .flatten()
                        .copied(),
                );
            }
            for eid in event_ids {
                let Some(e) = self.tree.events.get(&eid) else {
                    continue;
                };
                if e.negative || e.common.private || !seen.insert((eid, *id)) {
                    continue;
                }
                let mine = e.participants.iter().any(|p| match p.target {
                    ParticipantRef::Person(x) => x == *id && p.role == Role::Principal,
                    ParticipantRef::Family(_) => true,
                });
                if !mine {
                    continue;
                }
                let place = e
                    .place
                    .filter(|pl| self.tree.places.get(pl).is_some_and(|p| !p.common.private));
                let Some(place) = place else {
                    without_place += 1;
                    continue;
                };
                used.insert(place);
                events.push(MapEventDto {
                    id: eid,
                    kind: event_kind_id(&e.kind),
                    year: e.date.as_ref().and_then(year_of),
                    place,
                    person: *id,
                });
            }
        }
        let mut places: Vec<MapPlaceDto> = used
            .into_iter()
            .filter_map(|id| {
                let p = self.tree.places.get(&id)?;
                let opt = self.place_option(id)?;
                Some(MapPlaceDto {
                    id,
                    label: opt.full,
                    name: opt.name,
                    lat: p.lat,
                    lon: p.lon,
                })
            })
            .collect();
        places.sort_by(|a, b| a.label.cmp(&b.label).then(a.id.cmp(&b.id)));
        events.sort_by(|a, b| {
            a.year
                .cmp(&b.year)
                .then(a.id.cmp(&b.id))
                .then(a.person.cmp(&b.person))
        });
        let mut links = Vec::new();
        let mut linked: HashSet<(Uuid, Uuid)> = HashSet::new();
        for (child, _) in &scoped {
            for parent in self.parents_of(*child) {
                if generation.contains_key(&parent) && linked.insert((parent, *child)) {
                    links.push(MapLinkDto {
                        parent,
                        child: *child,
                    });
                }
            }
        }
        MapDataDto {
            places,
            people,
            events,
            links,
            events_without_place: without_place,
            layers: self.map_layers(),
        }
    }
}

/// One downloaded country of the offline gazetteer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GazetteerCountryDto {
    pub country: String,
    pub rows: u32,
    #[ts(type = "number")]
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GazetteerStatusDto {
    pub countries: Vec<GazetteerCountryDto>,
    /// The attribution GeoNames asks for.
    pub attribution: String,
}

/// A GeoNames place that might be the one a tree place means.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GazetteerCandidateDto {
    pub geoname_id: u32,
    pub country: String,
    pub name: String,
    /// A Ukrainian (or other Cyrillic) name; empty when none is known.
    pub local: String,
    pub lat: f64,
    pub lon: f64,
    /// GeoNames feature code: PPL, PPLA, PPLH (historical), ADM2, ...
    pub code: String,
    pub population: u32,
    /// "District, Region" as the dump names them.
    pub region: String,
    pub region_local: String,
    pub score: u32,
    /// The other parts of the typed place agree with the candidate's regions.
    pub region_match: bool,
}

impl From<gt_gazetteer::Candidate> for GazetteerCandidateDto {
    fn from(c: gt_gazetteer::Candidate) -> Self {
        let join = |a: &str, b: &str| {
            [a, b]
                .iter()
                .filter(|s| !s.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join(", ")
        };
        GazetteerCandidateDto {
            geoname_id: c.geoname_id,
            country: c.country,
            region: join(&c.admin2, &c.admin1),
            region_local: join(
                if c.admin2_local.is_empty() {
                    &c.admin2
                } else {
                    &c.admin2_local
                },
                if c.admin1_local.is_empty() {
                    &c.admin1
                } else {
                    &c.admin1_local
                },
            ),
            name: c.name,
            local: c.local,
            lat: c.lat,
            lon: c.lon,
            code: c.code,
            population: c.population,
            score: c.score,
            region_match: c.region_match,
        }
    }
}

/// Progress of a gazetteer download, emitted by the app while it works.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GazetteerProgressDto {
    pub country: String,
    /// `download` or `index`.
    pub phase: String,
    #[ts(type = "number")]
    pub done: u64,
    #[ts(type = "number")]
    pub total: u64,
}

impl Session {
    /// The names of a place, leaf first ("Village", "District", "Oblast"): what the gazetteer matches.
    pub fn place_parts(&self, id: Uuid) -> Vec<String> {
        let mut out = Vec::new();
        let mut next = Some(id);
        for _ in 0..16 {
            let Some(p) = next.and_then(|i| self.tree.places.get(&i)) else {
                break;
            };
            let name = p
                .names
                .first()
                .map(|n| n.name.clone())
                .or_else(|| p.raw.clone())
                .unwrap_or_default();
            if !name.trim().is_empty() {
                out.push(name);
            }
            next = p.parent;
        }
        out
    }
}
