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
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MediaLink {
    pub media: Uuid,
    pub region: Option<Region>,
    pub primary: bool,
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
