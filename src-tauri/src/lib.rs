mod commands;
mod database;
mod error;
mod models;
mod state;

use error::AppError;
use state::AppState;
use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind, TimezoneStrategy};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir {
                        file_name: Some("codex-usage-monitor".to_owned()),
                    }),
                ])
                .level(log::LevelFilter::Info)
                .timezone_strategy(TimezoneStrategy::UseLocal)
                .build(),
        )
        .setup(|app| {
            log::info!("Application starting");

            let app_data_dir = app.path().app_data_dir().map_err(|error| {
                Box::new(AppError::Tauri(error.to_string())) as Box<dyn std::error::Error>
            })?;

            let database = tauri::async_runtime::block_on(database::initialize(&app_data_dir))
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;

            app.manage(AppState::from_database(database));
            log::info!("Application ready");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::get_app_info,
            commands::system::health_check,
            commands::database::database_status
        ])
        .run(tauri::generate_context!())
}
