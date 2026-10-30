//! More windows: a connection opened in a window of its own (TablePlus' "Open in New
//! Window", "Move Tab to New Window"). What the window shows reaches the page as
//! `window.__LIBRATABLE_WINDOW__`, set before the page's scripts run.

use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::json;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use super::mode::{WORKSPACE_DEFAULT, WORKSPACE_MIN};

static NEXT: AtomicU32 = AtomicU32::new(1);

/// The script that tells the page its connection and the tab (JSON text) to show.
pub fn init_script(connection: &str, tab: Option<&str>) -> String {
    let value = json!({ "connection": connection, "tab": tab });
    format!("window.__LIBRATABLE_WINDOW__ = {value};")
}

pub fn open(app: &AppHandle, connection: &str, tab: Option<&str>) -> tauri::Result<String> {
    let label = loop {
        let l = format!("win-{}", NEXT.fetch_add(1, Ordering::Relaxed));
        if app.get_webview_window(&l).is_none() {
            break l;
        }
    };
    let b = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .initialization_script(init_script(connection, tab))
        .title("LibraTable")
        .inner_size(WORKSPACE_DEFAULT.0, WORKSPACE_DEFAULT.1)
        .min_inner_size(WORKSPACE_MIN.0, WORKSPACE_MIN.1)
        .center()
        .disable_drag_drop_handler();
    #[cfg(target_os = "macos")]
    let b = b
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    b.build()?;
    Ok(label)
}

#[cfg(test)]
mod tests {
    #[test]
    fn hands_the_connection_and_tab_to_the_page() {
        assert_eq!(
            super::init_script("c-1", Some(r#"{"kind":"data"}"#)),
            r#"window.__LIBRATABLE_WINDOW__ = {"connection":"c-1","tab":"{\"kind\":\"data\"}"};"#
        );
    }
}
