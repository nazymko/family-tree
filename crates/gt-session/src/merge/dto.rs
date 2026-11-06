//! What the merge screens show: the preview of a relative's copy and the result of applying it.
//! Everything the UI words (reasons, checks) is a token here; names and values are the family's own.

use crate::field_diff::FieldChangeDto;
use crate::log::LogChangeDto;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// What the copy's history lets the merge do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MergeHistory {
    /// The copy carries the history needed to tell the owner's edits from the relative's.
    Full,
    /// The copy has no history (an older app, or a tree that was only imported): everything that
    /// differs is shown for the owner to decide, and nothing is removed.
    None,
    /// The copy and this tree share people but no edit history (a restored backup, a rebuilt
    /// tree): the same fallback.
    Unrelated,
}

/// Why a line is listed and not applied (or why it was left out of the apply).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MergeWhy {
    /// Both of you changed the same thing differently.
    BothChanged,
    /// Removed here, changed in the copy.
    RemovedHere,
    /// Both of you added a record with the same id, and they differ.
    CreatedOnBoth,
    /// No history to tell who changed it, and the two differ.
    NoHistory,
    /// The change would make a living or private person easier to see.
    MoreVisible,
    /// The change would take something away (a name, a link, a witness).
    TakesAway,
    /// A photo the record needs is not in the copy.
    PhotoMissing,
    /// A photo's bytes do not match its hash.
    PhotoDamaged,
    /// A photo's path is not a normal path inside the tree.
    PhotoUnsafe,
    /// It points at a record that is not coming across.
    NeedsMissing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MaybeSameDto {
    pub id: Uuid,
    pub name: String,
    pub lifespan: String,
}

/// One record in the preview, in words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MergeLineDto {
    /// What the copy did to the record (for a removal: what it removed; for a conflict: what
    /// the relative changed it to).
    pub change: LogChangeDto,
    /// The person to open in the tree, when the record belongs to one the tree already has.
    pub person: Option<Uuid>,
    /// Set on a line that is not applied.
    pub why: Option<MergeWhy>,
    /// For a change taken from the copy or a removal: you also changed this record here.
    pub also_edited_here: bool,
    /// For a line that needs a decision: what you changed since the copy was made.
    pub yours: Vec<FieldChangeDto>,
    /// For a new person: people already in the tree who might be the same person. Never joined
    /// automatically.
    pub maybe_same: Vec<MaybeSameDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MergeCheckKind {
    /// The folder or zip is a Gene Tree copy that can be read.
    ReadableCopy,
    /// It is a copy of this tree (they share people or history).
    SameTree,
    /// What its history allows.
    History,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MergeCheckDto {
    pub kind: MergeCheckKind,
    pub ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MergePreviewDto {
    /// Identifies this preview; `merge_apply` accepts only the token of the current one.
    pub token: Uuid,
    /// Whose copy it is, as the owner named it.
    pub name: String,
    /// The folder or file the copy came from (its name only).
    pub source_name: String,
    pub history: MergeHistory,
    pub checks: Vec<MergeCheckDto>,
    /// Nothing in the copy needs doing or deciding.
    pub nothing_new: bool,
    /// Records the apply will write.
    pub to_apply: u32,
    pub added: Vec<MergeLineDto>,
    pub changed: Vec<MergeLineDto>,
    /// Changed on both sides, or more visible: listed, not applied.
    pub undecided: Vec<MergeLineDto>,
    /// Removed in the copy: listed, not applied.
    pub removed: Vec<MergeLineDto>,
    /// Cannot come across (a photo is missing or damaged, a needed record is not coming).
    pub blocked: Vec<MergeLineDto>,
    /// Records you changed that the copy leaves alone, or that you removed.
    pub left_alone: u32,
    pub photos: u32,
    #[ts(type = "number")]
    pub photo_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MergeResultDto {
    /// The merge as the change log knows it; `undo_merge` takes it.
    pub batch: Uuid,
    pub name: String,
    pub added: u32,
    pub changed: u32,
    pub photos_copied: u32,
    /// Lines that were listed and not applied (still waiting for you).
    pub left_for_you: u32,
}

/// What `save_copy_zip` made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CopyZipDto {
    pub path: String,
    #[ts(type = "number")]
    pub bytes: u64,
}

/// How big the zip of this tree would be, for the warning before it is made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CopySizeDto {
    /// The database plus the photos, before compression.
    #[ts(type = "number")]
    pub bytes: u64,
    pub photos: u32,
    pub people: u32,
}
