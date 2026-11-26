mod admin;
mod commands;
mod config;
mod cover;
mod error;
mod health;
mod journal;
mod kiosk;
mod outbox;
mod paths;
mod print;
mod render;
mod spotify;
mod state;

use tauri::{Manager, RunEvent, WindowEvent};

use crate::paths::AppPaths;
use crate::state::AppState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            match crate::paths::migrate_legacy_data(&data_dir) {
                Ok(true) => log::info!("migrated legacy Baby Playlist Kiosk data"),
                Ok(false) => {}
                Err(err) => log::warn!("legacy data migration failed: {err}"),
            }
            let paths = AppPaths::new(data_dir)?;
            paths.purge_previews()?;
            let config = config::load_config(app.handle())?;
            let state = AppState::new(app.handle().clone(), paths, config)?;
            app.manage(state);
            kiosk::setup(app)?;
            outbox::spawn(app.handle().clone());
            health::spawn(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            let app = window.app_handle();
            let state = app.state::<AppState>();
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    if !state.exit_allowed() {
                        api.prevent_close();
                    }
                }
                WindowEvent::Focused(true)
                | WindowEvent::Moved(_)
                | WindowEvent::Resized(_)
                | WindowEvent::ScaleFactorChanged { .. } => {
                    kiosk::reassert_window(app, window);
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::search_tracks,
            commands::prefetch_cover,
            commands::prepare_selection,
            commands::discard_preparation,
            commands::commit_selection,
            commands::admin_unlock,
            commands::admin_lock,
            commands::admin_status,
            commands::auth_start,
            commands::auth_logout,
            commands::get_public_config,
            commands::get_config,
            commands::set_config,
            commands::list_printers,
            commands::test_print,
            commands::list_recent,
            commands::reprint,
            commands::export_csv,
            commands::clear_history,
            commands::mark_refilled,
            commands::reset_printer_counters,
            commands::set_maintenance,
            commands::exit_kiosk,
        ])
        .build(tauri::generate_context!())
        .expect("error while building the kiosk application")
        .run(|app, event| {
            if let RunEvent::ExitRequested { api, .. } = event {
                let state = app.state::<AppState>();
                if !state.exit_allowed() {
                    api.prevent_exit();
                }
            }
        });
}
