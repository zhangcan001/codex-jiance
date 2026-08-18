use crate::{
    error::{AppError, CommandResult},
    models::system::{AppInfo, HealthStatus},
    state::AppState,
};
use tauri::State;

#[tauri::command]
pub fn get_app_info() -> CommandResult<AppInfo> {
    Ok(AppInfo {
        name: "Codex 用量监控器".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        environment: if cfg!(debug_assertions) {
            "development".to_owned()
        } else {
            "production".to_owned()
        },
    })
}

#[tauri::command]
pub async fn health_check(state: State<'_, AppState>) -> CommandResult<HealthStatus> {
    let _: i64 = sqlx::query_scalar("SELECT 1")
        .fetch_one(&state.db_pool)
        .await
        .map_err(AppError::from)
        .map_err(crate::error::CommandError::from)?;

    Ok(HealthStatus {
        status: "ok".to_owned(),
        database: "connected".to_owned(),
    })
}
