use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Runtime};

pub(crate) const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "codex-usage-monitor-tray";
const OPEN_DASHBOARD_ID: &str = "open-dashboard";
const HIDE_WINDOW_ID: &str = "hide-window";
const QUIT_ID: &str = "quit";

pub(crate) fn setup<R: Runtime>(app: &App<R>) -> tauri::Result<()> {
    let open_dashboard =
        MenuItem::with_id(app, OPEN_DASHBOARD_ID, "打开主界面", true, None::<&str>)?;
    let hide_window = MenuItem::with_id(app, HIDE_WINDOW_ID, "隐藏窗口", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, QUIT_ID, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_dashboard, &hide_window, &separator, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Codex 用量监控器")
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(handle_tray_icon_event);

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder.build(app)?;
    Ok(())
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        OPEN_DASHBOARD_ID => show_main_window(app),
        HIDE_WINDOW_ID => hide_main_window(app),
        QUIT_ID => app.exit(0),
        _ => {}
    }
}

fn handle_tray_icon_event<R: Runtime>(_tray: &tauri::tray::TrayIcon<R>, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        // The event callback is attached to the tray builder, so the app handle is
        // recovered from the tray resource before touching the window.
        show_main_window(_tray.app_handle());
    }
}

fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        log::error!("Main window is unavailable while opening Dashboard");
        return;
    };

    if let Err(error) = window.unminimize() {
        log::error!("Failed to unminimize main window: {error}");
    }
    if let Err(error) = window.show() {
        log::error!("Failed to show main window: {error}");
    }
    if let Err(error) = window.set_focus() {
        log::error!("Failed to focus main window: {error}");
    }
}

fn hide_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        if let Err(error) = window.hide() {
            log::error!("Failed to hide main window: {error}");
        }
    } else {
        log::error!("Main window is unavailable while hiding Dashboard");
    }
}
