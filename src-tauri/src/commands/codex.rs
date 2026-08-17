use crate::{codex, error::CommandResult, models::codex::CodexInstallationInfo};

#[tauri::command]
pub async fn detect_codex_environment() -> CommandResult<CodexInstallationInfo> {
    codex::detect().await.map_err(Into::into)
}
