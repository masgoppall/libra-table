//! The traffic lights (close, minimize, zoom): where they sit and which are shown.
//! tao applies `trafficLightPosition` only when its own view redraws, which never
//! happens under the webview, and AppKit moves the buttons back on resizes and focus
//! changes, so the window module places them itself after those events.

use tauri::{Runtime, WebviewWindow, Window};

/// The close button's top-left corner, in points from the window's top-left corner.
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    pub left: f64,
    pub top: f64,
    /// From one button's left edge to the next one's.
    pub spacing: f64,
    /// Minimize and zoom (the welcome window shows only the close button).
    pub resize_buttons: bool,
}

/// What `place` needs from Tauri's two window types.
pub trait NativeWindow {
    fn ns_window_ptr(&self) -> Option<usize>;
    fn on_main(&self, f: Box<dyn FnOnce() + Send>);
}

impl<R: Runtime> NativeWindow for Window<R> {
    fn ns_window_ptr(&self) -> Option<usize> {
        self.ns_window().ok().map(|p| p as usize)
    }
    fn on_main(&self, f: Box<dyn FnOnce() + Send>) {
        let _ = self.run_on_main_thread(f);
    }
}

impl<R: Runtime> NativeWindow for WebviewWindow<R> {
    fn ns_window_ptr(&self) -> Option<usize> {
        self.ns_window().ok().map(|p| p as usize)
    }
    fn on_main(&self, f: Box<dyn FnOnce() + Send>) {
        let _ = self.run_on_main_thread(f);
    }
}

#[cfg(target_os = "macos")]
pub fn place(w: &impl NativeWindow, layout: Layout) {
    let Some(ptr) = w.ns_window_ptr() else { return };
    w.on_main(Box::new(move || {
        // SAFETY: `ptr` is this window's NSWindow, which outlives the call, and AppKit is
        // only touched here on the main thread.
        let window = unsafe { &*(ptr as *const objc2_app_kit::NSWindow) };
        macos::place(window, layout);
    }));
}

#[cfg(not(target_os = "macos"))]
pub fn place(_w: &impl NativeWindow, _layout: Layout) {}

#[cfg(target_os = "macos")]
mod macos {
    use objc2_app_kit::{NSWindow, NSWindowButton};
    use objc2_foundation::{NSPoint, NSRect};

    use super::Layout;

    pub fn place(window: &NSWindow, layout: Layout) {
        let button = |kind| window.standardWindowButton(kind);
        let (Some(close), Some(mini), Some(zoom)) = (
            button(NSWindowButton::CloseButton),
            button(NSWindowButton::MiniaturizeButton),
            button(NSWindowButton::ZoomButton),
        ) else {
            return;
        };
        let size = close.frame().size;
        let height = window.frame().size.height;
        // The title bar holds the buttons: make it tall enough to center them.
        // SAFETY: the buttons always sit in the title bar view inside its container.
        if let Some(bar) = unsafe { close.superview().and_then(|v| v.superview()) } {
            let mut frame = bar.frame();
            frame.size.height = layout.top * 2.0 + size.height;
            frame.origin.y = height - frame.size.height;
            bar.setFrame(frame);
        }
        for (i, b) in [&close, &mini, &zoom].into_iter().enumerate() {
            let x = layout.left + i as f64 * layout.spacing;
            let target = NSRect::new(NSPoint::new(x, height - layout.top - size.height), size);
            // SAFETY: as above, every button has a superview.
            if let Some(parent) = unsafe { b.superview() } {
                b.setFrameOrigin(parent.convertRect_fromView(target, None).origin);
            }
        }
        mini.setHidden(!layout.resize_buttons);
        zoom.setHidden(!layout.resize_buttons);
    }
}
