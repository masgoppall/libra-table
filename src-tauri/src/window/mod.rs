//! The main window. Closing it while a connection is on screen goes back to the
//! connections page instead of quitting, as closing a connection window does in
//! TablePlus. The UI reports which page is visible; on the connections page the window
//! closes (and the app quits). `mode` switches between the welcome and workspace shapes.

mod chrome;
pub mod holders;
pub mod mode;
pub mod windows;

/// The app's first window; others show one connection each.
pub const MAIN: &str = "main";

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{Emitter, Manager, Window, WindowEvent};

/// True while a connection (not the connections page) is visible.
static IN_WORKSPACE: AtomicBool = AtomicBool::new(false);

pub fn set_in_workspace(visible: bool) {
    IN_WORKSPACE.store(visible, Ordering::Relaxed);
}

/// Window close (red button, ⌘W from the system): keep the window and tell the UI.
/// After resizes and focus changes, AppKit has moved the window buttons back: re-place
/// them.
pub fn on_event(window: &Window, event: &WindowEvent) {
    if window.label() != MAIN {
        return other_window_event(window, event);
    }
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

/// A connection window closes like any window; its connections close with it unless
/// another window still shows them.
fn other_window_event(window: &Window, event: &WindowEvent) {
    match event {
        WindowEvent::Destroyed => {
            let orphaned = holders::release_window(window.label());
            let app = window.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = app.state::<crate::state::AppState>();
                for id in orphaned {
                    crate::connections::disconnect(&state, &id).await;
                }
            });
        }
        WindowEvent::Resized(_)
        | WindowEvent::Focused(_)
        | WindowEvent::ScaleFactorChanged { .. }
        | WindowEvent::ThemeChanged(_) => chrome::place(window, mode::buttons_for_workspace()),
        _ => {}
    }
}
