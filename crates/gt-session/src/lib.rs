#![forbid(unsafe_code)]
//! Session layer between the UI and the stores: an open project held in memory, with DTOs,
//! paged person lists, the family view and person detail. No async; commands wrap these
//! calls in `spawn_blocking`.

pub mod agent;
mod audio_len;
pub mod chart;
pub mod command;
pub mod dates;
pub mod dto;
pub mod error;
pub mod family_view;
pub mod field_diff;
pub mod file_stamp;
pub mod godparents;
pub mod import_format;
pub mod index;
pub mod kinship;
pub mod log;
pub mod map;
pub mod map_layers;
pub mod merge;
mod mp4_tracks;
pub mod ozi;
pub mod place_events;
pub mod place_import;
pub use merge::{CopySizeDto, CopyZipDto, MergePreviewDto, MergeResultDto};
pub mod names;
pub mod ops;
pub mod person_detail;
pub mod query;
pub mod recording;
pub mod relation;
pub mod revert;
pub mod session;
pub mod settings;
mod touched;
pub mod undo;
mod web_links;

pub use agent::{
    AGENT_AUDIO_MAX, AGENT_FILE_MAX, AGENT_PDF_MAX, AgentCtx, AgentFile, AgentWrite,
    EventCorrection, NameCorrection, PLACE_MERGE_MAX, PlaceMergeOutcome, VerifiedEvidence,
    check_agent_file, check_agent_path, prepare_agent_file,
};
pub use command::{
    CitationArg, CitationInput, CitationPatch, CloneOf, DeleteImpact, DraftFamily, DraftPerson,
    EditCommand, EditResult, EventInput, EventOwner, EventOwnerKind, EvidenceInput, GodparentInput,
    HistoryEntry, HistoryOp, LivingPatch, NameInput, NewSource, NoteInput, OwnerKind, OwnerRef,
    ParticipantInput, PersonDraft, Relation, ReuseLast, SkippedFact, SourceInput, SourceRef,
    TextAreaInput, TextRegionInput, UndoReport, UndoState,
};
pub use command::{MapCorner, MapLayerInput};
pub use dto::*;
pub use error::{ApiError, ErrorCode};
pub use field_diff::{FieldChangeDto, person_field_changes, record_field_changes};
pub use log::{
    ActorDto, BatchDto, BatchRef, LocationDto, LocationPage, LogAgentDto, LogChangeDto, LogFactDto,
    LogFilterDto, LogPersonDto, LogRowDto, RecordHomeDto, RecordRefDto, RevealTarget,
    ReviewScopeDto,
};
pub use map::{
    GazetteerCandidateDto, GazetteerCountryDto, GazetteerProgressDto, GazetteerStatusDto,
    MapDataDto, MapEventDto, MapLinkDto, MapPersonDto, MapPlaceDto, MapScope, MapScopeKind,
};
pub use map_layers::{MapLayerDto, OziPlacementDto, prepare_map_file};
pub use names::{display_name, lifespan};
pub use ops::VERSION_GROUP_MAX;
pub use place_events::{
    DecadeCount, KindCount, PLACE_EVENTS_PAGE_MAX, PLACE_RECORD_PEOPLE_MAX,
    PLACE_RECORDS_BATCH_MAX, PlaceEventDto, PlaceEventPersonDto, PlaceEventsPage, PlaceEventsQuery,
    PlaceEventsWho, coarse_kind, place_record_kinds,
};
pub use place_import::{
    IMPORT_CHUNK, ParsedIndex, PlaceImportGroupDto, PlaceImportKey, PlaceImportLineDto,
    PlaceImportMapDto, PlaceImportPreviewDto, PlaceImportProgressDto, PlaceImportResultDto,
    PlaceImportScanDto, parse_index_files,
};
pub use session::{Session, SessionClock};
pub use settings::SettingsPatch;
pub use touched::people_reached;

pub mod lookup;
pub use lookup::parse_date_preview;

pub mod ai_read;
pub mod media;
pub mod thumbs;
pub mod trace;
pub use ai_read::{AiPerson, AiReadResult, AiRelation, parse_ai_reply};
pub use media::{
    AttachFailure, AttachResult, ExifSuggestion, MAX_RANGE_BYTES, MediaAbout, MediaSize,
    MediaSlice, PreparedAttach, prepare_attach,
};
pub use thumbs::{one_decode_at_a_time, render_px};
pub use trace::{TraceDraftState, TraceImageInfo};
pub mod transfer;
pub use transfer::{
    ExportOptions, ExportVersion, GroupMedia, ImportPhase, ImportProgress, ImportReportDto,
    ImportRequest, LivingExport, MentionWarning, MissingMedia, MissingReasonDto, PrivacyPreview,
    RelinkResult, RepairDto, UnparsedDateDto, import_gedcom, import_gedcom_with_hooks,
    import_gedcom_with_opener,
};
