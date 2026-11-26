#[cfg(target_os = "macos")]
pub fn set_kiosk_options() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationPresentationOptions};

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let options = NSApplicationPresentationOptions::HideDock
        | NSApplicationPresentationOptions::HideMenuBar
        | NSApplicationPresentationOptions::DisableAppleMenu
        | NSApplicationPresentationOptions::DisableProcessSwitching
        | NSApplicationPresentationOptions::DisableHideApplication
        | NSApplicationPresentationOptions::DisableMenuBarTransparency;
    app.setPresentationOptions(options);
}

#[cfg(target_os = "macos")]
pub fn clear_options() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationPresentationOptions};

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    app.setPresentationOptions(NSApplicationPresentationOptions::empty());
}

#[cfg(not(target_os = "macos"))]
pub fn set_kiosk_options() {}

#[cfg(not(target_os = "macos"))]
pub fn clear_options() {}
