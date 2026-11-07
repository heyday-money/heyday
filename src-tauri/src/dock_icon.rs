//! Runtime macOS Dock appearance. The installed bundle icon stays unchanged.
use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSImage};
use objc2_foundation::NSData;
use tauri::{AppHandle, Theme};

const LIGHT_ICON: &[u8] = include_bytes!("../icons/icon.png");
const DARK_ICON: &[u8] = include_bytes!("../icons/icon-dark.png");

pub fn update(app: &AppHandle, theme: Theme) {
    // AppKit objects must be created and used on the main thread. Theme events
    // come from the native window; frontend appearance preferences are separate.
    if let Err(error) = app.run_on_main_thread(move || {
        let Some(main_thread) = MainThreadMarker::new() else {
            return;
        };
        let bytes = if theme == Theme::Dark { DARK_ICON } else { LIGHT_ICON };
        let data = NSData::with_bytes(bytes);
        let Some(icon) = NSImage::initWithData(NSImage::alloc(), &data) else {
            eprintln!("Could not decode the bundled Dock icon");
            return;
        };
        let application = NSApplication::sharedApplication(main_thread);
        // SAFETY: A valid, non-null image is supplied on the AppKit main thread.
        unsafe { application.setApplicationIconImage(Some(&icon)) };
    }) {
        eprintln!("Could not update Dock icon: {error}");
    }
}
