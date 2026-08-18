use crate::{
    alerts::AlertServiceStatus,
    burn_rate::BurnRateEstimate,
    desktop::{
        DesktopEnvironmentInfo, DesktopMonitorStatus, DesktopThreadUsageInfo, DesktopUsageActivity,
    },
    error::CommandResult,
    history::MonitoringHistory,
    model_usage::ModelUsageReport,
    prediction::QuotaPrediction,
    project::ProjectUsageReport,
    rate_limit::RateLimitInfo,
    state::AppState,
    usage::CodexUsageInfo,
};
use tauri::State;

#[tauri::command]
pub async fn get_desktop_environment(
    state: State<'_, AppState>,
) -> CommandResult<DesktopEnvironmentInfo> {
    Ok(state.desktop_service.environment().await)
}

#[tauri::command]
pub async fn get_desktop_monitor_status(
    state: State<'_, AppState>,
) -> CommandResult<DesktopMonitorStatus> {
    Ok(state.desktop_service.status().await)
}

#[tauri::command]
pub async fn refresh_desktop_index(
    state: State<'_, AppState>,
) -> CommandResult<DesktopMonitorStatus> {
    state.desktop_service.refresh().await.map_err(Into::into)
}

#[tauri::command]
pub async fn get_desktop_activity(
    state: State<'_, AppState>,
) -> CommandResult<DesktopUsageActivity> {
    state.desktop_service.activity().await.map_err(Into::into)
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
    _force: bool,
) -> CommandResult<CodexUsageInfo> {
    Ok(state.desktop_service.usage().await)
}

#[tauri::command]
pub async fn get_thread_usage_status(
    state: State<'_, AppState>,
    _force_inventory: bool,
) -> CommandResult<DesktopThreadUsageInfo> {
    Ok(state.desktop_service.thread_usage().await)
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
