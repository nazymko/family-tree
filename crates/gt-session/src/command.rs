//! The edit commands the UI sends (Rust twin of `EditCommand` in `app/src/lib/api/types.ts`) and
//! the results of applying, undoing and redoing them. Field names are camelCase on the wire;
//! commands are tagged by `type`.

use crate::dto::{
    LivingOverrideDto, NameKindDto, NoteKindDto, ParentRelId, PersonRef, SexKind, UnionKindId,
};
use crate::field_diff::FieldChangeDto;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// The `livingOverride` of `updatePerson`: absent keeps the value, `null` clears it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(untagged)]
pub enum LivingPatch {
    #[default]
    #[serde(skip)]
    #[ts(skip)]
    Keep,
    Clear,
    Set(LivingOverrideDto),
}

/// Lets `#[ts(optional)]` mark the key optional in the generated TypeScript.
impl ts_rs::IsOption for LivingPatch {
    type Inner = LivingOverrideDto;
}

impl LivingPatch {
    pub fn is_keep(&self) -> bool {
        *self == LivingPatch::Keep
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum OwnerKind {
    Person,
    Family,
    Name,
    Event,
    Media,
    Citation,
    /// A note: only media (voice samples) can be attached to it.
    Note,
    ChildLink,
    /// A source: only media (pictures of the document) can be attached to it.
    Source,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OwnerRef {
    pub kind: OwnerKind,
    pub id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum EventOwnerKind {
    Person,
    Family,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EventOwner {
    pub kind: EventOwnerKind,
    pub id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NameInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub id: Option<Uuid>,
    pub kind: NameKindDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub kind_text: Option<String>,
    pub preferred: bool,
    pub private: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub given: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub call: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub surname_prefix: Option<String>,
    pub surnames: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub patronymic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub suffix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub nickname: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lang: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub date_raw: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantInput {
    pub person: Uuid,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EventInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub id: Option<Uuid>,
    /// `birth`, `death`, ..., `custom:<text>`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub date_raw: Option<String>,
    /// Place as comma-separated text (leaf first); `Some("")` clears the place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub place_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub place_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub age: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cause: Option<String>,
    pub negative: bool,
    pub private: bool,
    pub preferred: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub participants: Option<Vec<ParticipantInput>>,
    /// The whole list of godparents, in order; left out, the event keeps the ones it has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub godparents: Option<Vec<GodparentInput>>,
}

/// A godparent of an event: a person in the tree or a name as written, exactly one of the two.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GodparentInput {
    /// The godparent being kept or changed; none for a new one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub person: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    /// `godfather`, `godmother` or `godparent`.
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PersonDraft {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub given: Option<String>,
    pub surnames: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub patronymic: Option<String>,
    pub sex: SexKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub birth_date_raw: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub birth_place_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NewSource {
    pub title: String,
}

/// An existing source id, or a title (reused when a source has that title, else created).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(untagged)]
pub enum SourceRef {
    Id(Uuid),
    New(NewSource),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub original: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub primary_info: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub direct: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CitationInput {
    pub source: SourceRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub page: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub date_raw: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "0 | 1 | 2 | 3")]
    pub quality: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub evidence: Option<EvidenceInput>,
}

/// A change to an existing citation; absent fields stay as they are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CitationPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub page: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub date_raw: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "0 | 1 | 2 | 3")]
    pub quality: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub evidence: Option<EvidenceInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReuseLast {
    pub reuse_last: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CloneOf {
    pub clone_of: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(untagged)]
pub enum CitationArg {
    Reuse(ReuseLast),
    Clone(CloneOf),
    New(CitationInput),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct NoteInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub id: Option<Uuid>,
    pub kind: NoteKindDto,
    pub text: String,
    /// The readable retelling. Missing leaves the note's story as it is; an empty text takes it off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub story: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SourceInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub publication: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub abbreviation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub text: Option<String>,
    /// A kind id (`metric_book`, `other:<label>`); an empty text clears it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub issuer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub doc_number: Option<String>,
    /// The date on the document, as typed; an empty text clears it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub doc_date_raw: Option<String>,
    /// The archive or office that keeps it (reused when a repository has this name, else created);
    /// it becomes the first repository of the source. An empty text removes that repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub archive: Option<String>,
    /// The call number (case, file) in that archive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub call_number: Option<String>,
}

/// Where a piece of transcribed text is written: fractions 0..1 of the picture as displayed
/// (after its EXIF orientation), from its top left corner.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TextAreaInput {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

// Coordinates are checked to be finite before anything stores them, so equality is total.
impl Eq for TextAreaInput {}

/// A point on the Earth in millionths of a degree (lat -90..90, lon -180..180, times 1,000,000).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MapCorner {
    pub lat_e6: i32,
    pub lon_e6: i32,
}

/// How an old map lies on the Map. The order in the stack is kept by the tree (a new layer goes
/// on top); `ReorderMapLayers` changes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MapLayerInput {
    /// The series or atlas, such as "Schubert's two-verst map".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub series: Option<String>,
    /// The year or years it shows, as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub year: Option<String>,
    /// Where the scan came from: a web address or an archive reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source: Option<String>,
    /// The picture's corners: top left, top right, bottom right, bottom left. Absent while it is
    /// not placed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub corners: Option<Vec<MapCorner>>,
    /// 0 to 100.
    pub opacity: u8,
    pub visible: bool,
}

/// One piece of a picture's transcription, in reading order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TextRegionInput {
    /// Keeps a piece that is already there; a new one gets an id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub id: Option<Uuid>,
    /// Absent: text with no place on the picture (a whole transcript).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub area: Option<TextAreaInput>,
    /// Degrees clockwise the writing is turned: 0, 90, 180 or 270 (absent: 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "0 | 90 | 180 | 270")]
    pub rotation: Option<u16>,
    pub text: String,
    /// On a recording: when the piece is said, in milliseconds; give both or neither.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub start_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub end_ms: Option<u64>,
    /// On a recording: who says it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub speaker: Option<String>,
}

/// One person on the trace screen. `existing` set: the card stands for a person already in the
/// tree and nothing is created for it; otherwise a new person is made from the other fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DraftPerson {
    /// The caller's name for this card; families refer to it. Unique within one draft.
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub existing: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub given: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub surname: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub patronymic: Option<String>,
    pub sex: SexKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub birth_date_raw: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub death_date_raw: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub notes: Option<String>,
}

/// A family of the draft: partners and children are `DraftPerson::key`s.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DraftFamily {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub partner1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub partner2: Option<String>,
    /// The family already in the tree that the children join. Needed only when a parent from the
    /// tree is in more than one family; with exactly one, that family is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub existing_family: Option<Uuid>,
    pub children: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Relation {
    Father,
    Mother,
    Partner,
    Child,
    Sibling,
}

impl Relation {
    pub fn as_str(self) -> &'static str {
        match self {
            Relation::Father => "father",
            Relation::Mother => "mother",
            Relation::Partner => "partner",
            Relation::Child => "child",
            Relation::Sibling => "sibling",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EditCommand {
    CreatePerson {
        draft: PersonDraft,
    },
    UpdatePerson {
        id: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        sex: Option<SexKind>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        sex_text: Option<String>,
        #[serde(default, skip_serializing_if = "LivingPatch::is_keep")]
        #[ts(optional = nullable)]
        living_override: LivingPatch,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        private: Option<bool>,
    },
    DeletePerson {
        id: Uuid,
    },
    SaveName {
        person: Uuid,
        name: NameInput,
    },
    DeleteName {
        person: Uuid,
        name: Uuid,
    },
    SaveEvent {
        owner: EventOwner,
        event: EventInput,
    },
    DeleteEvent {
        owner: EventOwner,
        event: Uuid,
    },
    /// One more godparent on a baptism or christening.
    AddGodparent {
        event: Uuid,
        godparent: GodparentInput,
    },
    /// Takes one godparent off an event.
    RemoveGodparent {
        event: Uuid,
        godparent: Uuid,
    },
    QuickAdd {
        anchor: Uuid,
        relation: Relation,
        draft: PersonDraft,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        family: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        new_parent_set: Option<bool>,
        /// Father, mother or child: how they are related when it is not by birth (stepfather, adopted son, ...).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        parent_rel: Option<ParentRelId>,
    },
    /// Like QuickAdd, with someone already in the tree as the relative.
    LinkExisting {
        anchor: Uuid,
        relation: Relation,
        person: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        family: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        new_parent_set: Option<bool>,
        /// Father, mother or child: how they are related when it is not by birth (stepfather, adopted son, ...).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        parent_rel: Option<ParentRelId>,
    },
    SaveFamily {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        id: Option<Uuid>,
        partner1: Option<Uuid>,
        partner2: Option<Uuid>,
        kind: UnionKindId,
    },
    DeleteFamily {
        id: Uuid,
    },
    LinkChild {
        family: Uuid,
        child: Uuid,
        rel: (ParentRelId, ParentRelId),
        preferred: bool,
    },
    UpdateChildLink {
        family: Uuid,
        child: Uuid,
        rel: (ParentRelId, ParentRelId),
        preferred: bool,
    },
    UnlinkChild {
        family: Uuid,
        child: Uuid,
    },
    ReorderChildren {
        family: Uuid,
        order: Vec<Uuid>,
    },
    /// Adds the one entry per family that stands for more brothers and sisters nobody named.
    AddSiblingMarker {
        family: Uuid,
    },
    SaveNote {
        target: OwnerRef,
        note: NoteInput,
    },
    DeleteNote {
        target: OwnerRef,
        note: Uuid,
    },
    SaveSource {
        source: SourceInput,
    },
    AddCitation {
        target: OwnerRef,
        citation: CitationArg,
    },
    UpdateCitation {
        id: Uuid,
        citation: CitationPatch,
    },
    RemoveCitation {
        target: OwnerRef,
        citation: Uuid,
    },
    AttachMedia {
        target: OwnerRef,
        media: Vec<Uuid>,
    },
    DetachMedia {
        target: OwnerRef,
        media: Uuid,
    },
    /// Removes the media record and every link to it. The file stays in the project, so undo can bring it back.
    DeleteMedia {
        id: Uuid,
    },
    SetPrimaryMedia {
        person: Uuid,
        media: Uuid,
    },
    UpdateMedia {
        id: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        date_raw: Option<String>,
        /// Where it was made, as comma-separated text (smallest place first), found or created
        /// like an event's place; `Some("")` clears the place.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        place_text: Option<String>,
        /// An existing place, as the place picker gives it; wins over `place_text`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        place_id: Option<Uuid>,
    },
    /// Replaces the whole transcription of a picture: every piece, in reading order.
    SetMediaTranscription {
        id: Uuid,
        regions: Vec<TextRegionInput>,
        /// The ids of the pieces the sender knew of. A stored piece not among them was added
        /// meanwhile (by an agent, say) and stays, after the piece it followed. Absent: `regions`
        /// is the whole transcription.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        known: Option<Vec<Uuid>>,
    },
    /// How the picture is shown: degrees clockwise (0, 90, 180 or 270) on top of its EXIF
    /// orientation. The transcription's areas stay as they are, on the picture upright by EXIF.
    SetMediaRotation {
        id: Uuid,
        #[ts(type = "0 | 90 | 180 | 270")]
        rotation: u16,
    },
    /// Makes files versions of one page (copies in other quality): `primary` is shown and opened
    /// first, `others` are kept as its versions. A file already in a group brings its whole group
    /// along, so one command also joins two groups or makes another version the primary. No file,
    /// transcription or link is touched.
    GroupMediaVersions {
        primary: Uuid,
        others: Vec<Uuid>,
    },
    /// Takes one file out of its version group; it becomes a file of its own again. When it was
    /// the primary, `new_primary` (or else the first other version) is shown first from now on.
    UngroupMediaVersion {
        media: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        new_primary: Option<Uuid>,
    },
    /// A stretch of a sound recording, linked to the record where its story is told: a note,
    /// citation, event, person or family. Times in milliseconds from the start of the recording.
    AddAudioClip {
        target: OwnerRef,
        media: Uuid,
        #[ts(type = "number")]
        start_ms: u64,
        #[ts(type = "number")]
        end_ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        label: Option<String>,
    },
    /// New times and label for a clip, and with `move_to` the record it is linked to instead.
    UpdateAudioClip {
        target: OwnerRef,
        clip: Uuid,
        #[ts(type = "number")]
        start_ms: u64,
        #[ts(type = "number")]
        end_ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        label: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        move_to: Option<OwnerRef>,
    },
    DeleteAudioClip {
        target: OwnerRef,
        clip: Uuid,
    },
    /// Everything traced from a picture, in ONE undoable step. With `attach_picture` the staged
    /// trace picture is stored as a media item attached to every new person and to a new source.
    ImportDraft {
        persons: Vec<DraftPerson>,
        families: Vec<DraftFamily>,
        attach_picture: bool,
    },
    /// Makes a place from text, smallest place first ("Village, District, Oblast"), reusing the
    /// ones that exist. The place's id comes back as the result's focus.
    CreatePlace {
        text: String,
    },
    /// Sets the point of a place on the map, or clears it (both absent). Millionths of a degree
    /// (the command stays comparable): lat -90..90, lon -180..180, times 1,000,000.
    SetPlaceCoordinates {
        place: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        lat_e6: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        lon_e6: Option<i32>,
    },
    /// Lays a picture (an old scanned map) on the Map, changes how it lies, or takes it off the
    /// Map (`layer` absent). The picture itself stays in the tree either way. `title`, when
    /// given, is the picture's title; empty takes it away.
    SetMapLayer {
        media: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        layer: Option<MapLayerInput>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        title: Option<String>,
    },
    /// The order the old maps are drawn in, the lowest first. Every layer of the tree, once.
    ReorderMapLayers {
        order: Vec<Uuid>,
    },
    /// Shows some old maps and hides others, as one change: a whole series shown or hidden, or
    /// one map shown alone. Maps that are already so are left as they are; a map in both lists
    /// is refused.
    SetMapLayersVisible {
        #[serde(default)]
        shown: Vec<Uuid>,
        #[serde(default)]
        hidden: Vec<Uuid>,
    },
    /// An agent's own number for a person, unique within one named source file. Never exported.
    AddNumber {
        person: Uuid,
        source_file: String,
        number: String,
    },
}

impl EditCommand {
    /// The text shown in toasts and as the undo label.
    pub fn label(&self) -> String {
        match self {
            EditCommand::CreatePerson { .. } => "Add person".into(),
            EditCommand::UpdatePerson { .. } => "Edit person".into(),
            EditCommand::DeletePerson { .. } => "Delete person".into(),
            EditCommand::SaveName { .. } => "Edit name".into(),
            EditCommand::DeleteName { .. } => "Delete name".into(),
            EditCommand::SaveEvent { .. } => "Save event".into(),
            EditCommand::DeleteEvent { .. } => "Delete event".into(),
            EditCommand::AddGodparent { .. } => "Add godparent".into(),
            EditCommand::RemoveGodparent { .. } => "Remove godparent".into(),
            EditCommand::QuickAdd {
                relation,
                parent_rel,
                ..
            }
            | EditCommand::LinkExisting {
                relation,
                parent_rel,
                ..
            } => match (relation, parent_rel) {
                (Relation::Father, Some(ParentRelId::Step)) => "Add stepfather".into(),
                (Relation::Mother, Some(ParentRelId::Step)) => "Add stepmother".into(),
                _ => format!("Add {}", relation.as_str()),
            },
            EditCommand::SaveFamily { .. } => "Save family".into(),
            EditCommand::DeleteFamily { .. } => "Delete family".into(),
            EditCommand::LinkChild { .. } => "Add child to family".into(),
            EditCommand::UpdateChildLink { .. } => "Edit child link".into(),
            EditCommand::UnlinkChild { .. } => "Remove child from family".into(),
            EditCommand::ReorderChildren { .. } => "Reorder children".into(),
            EditCommand::AddSiblingMarker { .. } => "Add unnamed brothers and sisters".into(),
            EditCommand::SaveNote { .. } => "Save note".into(),
            EditCommand::DeleteNote { .. } => "Delete note".into(),
            EditCommand::SaveSource { .. } => "Save source".into(),
            EditCommand::AddCitation { .. } => "Add citation".into(),
            EditCommand::UpdateCitation { .. } => "Edit citation".into(),
            EditCommand::RemoveCitation { .. } => "Remove citation".into(),
            EditCommand::AttachMedia { .. } => "Attach media".into(),
            EditCommand::DetachMedia { .. } => "Detach media".into(),
            EditCommand::DeleteMedia { .. } => "Delete media".into(),
            EditCommand::SetPrimaryMedia { .. } => "Set primary media".into(),
            EditCommand::UpdateMedia { .. } => "Edit media".into(),
            EditCommand::SetMediaTranscription { .. } => "Edit transcription".into(),
            EditCommand::SetMediaRotation { .. } => "Turn picture".into(),
            EditCommand::GroupMediaVersions { .. } => "Group versions".into(),
            EditCommand::UngroupMediaVersion { .. } => "Ungroup version".into(),
            EditCommand::AddAudioClip { .. } => "Add clip".into(),
            EditCommand::UpdateAudioClip { .. } => "Edit clip".into(),
            EditCommand::DeleteAudioClip { .. } => "Delete clip".into(),
            EditCommand::ImportDraft { .. } => "Add traced people".into(),
            EditCommand::CreatePlace { .. } => "Add place".into(),
            EditCommand::SetPlaceCoordinates { lat_e6, lon_e6, .. } => {
                if lat_e6.is_none() && lon_e6.is_none() {
                    "Clear place coordinates".into()
                } else {
                    "Set place coordinates".into()
                }
            }
            EditCommand::SetMapLayer { layer: None, .. } => "Take map off the Map".into(),
            EditCommand::SetMapLayer { .. } => "Change map layer".into(),
            EditCommand::ReorderMapLayers { .. } => "Reorder map layers".into(),
            EditCommand::SetMapLayersVisible { shown, hidden } => {
                match (shown.is_empty(), hidden.is_empty()) {
                    (false, true) => "Show maps on the Map".into(),
                    (true, false) => "Hide maps on the Map".into(),
                    _ => "Choose maps shown on the Map".into(),
                }
            }
            EditCommand::AddNumber { .. } => "Record number".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UndoState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_label: Option<String>,
    pub redo_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EditResult {
    pub label: String,
    pub focus: Option<Uuid>,
    /// Every record the change set wrote or deleted.
    pub touched: Vec<Uuid>,
    pub undo: UndoState,
}

/// A fact an undo from the change log left alone, because someone has changed it since.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFact {
    /// `person`, `family`, `event`, `note`, ...
    pub kind: String,
    pub person: Option<PersonRef>,
    pub detail: String,
    /// The same fact as data, for the UI to word in its own language.
    pub fact: crate::log::LogFactDto,
}

/// What an undo from the change log did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UndoReport {
    pub undone: u32,
    pub skipped: Vec<SkippedFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum HistoryOp {
    Create,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    #[ts(type = "number")]
    pub ts: i64,
    pub label: String,
    pub op: HistoryOp,
    /// What changed in a person's sex and names, field by field; empty for other records and
    /// for a row with no earlier version to compare (create, delete).
    pub fields: Vec<FieldChangeDto>,
}

/// What deleting a person takes with them, for the confirmation text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DeleteImpact {
    pub families: u32,
    pub events: u32,
}
