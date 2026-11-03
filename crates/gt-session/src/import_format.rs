//! The import-format seam lives in `gt-import` (shared with the `gt` command line); this module
//! re-exports it with errors as `ApiError`.

use crate::error::{ApiError, ErrorCode};
use gt_gedcom::ImportOptions;
use std::path::Path;

pub use gt_import::{Converted, ImportError, ImportErrorKind, ImportFormat, TempDirGuard};

impl From<ImportError> for ApiError {
    fn from(e: ImportError) -> Self {
        let code = match e.kind {
            ImportErrorKind::Io => ErrorCode::Io,
            ImportErrorKind::Invalid => ErrorCode::Invalid,
        };
        ApiError::new(code, e.message)
    }
}

/// Which format `path` holds (see `gt_import::detect`).
pub fn detect(path: &Path) -> Result<ImportFormat, ApiError> {
    Ok(gt_import::detect(path)?)
}

/// Convert `path` into an import outcome (see `gt_import::convert`).
pub fn convert(
    format: ImportFormat,
    path: &Path,
    opts: &ImportOptions,
) -> Result<Converted, ApiError> {
    Ok(gt_import::convert(format, path, opts)?)
}
