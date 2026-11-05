//! The error every command returns to the UI: a stable `code` and a sentence the user can act on.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotFound,
    Invalid,
    Conflict,
    Io,
    NoProject,
    Stale,
    Internal,
    /// The agent gate said no; the message says what to do instead.
    AgentRefused,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[error("{message}")]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn not_found(what: &str, id: impl std::fmt::Display) -> Self {
        Self::new(
            ErrorCode::NotFound,
            format!("No {what} with id \"{id}\" in this project."),
        )
    }
}

impl From<gt_store::StoreError> for ApiError {
    fn from(e: gt_store::StoreError) -> Self {
        use gt_store::StoreError as S;
        let code = match &e {
            S::Io(_) => ErrorCode::Io,
            S::NotAProject(_) => ErrorCode::NoProject,
            S::NewerSchema { .. } => ErrorCode::Invalid,
            S::AlreadyExists(_) | S::Locked(_) => ErrorCode::Conflict,
            S::Sql(_) | S::Corrupt(_) => ErrorCode::Internal,
        };
        let message = match &e {
            S::NotAProject(p) => format!(
                "The folder {} is not a Gene Tree project. Choose a folder that contains tree.db.",
                p.display()
            ),
            S::NewerSchema { found, supported } => format!(
                "This project was saved by a newer version (schema {found}, this app supports {supported}). Update the app to open it."
            ),
            other => other.to_string(),
        };
        Self { code, message }
    }
}
