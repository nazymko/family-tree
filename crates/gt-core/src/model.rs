use crate::date::GenDate;
use crate::raw::RawNode;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Common {
    pub id: Uuid,
    pub private: bool,
    pub changed: Option<i64>,
    pub citations: Vec<Uuid>,
    pub notes: Vec<Uuid>,
    pub media: Vec<MediaLink>,
    pub ext_ids: Vec<ExternalId>,
    pub unknown: Vec<RawNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ExternalId {
    pub kind: ExtIdKind,
    pub value: String,
}

/// `ExtIdKind::Exid` schemes starting with this hold an agent's own numbering (`agent:<file>`).
/// They stay in the project and are left out of every export.
pub const AGENT_NUMBER_SCHEME: &str = "agent:";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExtIdKind {
    Xref,
    Uid,
    FamilySearch,
    AncestryApid,
    Exid { scheme: String },
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Assertion {
    pub preferred: bool,
    pub proof: Option<Proof>,
    pub confidence: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Proof {
    Proven,
    Disproven,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Sex {
    Male,
    Female,
    Intersex,
    Unknown,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Person {
    pub common: Common,
    pub sex: Sex,
    pub names: Vec<Name>,
    pub living_override: Option<LivingOverride>,
    /// The "some more brothers and sisters, unnamed" entry of a family: no names, no events, not
    /// counted, searched, exported or used in kinship and living calculations. Old change-log
    /// rows have no such field.
    #[serde(default)]
    pub sibling_marker: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LivingOverride {
    Living,
    Deceased,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Name {
    pub id: Uuid,
    pub kind: NameKind,
    pub assertion: Assertion,
    pub private: bool,
    pub prefix: Option<String>,
    pub given: Option<String>,
    pub call: Option<String>,
    pub surname_prefix: Option<String>,
    pub surnames: Vec<String>,
    pub patronymic: Option<String>,
    pub suffix: Option<String>,
    pub nickname: Option<String>,
    pub lang: Option<String>,
    pub date: Option<GenDate>,
    pub raw: Option<String>,
    pub citations: Vec<Uuid>,
    pub notes: Vec<Uuid>,
    pub unknown: Vec<RawNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NameKind {
    Birth,
    Married,
    Aka,
    Religious,
    Nickname,
    Immigrant,
    Professional,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Family {
    pub common: Common,
    pub partner1: Option<Uuid>,
    pub partner2: Option<Uuid>,
    pub kind: UnionKind,
    pub children: Vec<ChildLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UnionKind {
    Married,
    CivilUnion,
    Unmarried,
    Unknown,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChildLink {
    pub person: Uuid,
    pub rel1: ParentRel,
    pub rel2: ParentRel,
    pub preferred: bool,
    pub assertion: Assertion,
    pub citations: Vec<Uuid>,
    pub unknown: Vec<RawNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ParentRel {
    Birth,
    Adopted,
    Step,
    Foster,
    Guardian,
    Sealed,
    Unknown,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub common: Common,
    pub kind: EventKind,
    pub date: Option<GenDate>,
    pub place: Option<Uuid>,
    pub description: Option<String>,
    pub age: Option<String>,
    pub cause: Option<String>,
    pub negative: bool,
    pub assertion: Assertion,
    pub participants: Vec<Participant>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Participant {
    pub target: ParticipantRef,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ParticipantRef {
    Person(Uuid),
    Family(Uuid),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Role {
    Principal,
    Witness,
    Godparent,
    Clergy,
    Officiator,
    Informant,
    Parent,
    Father,
    Mother,
    Child,
    Spouse,
    Husband,
    Wife,
    Friend,
    Neighbor,
    Multiple,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EventKind {
    Birth,
    Christening,
    Baptism,
    Death,
    Missing,
    Burial,
    Cremation,
    Adoption,
    Marriage,
    MarriageBanns,
    MarriageContract,
    MarriageLicense,
    MarriageSettlement,
    Engagement,
    Divorce,
    DivorceFiled,
    Annulment,
    Separation,
    Census,
    Residence,
    Occupation,
    Education,
    Religion,
    Nationality,
    Title,
    Emigration,
    Immigration,
    Naturalization,
    Probate,
    Will,
    Graduation,
    Retirement,
    Military,
    PhysicalDescription,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Place {
    pub common: Common,
    pub names: Vec<PlaceName>,
    pub parent: Option<Uuid>,
    pub kind: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub raw: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlaceName {
    pub name: String,
    pub lang: Option<String>,
    pub date: Option<GenDate>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Source {
    pub common: Common,
    pub title: Option<String>,
    pub author: Option<String>,
    pub publication: Option<String>,
    pub abbreviation: Option<String>,
    pub text: Option<String>,
    pub repositories: Vec<RepoRef>,
    /// What kind of material the source is (a registry extract, a metric book, a photo...).
    pub kind: Option<SourceKind>,
    /// The office that issued the document (a particular registry or service centre).
    pub issuer: Option<String>,
    /// The number the issuing office gave the document.
    pub doc_number: Option<String>,
    /// The date on the document.
    pub doc_date: Option<GenDate>,
}

/// The kind of a source. The ids are stable (stored, exported to GEDCOM, used by the agent API);
/// `Other` carries a free label.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SourceKind {
    CivilRegistryExtract,
    CivilCertificate,
    MetricBook,
    Census,
    HouseholdBook,
    MilitaryRecord,
    ArchiveDocument,
    PersonalDocument,
    Photo,
    Gravestone,
    OralTestimony,
    BookOrArticle,
    WebsiteOrDatabase,
    GenealogyTree,
    ResearchAnalysis,
    /// Anything else; the text is a label, empty for plain "other".
    Other(String),
}

impl SourceKind {
    /// The ids of the fixed kinds, in display order, `other` last.
    pub const IDS: [&'static str; 16] = [
        "civil_registry_extract",
        "civil_certificate",
        "metric_book",
        "census",
        "household_book",
        "military_record",
        "archive_document",
        "personal_document",
        "photo",
        "gravestone",
        "oral_testimony",
        "book_or_article",
        "website_or_database",
        "genealogy_tree",
        "research_analysis",
        "other",
    ];

    /// The stable id; `other` with a label is `other:<label>`.
    pub fn id(&self) -> String {
        use SourceKind::*;
        let s = match self {
            CivilRegistryExtract => "civil_registry_extract",
            CivilCertificate => "civil_certificate",
            MetricBook => "metric_book",
            Census => "census",
            HouseholdBook => "household_book",
            MilitaryRecord => "military_record",
            ArchiveDocument => "archive_document",
            PersonalDocument => "personal_document",
            Photo => "photo",
            Gravestone => "gravestone",
            OralTestimony => "oral_testimony",
            BookOrArticle => "book_or_article",
            WebsiteOrDatabase => "website_or_database",
            GenealogyTree => "genealogy_tree",
            ResearchAnalysis => "research_analysis",
            Other(l) if l.trim().is_empty() => "other",
            Other(l) => return format!("other:{}", l.trim()),
        };
        s.to_string()
    }

    /// Reads an id. An unknown id becomes `Other(text)`; an empty text is none.
    pub fn parse(s: &str) -> Option<SourceKind> {
        use SourceKind::*;
        let t = s.trim();
        Some(match t {
            "" => return None,
            "civil_registry_extract" => CivilRegistryExtract,
            "civil_certificate" => CivilCertificate,
            "metric_book" => MetricBook,
            "census" => Census,
            "household_book" => HouseholdBook,
            "military_record" => MilitaryRecord,
            "archive_document" => ArchiveDocument,
            "personal_document" => PersonalDocument,
            "photo" => Photo,
            "gravestone" => Gravestone,
            "oral_testimony" => OralTestimony,
            "book_or_article" => BookOrArticle,
            "website_or_database" => WebsiteOrDatabase,
            "genealogy_tree" => GenealogyTree,
            "research_analysis" => ResearchAnalysis,
            "other" => Other(String::new()),
            _ => match t.strip_prefix("other:") {
                Some(l) => Other(l.trim().to_string()),
                None => Other(t.to_string()),
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RepoRef {
    pub repository: Uuid,
    pub call_number: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Repository {
    pub common: Common,
    pub name: String,
    pub address: Option<String>,
    pub urls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Citation {
    pub common: Common,
    pub source: Option<Uuid>,
    pub page: Option<String>,
    pub date: Option<GenDate>,
    pub text: Option<String>,
    pub quality: Option<u8>,
    pub evidence: Evidence,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Evidence {
    pub original: Option<bool>,
    pub primary_info: Option<bool>,
    pub direct: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Media {
    pub common: Common,
    pub path: String,
    pub original_name: Option<String>,
    pub mime: Option<String>,
    pub title: Option<String>,
    pub date: Option<GenDate>,
    pub description: Option<String>,
    pub sha256: Option<String>,
    /// What is written on the picture, in reading order. The text belongs to the picture
    /// wherever it is attached. Old change-log rows have no such field.
    #[serde(default)]
    pub transcription: Vec<TextRegion>,
    /// How the picture is shown: degrees clockwise (0, 90, 180 or 270) on top of its EXIF
    /// orientation. `None`: nobody has turned it, which shows as 0. The transcription's areas stay
    /// on the picture upright by EXIF, before this turn. Old change-log rows have no such field.
    #[serde(default)]
    pub rotation: Option<u16>,
    /// Where it was made (a recording recorded, a picture taken): a place of the same kind an
    /// event has. Old change-log rows have no such field.
    #[serde(default)]
    pub place: Option<Uuid>,
}

impl Media {
    /// The turn the picture is shown with: 0 when nobody has set one.
    pub fn shown_rotation(&self) -> u16 {
        self.rotation.unwrap_or(0)
    }
}

/// One piece of the transcription of a picture: a text and, when known, where it is written.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TextRegion {
    pub id: Uuid,
    /// Where on the picture, as fractions of the picture as displayed (after its EXIF
    /// orientation). `None`: text with no place on the picture, such as a whole transcript.
    #[serde(default)]
    pub area: Option<Region>,
    /// Degrees clockwise the writing is turned: 0, 90, 180 or 270.
    #[serde(default)]
    pub rotation: u16,
    pub text: String,
    /// On a sound recording: when in it this piece is said. A time-coded transcript is the
    /// recording's counterpart of a picture's pieces; it is optional and absent by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<TimeSpan>,
    /// On a sound recording: who says it, as the owner or agent wrote it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
}

/// A stretch of a sound recording, in milliseconds from its start; `start_ms < end_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TimeSpan {
    pub start_ms: u64,
    pub end_ms: u64,
}

/// A stretch of a sound recording linked to the record where its story is told: the time
/// counterpart of a picture region on a media link.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Clip {
    /// Stable across edits of its times or label, so a change is one item changing.
    pub id: Uuid,
    pub start_ms: u64,
    pub end_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Characters of a clip's label.
pub const CLIP_LABEL_MAX: usize = 200;
/// Characters of a transcript piece's speaker.
pub const SPEAKER_MAX: usize = 200;

/// The rotations a text region may have.
pub const TEXT_ROTATIONS: [u16; 4] = [0, 90, 180, 270];

/// The smallest width or height of a marked piece, as a fraction of the picture. The app's
/// viewer draws no smaller box either.
pub const TEXT_AREA_MIN: f32 = 0.01;

/// Bytes of the text of one piece of a transcription.
pub const TEXT_PIECE_MAX: usize = 64 * 1024;

/// Makes a stored piece sound again: an area is pulled onto the picture (and dropped when
/// nothing of it is left), an unknown rotation becomes 0. True when anything changed.
pub fn mend_text_region(r: &mut TextRegion) -> bool {
    let mut changed = false;
    if !TEXT_ROTATIONS.contains(&r.rotation) {
        r.rotation = 0;
        changed = true;
    }
    if r.time.is_some_and(|t| t.start_ms >= t.end_ms) {
        r.time = None;
        changed = true;
    }
    if let Some(a) = r.area.clone() {
        let ok = [a.x, a.y, a.w, a.h].iter().all(|v| v.is_finite());
        let x = if ok { a.x.clamp(0.0, 1.0) } else { 0.0 };
        let y = if ok { a.y.clamp(0.0, 1.0) } else { 0.0 };
        let w = if ok { a.w.min(1.0 - x) } else { 0.0 };
        let h = if ok { a.h.min(1.0 - y) } else { 0.0 };
        let mended = (w > 0.0 && h > 0.0).then_some(Region { x, y, w, h });
        if mended.as_ref() != Some(&a) {
            r.area = mended;
            changed = true;
        }
    }
    changed
}

/// The whole transcription as one text: the parts in order, each trimmed, empty ones left out,
/// with a blank line between them.
pub fn transcription_text(regions: &[TextRegion]) -> String {
    regions
        .iter()
        .map(|r| r.text.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MediaLink {
    pub media: Uuid,
    pub region: Option<Region>,
    pub primary: bool,
    /// A stretch of a sound recording: the link is to that stretch only. Old rows have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<Clip>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Region {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Note {
    pub common: Common,
    pub kind: NoteKind,
    pub text: String,
    pub lang: Option<String>,
    pub shared: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum NoteKind {
    General,
    Story,
    Research,
    Transcription,
    Conclusion,
    Other(String),
}
