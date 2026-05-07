//! System tray icon + menu. Mirrors Laragon's silent-runner UX: clicking
//! the close button hides the window to the tray instead of exiting,
//! tray click toggles visibility, tray menu has explicit Show / Hide /
//! Quit, and we can rebuild the menu later when service status changes.

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Window, WindowEvent};

const MAIN_WINDOW: &str = "main";

pub fn install(app: &App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "tray_show", "Show Stackpilot", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "tray_hide", "Hide window", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "tray_quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show, &hide, &sep, &quit])?;

    let _tray = TrayIconBuilder::with_id("stackpilot-main")
        .tooltip("Stackpilot")
        .icon(app.default_window_icon().cloned().unwrap_or_else(|| {
            // Default icon is bundled with every Tauri build; this branch
            // shouldn't fire in practice.
            tauri::image::Image::new_owned(vec![0; 4], 1, 1)
        }))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(handle_tray_event)
        .build(app)?;

    Ok(())
}

fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id.as_ref() {
        "tray_show" => show_window(app),
        "tray_hide" => hide_window(app),
        "tray_quit" => app.exit(0),
        _ => {}
    }
}

fn handle_tray_event(tray: &tauri::tray::TrayIcon, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        toggle_window(tray.app_handle());
    }
}

fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn hide_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.hide();
    }
}

fn toggle_window(app: &AppHandle) {
    let Some(w) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    let visible = w.is_visible().unwrap_or(false);
    if visible {
        let _ = w.hide();
    } else {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Window event handler: when the user clicks the X and `close_to_tray` is
/// on, prevent the actual close and hide the window instead.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        let state = window.state::<crate::state::AppState>();
        if state
            .close_to_tray
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            api.prevent_close();
            let _ = window.hide();
        }
    }
}
