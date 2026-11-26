fn main() {
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "ping",
            "search_tracks",
            "prefetch_cover",
            "prepare_selection",
            "discard_preparation",
            "commit_selection",
            "admin_unlock",
            "admin_lock",
            "admin_status",
            "auth_start",
            "auth_logout",
            "get_public_config",
            "get_config",
            "set_config",
            "list_printers",
            "test_print",
            "list_recent",
            "reprint",
            "export_csv",
            "clear_history",
            "mark_refilled",
            "reset_printer_counters",
            "set_maintenance",
            "exit_kiosk",
        ]));
    if let Err(error) = tauri_build::try_build(attributes) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
