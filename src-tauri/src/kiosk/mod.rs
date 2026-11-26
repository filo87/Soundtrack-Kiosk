use tauri::menu::{Menu, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, Position, Size, Window};

mod macos;

pub fn setup(app: &mut tauri::App) -> tauri::Result<()> {
    install_menu(app)?;
    let Some(webview) = app.get_webview_window("main") else {
        return Ok(());
    };
    let window = webview.as_ref().window();
    if let Some(monitor) = window.primary_monitor()? {
        let _ = window.set_position(Position::Physical(*monitor.position()));
        let _ = window.set_size(Size::Physical(*monitor.size()));
    }
    let _ = window.set_simple_fullscreen(true);
    let _ = window.set_always_on_top(true);
    window.show()?;
    window.set_focus()?;
    let handle = app.handle().clone();
    let _ = handle.run_on_main_thread(macos::set_kiosk_options);
    Ok(())
}

fn install_menu(app: &tauri::App) -> tauri::Result<()> {
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let menu = Menu::with_items(app, &[&edit])?;
    app.set_menu(menu)?;
    Ok(())
}

pub fn set_maintenance(app: &AppHandle, on: bool) {
    let Some(webview) = app.get_webview_window("main") else {
        return;
    };
    let window = webview.as_ref().window();
    if on {
        let _ = window.set_always_on_top(false);
        let _ = window.set_simple_fullscreen(false);
        let handle = app.clone();
        let _ = handle.run_on_main_thread(macos::clear_options);
        let _ = window.show();
    } else {
        let _ = window.set_always_on_top(true);
        reassert(app);
    }
}

pub fn reassert(app: &AppHandle) {
    let Some(webview) = app.get_webview_window("main") else {
        return;
    };
    let window = webview.as_ref().window();
    reassert_window(app, &window);
}

pub fn reassert_window(app: &AppHandle, window: &Window) {
    let state = app.state::<crate::state::AppState>();
    if state.is_maintenance() {
        return;
    }
    if bounds_differ(window) {
        if let Ok(Some(monitor)) = window.primary_monitor() {
            let _ = window.set_position(Position::Physical(*monitor.position()));
            let _ = window.set_size(Size::Physical(*monitor.size()));
        }
    }
    let _ = window.set_simple_fullscreen(true);
    let _ = window.set_always_on_top(true);
    let handle = app.clone();
    let _ = handle.run_on_main_thread(macos::set_kiosk_options);
}

fn bounds_differ(window: &Window) -> bool {
    let Ok(Some(monitor)) = window.primary_monitor() else {
        return false;
    };
    let position_matches = window
        .outer_position()
        .map(|position| position == *monitor.position())
        .unwrap_or(false);
    let size_matches = window
        .outer_size()
        .map(|size| size == *monitor.size())
        .unwrap_or(false);
    !(position_matches && size_matches)
}
