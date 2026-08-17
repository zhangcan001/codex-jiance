use crate::{
    account::CodexAccountInfo,
    alerts::AlertServiceStatus,
    burn_rate::BurnRateEstimate,
    codex,
    error::CommandResult,
    history::MonitoringHistory,
    model_usage::ModelUsageReport,
    models::codex::{AppServerStatusInfo, CodexInstallationInfo, SchemaCompatibilityReport},
    prediction::QuotaPrediction,
    project::ProjectUsageReport,
    rate_limit::RateLimitInfo,
    state::AppState,
    thread_usage::ThreadUsageInfo,
    usage::CodexUsageInfo,
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

#[tauri::command]
pub async fn get_codex_account(
    state: State<'_, AppState>,
    force: bool,
) -> CommandResult<CodexAccountInfo> {
    Ok(state.account_service.get_account(force).await)
}

#[tauri::command]
pub async fn get_codex_rate_limits(
    state: State<'_, AppState>,
    force: bool,
) -> CommandResult<RateLimitInfo> {
    Ok(state.rate_limit_service.get_rate_limits(force).await)
}

#[tauri::command]
pub async fn get_codex_burn_rates(
    state: State<'_, AppState>,
    force: bool,
) -> CommandResult<Vec<BurnRateEstimate>> {
    Ok(state.burn_rate_service.get_burn_rates(force).await)
}

#[tauri::command]
pub async fn get_codex_quota_predictions(
    state: State<'_, AppState>,
    force: bool,
) -> CommandResult<Vec<QuotaPrediction>> {
    Ok(state.quota_prediction_service.get_predictions(force).await)
}

#[tauri::command]
pub async fn get_alert_status(state: State<'_, AppState>) -> CommandResult<AlertServiceStatus> {
    Ok(state.alert_service.status())
}

#[tauri::command]
pub async fn request_alert_notification_permission(
    state: State<'_, AppState>,
) -> CommandResult<AlertServiceStatus> {
    Ok(state.alert_service.request_notification_permission().await)
}

#[tauri::command]
pub async fn get_codex_usage(
    state: State<'_, AppState>,
    force: bool,
) -> CommandResult<CodexUsageInfo> {
    Ok(state.usage_service.get_usage(force).await)
}

#[tauri::command]
pub async fn get_thread_usage_status(
    state: State<'_, AppState>,
    force_inventory: bool,
) -> CommandResult<ThreadUsageInfo> {
    Ok(state.thread_usage_service.get_status(force_inventory).await)
}

#[tauri::command]
pub async fn get_project_usage(
    state: State<'_, AppState>,
    start_at: Option<i64>,
    end_at: Option<i64>,
) -> CommandResult<ProjectUsageReport> {
    state
        .project_service
        .get_usage(start_at, end_at)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn get_model_usage(
    state: State<'_, AppState>,
    start_at: Option<i64>,
    end_at: Option<i64>,
) -> CommandResult<ModelUsageReport> {
    state
        .model_usage_service
        .get_usage(start_at, end_at)
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn get_monitoring_history(
    state: State<'_, AppState>,
    start_at: Option<i64>,
    end_at: Option<i64>,
) -> CommandResult<MonitoringHistory> {
    state
        .history_service
        .get_history(start_at, end_at)
        .await
        .map_err(Into::into)
}
