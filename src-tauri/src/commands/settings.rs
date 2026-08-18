use crate::{error::CommandResult, settings::AppSettings, state::AppState};
use tauri::State;

#[tauri::command]
pub fn get_app_settings(
    state: State<'_, AppState>,
) -> CommandResult<crate::settings::AppSettingsSnapshot> {
    Ok(state.settings_service.snapshot())
}

#[tauri::command]
pub async fn update_app_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> CommandResult<crate::settings::AppSettingsSnapshot> {
    state
        .settings_service
        .update(settings)
        .await
        .map_err(Into::into)
}
