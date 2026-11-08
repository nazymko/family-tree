//! Wire types. Each mirrors a type in `app/src/lib/api/types.ts`; the `export_bindings` test
//! regenerates the TypeScript into `app/src/lib/api/generated/`. Field names are camelCase.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

macro_rules! wire_enum {
    ($(#[$m:meta])* $name:ident, $rename:literal, { $($variant:ident),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
        #[serde(rename_all = $rename)]
        pub enum $name { $($variant),+ }
    };
}

wire_enum!(DayOrder, "lowercase", { Dmy, Mdy });
wire_enum!(SexKind, "lowercase", { Male, Female, Intersex, Unknown, Other });
wire_enum!(Theme, "lowercase", { Light, Dark, System });
wire_enum!(Language, "lowercase", { En, Uk, System });
// Which way the tree charts grow: `reversed` puts the ancestors at the bottom (pedigree: on the left).
wire_enum!(TreeDirection, "lowercase", { Normal, Reversed });
wire_enum!(DateStatus, "lowercase", { Ok, Phrase, Unparsed });
wire_enum!(HitKindDto, "lowercase", { Person, Place, Source, Note });
wire_enum!(LivingOverrideDto, "lowercase", { Living, Deceased });
wire_enum!(NameKindDto, "lowercase", {
    Birth, Married, Aka, Religious, Nickname, Immigrant, Professional, Other
});
wire_enum!(NoteKindDto, "lowercase", {
    General, Story, Research, Transcription, Conclusion, Other
});
wire_enum!(ParentRelId, "snake_case", {
    Birth, Adopted, Step, Foster, Guardian, Sealed, Unknown, Other
});
wire_enum!(UnionKindId, "snake_case", { Married, CivilUnion, Unmarried, Unknown, Other });
wire_enum!(LocationKind, "snake_case", { SyncedFolder, NetworkShare });
wire_enum!(
    /// Name display order.
    NameOrder, "kebab-case", { GivenFirst, SurnameFirst }
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum SortKey {
    #[default]
    Surname,
    Given,
    Birth,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum LivingFilter {
    #[default]
    Any,
    Living,
    Deceased,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PersonRef {
    pub id: Uuid,
    pub name: String,
    /// "c. 1968 – 2021", "b. 1970", "" (see `dates::lifespan_text`).
    pub lifespan: String,
    pub sex: SexKind,
    pub living: bool,
    pub private: bool,
    /// The surname of the birth name, only when it differs from the surname the person is shown under
    /// (see `names::birth_surname_shown`). Absent for hidden or replaced names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub birth_surname: Option<String>,
    /// The "more brothers and sisters, unnamed" entry: it has no name or dates and the app draws
    /// its label.
    pub sibling_marker: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PersonRow {
    #[serde(flatten)]
    pub person: PersonRef,
    pub surname: String,
    pub given: String,
    pub birth: String,
    pub death: String,
    pub source_count: u32,
    /// Unix milliseconds.
    #[ts(type = "number")]
    pub changed: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PersonQuery {
    pub sort: SortKey,
    pub desc: bool,
    pub text: String,
    pub living: LivingFilter,
    pub no_source: bool,
    pub no_birth_date: bool,
    pub private_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LetterOffset {
    pub letter: String,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub total: u32,
    pub offset: u32,
    pub items: Vec<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub letters: Option<Vec<LetterOffset>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DateDto {
    pub raw: String,
    pub display: String,
    pub status: DateStatus,
    pub sort_min: Option<i32>,
    pub sort_max: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaceOption {
    pub id: Uuid,
    pub name: String,
    pub full: String,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NameDto {
    pub id: Uuid,
    pub kind: NameKindDto,
    pub kind_text: Option<String>,
    pub preferred: bool,
    pub private: bool,
    pub display: String,
    pub prefix: Option<String>,
    pub given: Option<String>,
    pub call: Option<String>,
    pub surname_prefix: Option<String>,
    pub surnames: Vec<String>,
    pub patronymic: Option<String>,
    pub suffix: Option<String>,
    pub nickname: Option<String>,
    pub lang: Option<String>,
    pub date: Option<DateDto>,
    pub citations: Vec<CitationDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantDto {
    pub person: PersonRef,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EventDto {
    pub id: Uuid,
    /// `birth`, `death`, ..., `custom:<text>`.
    pub kind: String,
    pub label: String,
    pub is_fact: bool,
    pub date: Option<DateDto>,
    pub place: Option<PlaceOption>,
    pub description: Option<String>,
    pub age: Option<String>,
    pub cause: Option<String>,
    pub negative: bool,
    pub private: bool,
    pub preferred: bool,
    pub participants: Vec<ParticipantDto>,
    pub citations: Vec<CitationDto>,
    pub notes: Vec<NoteDto>,
    pub media: Vec<MediaLinkDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceDto {
    pub id: Uuid,
    pub title: Option<String>,
    pub author: Option<String>,
    pub publication: Option<String>,
    pub abbreviation: Option<String>,
    pub text: Option<String>,
    pub citation_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceDto {
    pub original: Option<bool>,
    pub primary_info: Option<bool>,
    pub direct: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CitationDto {
    pub id: Uuid,
    pub source: Option<SourceDto>,
    pub page: Option<String>,
    pub date: Option<DateDto>,
    pub text: Option<String>,
    #[ts(type = "0 | 1 | 2 | 3 | null")]
    pub quality: Option<u8>,
    #[ts(inline)]
    pub evidence: EvidenceDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NoteDto {
    pub id: Uuid,
    pub kind: NoteKindDto,
    pub text: String,
    pub shared: bool,
    pub lang: Option<String>,
    /// Voice samples (and any other media) attached to the note.
    pub media: Vec<MediaLinkDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaDto {
    pub id: Uuid,
    pub title: Option<String>,
    pub mime: Option<String>,
    pub original_name: Option<String>,
    pub date: Option<DateDto>,
    pub description: Option<String>,
    /// The file is in the project folder.
    pub resolved: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MediaLinkDto {
    pub media: MediaDto,
    pub primary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ParentSetDto {
    pub family_id: Uuid,
    pub parents: (Option<PersonRef>, Option<PersonRef>),
    pub rel: (ParentRelId, ParentRelId),
    pub preferred: bool,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChildDto {
    pub person: PersonRef,
    pub rel: (ParentRelId, ParentRelId),
    pub birth: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PartnershipDto {
    pub family_id: Uuid,
    pub partner: Option<PersonRef>,
    /// The person is the family's second partner, so a child's `rel.1` is the link to them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub focus_second: Option<bool>,
    pub kind: UnionKindId,
    pub summary: Option<String>,
    pub events: Vec<EventDto>,
    pub children: Vec<ChildDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FamilyViewDto {
    pub focus: PersonRef,
    pub focus_summary: String,
    pub parent_sets: Vec<ParentSetDto>,
    pub partnerships: Vec<PartnershipDto>,
    pub siblings: Vec<PersonRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PersonDetail {
    #[serde(rename = "ref")]
    #[ts(rename = "ref")]
    pub person_ref: PersonRef,
    pub sex: SexKind,
    pub sex_text: Option<String>,
    pub living_override: Option<LivingOverrideDto>,
    pub living_computed: bool,
    pub private: bool,
    pub names: Vec<NameDto>,
    pub events: Vec<EventDto>,
    pub parent_sets: Vec<ParentSetDto>,
    pub partnerships: Vec<PartnershipDto>,
    pub notes: Vec<NoteDto>,
    pub media: Vec<MediaLinkDto>,
    pub citations: Vec<CitationDto>,
    #[ts(type = "number")]
    pub changed: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitDto {
    pub kind: HitKindDto,
    pub id: Uuid,
    pub label: String,
    pub snippet: Option<String>,
    pub person: Option<PersonRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LocationWarningDto {
    pub kind: LocationKind,
    pub provider: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub dir: String,
    pub name: String,
    pub person_count: u32,
    pub home_person: Option<Uuid>,
    pub source_version: Option<String>,
    pub location_warning: Option<LocationWarningDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub day_order: DayOrder,
    pub name_order: NameOrder,
    pub theme: Theme,
    /// Interface language; `system` follows the OS language.
    pub language: Language,
    pub backups_to_keep: u32,
    /// App setting: which way the tree charts and the family tree grow.
    pub tree_direction: TreeDirection,
    /// Project setting.
    pub living_max_age: u32,
    /// Project setting.
    pub home_person: Option<Uuid>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            day_order: DayOrder::Dmy,
            name_order: NameOrder::GivenFirst,
            theme: Theme::System,
            language: Language::Uk,
            backups_to_keep: 10,
            tree_direction: TreeDirection::Normal,
            living_max_age: 100,
            home_person: None,
        }
    }
}

/// Live feedback while a date is typed (`Api.parseDate`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DatePreview {
    pub raw: String,
    pub understood_as: String,
    pub range: Option<String>,
    pub status: DateStatus,
    pub warnings: Vec<String>,
}

// ---- Charts (mirrors ChartRequest .. OutlineItem in types.ts) ----

wire_enum!(RoleKind, "snake_case", {
    SelfPerson, Focus, Ancestor, Descendant, Sibling, AuntUncle, NieceNephew, Cousin, Partner,
    PartnerOfSibling, PartnerOfAuntUncle, PartnerOfDescendant, AncestorNo, DescendantNo, Generation,
    Kin, PartnerOfKin
});
// What the chart built around a person shows besides their own line: nothing, their brothers and sisters, or
// all relatives (the brothers and sisters of every ancestor too, with all their descendants).
wire_enum!(AroundKin, "lowercase", { Line, Siblings, All });
wire_enum!(OutlineGroupKind, "snake_case", { Parents, Siblings, Partners, Children, AuntsUncles });
wire_enum!(ChartKind, "lowercase", { Pedigree, Descendants, Relatives, Around, Whole });
wire_enum!(EdgeKindDto, "lowercase", { Parent, Partner, Dup, Sibling });

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChartRequest {
    pub kind: ChartKind,
    pub root: Uuid,
    pub generations: u32,
    /// Relatives chart only: show the parents' brothers and sisters (default on).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub aunts_uncles: Option<bool>,
    /// Chart built around a person only: who is shown besides the line (default `line`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub kin: Option<AroundKin>,
    /// Chart built around people only: everybody the chart is built around, `root` included. Two or more
    /// people give one chart around all of them, with the wider family `kin` asks for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub roots: Option<Vec<Uuid>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
pub struct ChartSize {
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChartMeta {
    pub chart_id: u32,
    pub kind: ChartKind,
    pub bounds: ChartSize,
    pub node_count: u32,
    pub root_node: u32,
    pub truncated: bool,
    /// Tree revision the chart was laid out at.
    #[ts(type = "number")]
    pub version: u64,
    /// Chart built around several people: the boxes of all of them (first is the root). Absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub chosen_nodes: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, TS)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ChartNav {
    pub parent: Option<u32>,
    pub child: Option<u32>,
    pub prev: Option<u32>,
    pub next: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChartNode {
    pub id: u32,
    pub person: Option<PersonRef>,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    #[serde(rename = "gen")]
    #[ts(rename = "gen")]
    pub generation: i16,
    pub ahnen: Option<u32>,
    pub dup_of: Option<u32>,
    pub more_up: bool,
    pub more_down: bool,
    pub nav: ChartNav,
    pub label: String,
    /// Relatives chart only: how this person is related to the centred one ("brother", "half sister").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub relation: Option<String>,
    /// The same information as `relation` / the role inside `label`, as data, so the UI can word it in its own language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub role: Option<RoleDto>,
    /// How this person is related to the chart's reference person, as data (see `Kinship`); absent for the
    /// reference person, for unrelated people and when the boxes carry no text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub kinship: Option<Kinship>,
}

wire_enum!(
    /// The shape of a `Kinship`.
    KinshipKind, "snake_case", {
        SelfPerson, Blood, Partner, PartnerOfRelative, RelativeOfPartner
    }
);

/// How `to` is related to `from`, structured so the UI words it in its own language.
/// `up` and `down` count generations from `from` up to the nearest common ancestor and from there down to the
/// person described. Meaning by `kind`:
/// - `blood`: `to` is that blood relative of `from`; `half` when only one parent is shared at the junction.
/// - `partner`: `to` is a partner of `from` (`married` when the union is a marriage).
/// - `partner_of_relative`: `to` is the partner of the blood relative described by `up`/`down`/`half`.
/// - `relative_of_partner`: `to` is the blood relative described by `up`/`down`/`half` of a partner of `from`
///   whose sex is `via` (husband or wife; the Ukrainian words depend on it).
///
/// `sex` is the sex of `to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Kinship {
    pub kind: KinshipKind,
    pub up: u8,
    pub down: u8,
    pub half: bool,
    pub married: bool,
    pub sex: SexKind,
    pub via: SexKind,
}

/// What a chart person is: how they are related, or which generation / ancestor number they are.
/// `n` is the number of generations (or the ancestor or generation number), `k` the position among children
/// in a descendant outline (for `kin` and `partner_of_kin`: `n` generations up to the common ancestor and `k`
/// down from it), `sex` the sex of the person described.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct RoleDto {
    pub kind: RoleKind,
    pub n: i32,
    pub half: bool,
    pub sex: SexKind,
    pub k: u32,
    /// Partner roles only: the union with the person this one sits beside is a marriage (husband / wife).
    pub married: bool,
}

impl RoleDto {
    pub fn new(kind: RoleKind, n: i32, sex: SexKind) -> Self {
        Self {
            kind,
            n,
            half: false,
            sex,
            k: 0,
            married: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ChartEdge {
    pub from: u32,
    pub to: u32,
    pub kind: EdgeKindDto,
    /// Flat `x0, y0, x1, y1, ...`.
    pub pts: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct ChartWindow {
    pub nodes: Vec<ChartNode>,
    pub edges: Vec<ChartEdge>,
}

/// A heading above one family (connected group of people) in the whole-tree chart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WholeBlockDto {
    /// "Koval family (34 people)" or "Unconnected people (12)": in English, for tools. The app
    /// words the heading in its own language from `surname`, `people` and `unconnected`.
    pub title: String,
    /// The family's surname; empty when it has none and for the unconnected people.
    pub surname: String,
    pub people: u32,
    /// The block of people who have no family link at all.
    pub unconnected: bool,
    /// The heading strip; the block's people sit below it (above it never, in either direction).
    pub rect: Rect,
    /// Height of the area under the heading that holds the block's people.
    pub body_h: f32,
}

/// The whole-tree chart: the usual chart meta plus the block headings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WholeMeta {
    pub meta: ChartMeta,
    pub blocks: Vec<WholeBlockDto>,
    /// A chart of chosen people: how many of them are still in the project (never the markers).
    /// 0 for the chart of everyone.
    pub chosen: u32,
}

/// Where a person sits in a whole-tree chart.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChartLocation {
    pub node_id: u32,
    pub rect: Rect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OutlineItem {
    pub node_id: u32,
    pub person: Option<PersonRef>,
    /// "2 Father", "Child 3".
    pub label: String,
    pub has_children: bool,
    /// Relatives chart only: how the person is related to the centred one, as the plain label ("brother"),
    /// for callers that need it as data rather than inside `label`. Absent for the centred person and headings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub relation: Option<String>,
    /// The role as data (see `ChartNode::role`); absent for group headings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub role: Option<RoleDto>,
    /// How the person is related to the chart's reference person (see `ChartNode::kinship`); pedigree and
    /// descendants outlines only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub kinship: Option<Kinship>,
    /// Set on the heading rows of the relatives outline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub group: Option<OutlineGroupKind>,
    /// The person is also drawn earlier on the chart.
    #[serde(default)]
    pub appears_earlier: bool,
    /// The person is also listed higher up in the relatives outline.
    #[serde(default)]
    pub also_above: bool,
}

/// A child in a `FamilyDto`: the child plus whether this family is the child's preferred one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FamilyChildDto {
    #[serde(flatten)]
    pub child: ChildDto,
    pub preferred: bool,
}

/// One family (union) read by id (`Api.getFamily`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FamilyDto {
    pub id: Uuid,
    pub partners: (Option<PersonRef>, Option<PersonRef>),
    pub kind: UnionKindId,
    pub summary: Option<String>,
    pub events: Vec<EventDto>,
    pub children: Vec<FamilyChildDto>,
    pub notes: Vec<NoteDto>,
}

/// One entry of the recent-projects list (`Api.recentProjects`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RecentProject {
    pub dir: String,
    pub name: String,
    /// Unix milliseconds of the last open.
    #[ts(type = "number")]
    pub opened_at: i64,
    #[serde(default)]
    pub missing: bool,
}
