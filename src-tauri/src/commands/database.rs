use crate::{
    database::migrations,
    error::{AppError, CommandResult},
    models::system::DatabaseStatus,
    state::AppState,
};
use tauri::State;

#[tauri::command]
pub async fn database_status(state: State<'_, AppState>) -> CommandResult<DatabaseStatus> {
    let _: i64 = sqlx::query_scalar("SELECT 1")
        .fetch_one(&state.db_pool)
        .await
        .map_err(AppError::from)
        .map_err(crate::error::CommandError::from)?;

    let schema_version = migrations::get_schema_version(&state.db_pool)
        .await
        .map_err(crate::error::CommandError::from)?;

    Ok(DatabaseStatus {
        connected: true,
        path: state.database_path.to_string_lossy().into_owned(),
        schema_version,
    })
}
