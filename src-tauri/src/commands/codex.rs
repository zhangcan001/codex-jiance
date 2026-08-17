use crate::{
    codex,
    error::CommandResult,
    models::codex::{AppServerStatusInfo, CodexInstallationInfo, SchemaCompatibilityReport},
    state::AppState,
};
use tauri::State;

#[tauri::command]
pub async fn detect_codex_environment() -> CommandResult<CodexInstallationInfo> {
    codex::detect().await.map_err(Into::into)
}

#[tauri::command]
pub async fn start_codex_app_server(
    state: State<'_, AppState>,
) -> CommandResult<AppServerStatusInfo> {
    state.app_server_manager.start().await.map_err(Into::into)
}

#[tauri::command]
pub async fn stop_codex_app_server(
    state: State<'_, AppState>,
) -> CommandResult<AppServerStatusInfo> {
    state.app_server_manager.stop().await.map_err(Into::into)
}

#[tauri::command]
pub async fn get_codex_app_server_status(
    state: State<'_, AppState>,
) -> CommandResult<AppServerStatusInfo> {
    state.app_server_manager.status().await.map_err(Into::into)
}

#[tauri::command]
pub async fn check_codex_schema_compatibility(
    state: State<'_, AppState>,
    force: bool,
) -> CommandResult<SchemaCompatibilityReport> {
    Ok(state.schema_compatibility_service.check(force).await)
}
