// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = codex_usage_monitor_lib::run() {
        eprintln!("Application failed to start: {error}");
        std::process::exit(1);
    }
}
