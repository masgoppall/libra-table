//! Closing the window while a connection is on screen goes back to the connections page
//! instead of quitting, as closing a connection window does in TablePlus. The UI reports
//! which page is visible; on the connections page the window closes (and the app quits).

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{Emitter, Window, WindowEvent};

/// True while a connection (not the connections page) is visible.
static IN_WORKSPACE: AtomicBool = AtomicBool::new(false);

pub fn set_in_workspace(visible: bool) {
    IN_WORKSPACE.store(visible, Ordering::Relaxed);
}

/// Window close (red button, ⌘W from the system): keep the window and tell the UI.
pub fn on_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        if IN_WORKSPACE.load(Ordering::Relaxed) {
            api.prevent_close();
            let _ = window.emit("close-requested", ());
        }
    }
}
