mod account;
mod codex;
mod commands;
mod database;
mod error;
mod models;
pub mod pricing;
mod rate_limit;
mod state;
mod usage;

use error::AppError;
use state::AppState;
use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind, TimezoneStrategy};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> Result<(), tauri::Error> {
    let app = tauri::Builder::default()
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
            commands::database::database_status,
            commands::codex::detect_codex_environment,
            commands::codex::start_codex_app_server,
            commands::codex::stop_codex_app_server,
            commands::codex::get_codex_app_server_status,
            commands::codex::check_codex_schema_compatibility,
            commands::codex::get_codex_account,
            commands::codex::get_codex_rate_limits,
            commands::codex::get_codex_usage
        ])
        .build(tauri::generate_context!())?;

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            log::info!(
                "Application exit received; cleaning up Usage, Rate Limit, Account, and App Server"
            );
            let state = app_handle.state::<AppState>();
            tauri::async_runtime::block_on(state.usage_service.shutdown());
            tauri::async_runtime::block_on(state.rate_limit_service.shutdown());
            tauri::async_runtime::block_on(state.account_service.shutdown());
            if let Err(error) = tauri::async_runtime::block_on(state.app_server_manager.shutdown())
            {
                log::error!("App Server cleanup on application exit failed: {error}");
            }
        }
    });

    Ok(())
}
