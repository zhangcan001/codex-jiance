use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O 错误：{0}")]
    Io(#[from] std::io::Error),

    #[error("数据库错误")]
    Database(#[from] sqlx::Error),

    #[error("数据库迁移错误")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("序列化错误：{0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Tauri 错误：{0}")]
    Tauri(String),

    #[error("应用状态无效：{0}")]
    InvalidState(String),

    #[error("进程错误：{0}")]
    Process(String),

    #[error("进程操作超时：{0}")]
    ProcessTimeout(String),

    #[cfg(test)]
    #[error("Codex detection error: {0}")]
    CodexDetection(String),

    #[cfg(test)]
    #[error("Codex CLI was not found: {0}")]
    CodexNotFound(String),

    #[cfg(test)]
    #[error("App Server is unavailable: {0}")]
    AppServerUnavailable(String),

    #[cfg(test)]
    #[error("App Server start failed: {0}")]
    AppServerStart(String),

    #[cfg(test)]
    #[error("App Server initialization failed: {0}")]
    AppServerInitialization(String),

    #[cfg(test)]
    #[error("Schema compatibility error: {0}")]
    SchemaCompatibility(String),

    #[cfg(test)]
    #[error("Schema generation is unavailable: {0}")]
    SchemaGenerationUnavailable(String),

    #[cfg(test)]
    #[error("Account service error: {0}")]
    AccountService(String),

    #[error("设置错误：{0}")]
    Settings(String),

    #[cfg(test)]
    #[error("App Server stop failed: {0}")]
    AppServerStop(String),

    #[cfg(test)]
    #[error("RPC remote error: {0}")]
    RpcRemote(String),

    #[cfg(test)]
    #[error("RPC disconnected: {0}")]
    RpcDisconnected(String),

    #[cfg(test)]
    #[error("RPC protocol error: {0}")]
    RpcProtocol(String),

    #[cfg(test)]
    #[error("RPC request timed out: {0}")]
    RpcTimeout(String),

    #[error("未知应用错误：{0}")]
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
                message: format!("文件系统操作失败：{error}"),
            },
            AppError::Database(_) => Self {
                code: "DATABASE_ERROR".to_owned(),
                message: "数据库操作失败。".to_owned(),
            },
            AppError::Migration(_) => Self {
                code: "DATABASE_ERROR".to_owned(),
                message: "数据库迁移失败。".to_owned(),
            },
            AppError::Serialization(error) => Self {
                code: "SERIALIZATION_ERROR".to_owned(),
                message: format!("响应序列化失败：{error}"),
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
            #[cfg(test)]
            AppError::CodexDetection(message) => Self {
                code: "CODEX_DETECTION_ERROR".to_owned(),
                message,
            },
            #[cfg(test)]
            AppError::CodexNotFound(message) => Self {
                code: "CODEX_NOT_FOUND".to_owned(),
                message,
            },
            #[cfg(test)]
            AppError::AppServerUnavailable(message) => Self {
                code: "APP_SERVER_UNAVAILABLE".to_owned(),
                message,
            },
            #[cfg(test)]
            AppError::AppServerStart(message) => Self {
                code: "APP_SERVER_START_ERROR".to_owned(),
                message,
            },
            #[cfg(test)]
            AppError::AppServerInitialization(message) => Self {
                code: "APP_SERVER_INITIALIZATION_ERROR".to_owned(),
                message,
            },
            #[cfg(test)]
            AppError::SchemaCompatibility(message) => Self {
                code: "SCHEMA_COMPATIBILITY_ERROR".to_owned(),
                message,
            },
            #[cfg(test)]
            AppError::SchemaGenerationUnavailable(message) => Self {
                code: "SCHEMA_GENERATION_UNAVAILABLE".to_owned(),
                message,
            },
            #[cfg(test)]
            AppError::AccountService(message) => Self {
                code: "ACCOUNT_SERVICE_ERROR".to_owned(),
                message,
            },
            AppError::Settings(message) => Self {
                code: "SETTINGS_ERROR".to_owned(),
                message,
            },
            #[cfg(test)]
            AppError::AppServerStop(message) => Self {
                code: "APP_SERVER_STOP_ERROR".to_owned(),
                message,
            },
            #[cfg(test)]
            AppError::RpcRemote(message) => Self {
                code: "RPC_REMOTE_ERROR".to_owned(),
                message,
            },
            #[cfg(test)]
            AppError::RpcDisconnected(message) => Self {
                code: "RPC_DISCONNECTED".to_owned(),
                message,
            },
            #[cfg(test)]
            AppError::RpcProtocol(message) => Self {
                code: "RPC_PROTOCOL_ERROR".to_owned(),
                message,
            },
            #[cfg(test)]
            AppError::RpcTimeout(message) => Self {
                code: "RPC_TIMEOUT".to_owned(),
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
