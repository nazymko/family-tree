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
    pub backups_to_keep: u32,
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
            backups_to_keep: 10,
            living_max_age: 100,
            home_person: None,
        }
    }
}
