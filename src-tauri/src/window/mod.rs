//! The main window. Closing it while a connection is on screen goes back to the
//! connections page instead of quitting, as closing a connection window does in
//! TablePlus. The UI reports which page is visible; on the connections page the window
//! closes (and the app quits). `mode` switches between the welcome and workspace shapes.

mod chrome;
pub mod mode;

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{Emitter, Window, WindowEvent};

/// True while a connection (not the connections page) is visible.
static IN_WORKSPACE: AtomicBool = AtomicBool::new(false);

pub fn set_in_workspace(visible: bool) {
    IN_WORKSPACE.store(visible, Ordering::Relaxed);
}

/// Window close (red button, ⌘W from the system): keep the window and tell the UI.
/// After resizes and focus changes, AppKit has moved the window buttons back: re-place
/// them.
pub fn on_event(window: &Window, event: &WindowEvent) {
    match event {
        WindowEvent::CloseRequested { api, .. } => {
            if IN_WORKSPACE.load(Ordering::Relaxed) {
                api.prevent_close();
                let _ = window.emit("close-requested", ());
            }
        }
        WindowEvent::Resized(_)
        | WindowEvent::Focused(_)
        | WindowEvent::ScaleFactorChanged { .. }
        | WindowEvent::ThemeChanged(_) => chrome::place(window, mode::buttons()),
        _ => {}
    }
}
