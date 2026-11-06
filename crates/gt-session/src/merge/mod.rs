//! Merging a relative's copy of the tree back into this one, and the copies themselves: a zip to
//! send, a received zip to open, a folder or zip to read as a scratch copy.
//!
//! Nothing here deletes a record, overwrites a field the owner changed, or trusts the copy: the
//! copy is opened from a scratch folder and every photo is checked against its hash.

mod apply;
mod copy;
pub mod dto;
mod plan;

pub use copy::{open_received_copy, remove_stale_scratch, remove_stale_scratch_now};
pub use dto::{
    CopySizeDto, CopyZipDto, MaybeSameDto, MergeCheckDto, MergeCheckKind, MergeHistory,
    MergeLineDto, MergePreviewDto, MergeResultDto, MergeWhy,
};
pub(crate) use plan::Plan;
