//! The window's two shapes: the compact welcome window of the connections page (like
//! TablePlus' welcome window: fixed size, centered, close button only) and the large,
//! resizable workspace window, whose size is remembered while the app runs.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::Deserialize;
use tauri::{LogicalSize, Runtime, WebviewWindow};

use super::chrome::{self, Layout};

/// The welcome window, in points (TablePlus' size).
pub const WELCOME: (f64, f64) = (680.0, 608.0);
const WORKSPACE_DEFAULT: (f64, f64) = (1280.0, 820.0);
const WORKSPACE_MIN: (f64, f64) = (960.0, 600.0);

/// The close button as TablePlus places it in its welcome window.
const WELCOME_BUTTONS: Layout = Layout {
    left: 18.0,
    top: 18.0,
    spacing: 23.0,
    resize_buttons: false,
};
/// Centered in the workspace's 52px toolbar, spaced as in TablePlus' main window.
const WORKSPACE_BUTTONS: Layout = Layout {
    left: 16.0,
    top: 19.0,
    spacing: 23.0,
    resize_buttons: true,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Welcome,
    Workspace,
}

/// The app starts in the welcome window (tauri.conf.json has its size).
static WELCOME_SHOWN: AtomicBool = AtomicBool::new(true);
/// The workspace window's last size, restored when it opens again.
static WORKSPACE_SIZE: Mutex<Option<LogicalSize<f64>>> = Mutex::new(None);

/// Where the window buttons go in the current mode.
pub fn buttons() -> Layout {
    if WELCOME_SHOWN.load(Ordering::SeqCst) {
        WELCOME_BUTTONS
    } else {
        WORKSPACE_BUTTONS
    }
}

/// Switches the window to `mode`. Already in it, only the buttons are placed again (the
/// UI asks once the window is on screen, after AppKit's first layout).
pub fn apply<R: Runtime>(w: &WebviewWindow<R>, mode: Mode) -> tauri::Result<()> {
    let welcome_mode = mode == Mode::Welcome;
    if WELCOME_SHOWN.swap(welcome_mode, Ordering::SeqCst) == welcome_mode {
        chrome::place(w, buttons());
        return Ok(());
    }
    if welcome_mode {
        if w.is_fullscreen()? {
            w.set_fullscreen(false)?;
        }
        if w.is_maximized()? {
            w.unmaximize()?;
        }
        let size = w.inner_size()?.to_logical(w.scale_factor()?);
        *WORKSPACE_SIZE.lock().unwrap() = Some(size);
        return welcome(w);
    }
    let (min_w, min_h) = WORKSPACE_MIN;
    let (w0, h0) = WORKSPACE_DEFAULT;
    let size = WORKSPACE_SIZE
        .lock()
        .unwrap()
        .unwrap_or(LogicalSize::new(w0, h0));
    w.set_resizable(true)?;
    w.set_maximizable(true)?;
    w.set_min_size(Some(LogicalSize::new(min_w, min_h)))?;
    resize_centered(w, size)?;
    chrome::place(w, WORKSPACE_BUTTONS);
    Ok(())
}

/// Shapes the window as the welcome window (also at startup, for its buttons).
pub fn welcome<R: Runtime>(w: &WebviewWindow<R>) -> tauri::Result<()> {
    let (width, height) = WELCOME;
    w.set_min_size(None::<LogicalSize<f64>>)?;
    resize_centered(w, LogicalSize::new(width, height))?;
    w.set_resizable(false)?;
    w.set_maximizable(false)?;
    chrome::place(w, WELCOME_BUTTONS);
    Ok(())
}

/// Gives the window `size` (its content, in points) centered on its screen in one step.
/// A resize followed by a move showed the window growing or shrinking from its top-left
/// corner first, so it jumped sideways before settling in the middle.
#[cfg(target_os = "macos")]
fn resize_centered<R: Runtime>(w: &WebviewWindow<R>, size: LogicalSize<f64>) -> tauri::Result<()> {
    let Ok(ptr) = w.ns_window().map(|p| p as usize) else {
        w.set_size(size)?;
        return w.center();
    };
    w.run_on_main_thread(move || {
        // SAFETY: `ptr` is this window's NSWindow, which outlives the call, and AppKit is
        // only touched here on the main thread.
        let window = unsafe { &*(ptr as *const objc2_app_kit::NSWindow) };
        macos::set_frame_centered(window, size.width, size.height);
    })
}

#[cfg(not(target_os = "macos"))]
fn resize_centered<R: Runtime>(w: &WebviewWindow<R>, size: LogicalSize<f64>) -> tauri::Result<()> {
    w.set_size(size)?;
    w.center()
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2_app_kit::NSWindow;
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    /// The frame for a `width` × `height` content area, centered in the visible part of the
    /// window's screen (not under the menu bar or the Dock) and no larger than it.
    pub fn set_frame_centered(window: &NSWindow, width: f64, height: f64) {
        let content = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));
        let mut frame = window.frameRectForContentRect(content);
        match window.screen() {
            Some(screen) => {
                let area = screen.visibleFrame();
                frame.size.width = frame.size.width.min(area.size.width);
                frame.size.height = frame.size.height.min(area.size.height);
                frame.origin.x =
                    area.origin.x + ((area.size.width - frame.size.width) / 2.0).round();
                frame.origin.y =
                    area.origin.y + ((area.size.height - frame.size.height) / 2.0).round();
            }
            // Off every screen: keep the top-left corner where it is.
            None => {
                let old = window.frame();
                frame.origin.x = old.origin.x;
                frame.origin.y = old.origin.y + old.size.height - frame.size.height;
            }
        }
        window.setFrame_display(frame, true);
    }
}
