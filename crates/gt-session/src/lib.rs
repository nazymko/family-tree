#![forbid(unsafe_code)]
//! Session layer between the UI and the stores: an open project held in memory, with DTOs,
//! paged person lists, the family view and person detail. No async; commands wrap these
//! calls in `spawn_blocking`.

pub mod chart;
pub mod command;
pub mod dates;
pub mod dto;
pub mod error;
pub mod family_view;
pub mod index;
pub mod names;
pub mod ops;
pub mod person_detail;
pub mod query;
pub mod session;
pub mod settings;
pub mod undo;

pub use command::{
    CitationArg, CitationInput, CitationPatch, CloneOf, DeleteImpact, EditCommand, EditResult,
    EventInput, EventOwner, EventOwnerKind, EvidenceInput, HistoryEntry, HistoryOp, LivingPatch,
    NameInput, NewSource, NoteInput, OwnerKind, OwnerRef, ParticipantInput, PersonDraft, Relation,
    ReuseLast, SourceInput, SourceRef, UndoState,
};
pub use dto::*;
pub use error::{ApiError, ErrorCode};
pub use names::{display_name, lifespan};
pub use session::{Session, SessionClock};
pub use settings::SettingsPatch;

pub mod lookup;
pub use lookup::parse_date_preview;

pub mod media;
pub mod thumbs;
pub use media::{AttachFailure, AttachResult, ExifSuggestion, MediaSize};
