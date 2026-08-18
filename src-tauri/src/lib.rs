#[cfg(test)]
mod account;
mod alerts;
mod burn_rate;
#[cfg(test)]
mod codex;
mod commands;
mod database;
mod desktop;
mod error;
mod history;
mod model_usage;
mod models;
mod prediction;
pub mod pricing;
mod project;
mod rate_limit;
mod settings;
mod state;
#[cfg(test)]
mod thread_usage;
mod time;
mod tray;
mod usage;

use error::AppError;
use state::AppState;
use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_autostart::{init as init_autostart, MacosLauncher};
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
        .plugin(tauri_plugin_notification::init())
        .plugin(init_autostart(MacosLauncher::LaunchAgent, None))
        .setup(|app| {
            log::info!("Application starting");

            let app_data_dir = app.path().app_data_dir().map_err(|error| {
                Box::new(AppError::Tauri(error.to_string())) as Box<dyn std::error::Error>
            })?;

            let database = tauri::async_runtime::block_on(database::initialize(&app_data_dir))
                .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;

            let autostart = Arc::new(settings::TauriAutostartBackend::new(app.handle().clone()));
            let settings_service = tauri::async_runtime::block_on(
                settings::SettingsService::initialize(database.pool.clone(), autostart),
            )
            .map_err(|error| Box::new(error) as Box<dyn std::error::Error>)?;
            app.manage(AppState::from_database(
                database,
                app.handle().clone(),
                settings_service,
            ));
            app.state::<AppState>().alert_service.start();
            app.state::<AppState>().desktop_service.start();
            tray::setup(app)?;
            log::info!("Application ready");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::get_app_info,
            commands::system::health_check,
            commands::database::database_status,
            commands::settings::get_app_settings,
            commands::settings::update_app_settings,
            commands::codex::get_desktop_environment,
            commands::codex::get_desktop_monitor_status,
            commands::codex::refresh_desktop_index,
            commands::codex::get_desktop_activity,
            commands::codex::get_codex_rate_limits,
            commands::codex::get_codex_burn_rates,
            commands::codex::get_codex_quota_predictions,
            commands::codex::get_alert_status,
            commands::codex::request_alert_notification_permission,
            commands::codex::get_codex_usage,
            commands::codex::get_thread_usage_status,
            commands::codex::get_project_usage,
            commands::codex::get_model_usage,
            commands::codex::get_monitoring_history
        ])
        .on_window_event(|window, event| {
            if window.label() != tray::MAIN_WINDOW_LABEL {
                return;
            }

            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let state = window.app_handle().state::<AppState>();
                if state.settings_service.close_to_tray() {
                    if let Err(error) = window.hide() {
                        log::error!("Failed to hide main window to tray: {error}");
                    }
                } else {
                    window.app_handle().exit(0);
                }
            }
        })
        .build(tauri::generate_context!())?;

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            log::info!("Application exit received; cleaning up Desktop monitoring services");
            let state = app_handle.state::<AppState>();
            tauri::async_runtime::block_on(state.alert_service.shutdown());
            tauri::async_runtime::block_on(state.desktop_service.shutdown());
            tauri::async_runtime::block_on(state.rate_limit_service.shutdown());
        }
    });

    Ok(())
}
