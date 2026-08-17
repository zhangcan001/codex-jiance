use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error")]
    Database(#[from] sqlx::Error),

    #[error("Database migration error")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Tauri error: {0}")]
    Tauri(String),

    #[error("Invalid application state: {0}")]
    InvalidState(String),

    #[error("Process error: {0}")]
    Process(String),

    #[error("Process timed out: {0}")]
    ProcessTimeout(String),

    #[error("Codex detection error: {0}")]
    CodexDetection(String),

    #[error("Codex CLI was not found: {0}")]
    CodexNotFound(String),

    #[error("App Server is unavailable: {0}")]
    AppServerUnavailable(String),

    #[error("App Server start failed: {0}")]
    AppServerStart(String),

    #[error("App Server stop failed: {0}")]
    AppServerStop(String),

    #[error("Unknown application error: {0}")]
    Unknown(String),
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self::Unknown(message)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl From<AppError> for CommandError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::Io(error) => Self {
                code: "IO_ERROR".to_owned(),
                message: format!("File system operation failed: {error}"),
            },
            AppError::Database(_) => Self {
                code: "DATABASE_ERROR".to_owned(),
                message: "Database operation failed.".to_owned(),
            },
            AppError::Migration(_) => Self {
                code: "DATABASE_ERROR".to_owned(),
                message: "Database migration failed.".to_owned(),
            },
            AppError::Serialization(error) => Self {
                code: "SERIALIZATION_ERROR".to_owned(),
                message: format!("Response serialization failed: {error}"),
            },
            AppError::Tauri(message) => Self {
                code: "TAURI_ERROR".to_owned(),
                message,
            },
            AppError::InvalidState(message) => Self {
                code: "INVALID_STATE".to_owned(),
                message,
            },
            AppError::Process(message) => Self {
                code: "PROCESS_ERROR".to_owned(),
                message,
            },
            AppError::ProcessTimeout(message) => Self {
                code: "PROCESS_TIMEOUT".to_owned(),
                message,
            },
            AppError::CodexDetection(message) => Self {
                code: "CODEX_DETECTION_ERROR".to_owned(),
                message,
            },
            AppError::CodexNotFound(message) => Self {
                code: "CODEX_NOT_FOUND".to_owned(),
                message,
            },
            AppError::AppServerUnavailable(message) => Self {
                code: "APP_SERVER_UNAVAILABLE".to_owned(),
                message,
            },
            AppError::AppServerStart(message) => Self {
                code: "APP_SERVER_START_ERROR".to_owned(),
                message,
            },
            AppError::AppServerStop(message) => Self {
                code: "APP_SERVER_STOP_ERROR".to_owned(),
                message,
            },
            AppError::Unknown(message) => Self {
                code: "UNKNOWN_ERROR".to_owned(),
                message,
            },
        }
    }
}

pub type CommandResult<T> = Result<T, CommandError>;
