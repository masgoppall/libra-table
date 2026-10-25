//! The native app menu. Custom items carry an action id that the UI handles (`menu`
//! event); the UI also handles the same keys itself, so the accelerators here are a
//! fallback and a place to discover them. There is deliberately no "Close Window"
//! item: ⌘W closes a tab.

use std::collections::BTreeMap;
use std::sync::RwLock;

use tauri::menu::{Menu, MenuBuilder, MenuItem, PredefinedMenuItem, Submenu, SubmenuBuilder};
use tauri::{AppHandle, Wry};

/// Custom shortcuts from Settings: action id → keys in the UI's notation ("Mod+K").
static OVERRIDES: RwLock<BTreeMap<String, String>> = RwLock::new(BTreeMap::new());

/// One custom item: action id, label, accelerator.
type Item = (&'static str, &'static str, Option<&'static str>);

/// A group of items; groups are separated by a line.
type Section = &'static [Item];

const FILE: &[Section] = &[
    &[
        ("new-connection", "New Connection…", Some("CmdOrCtrl+N")),
        ("new-query", "New Query Tab", Some("CmdOrCtrl+T")),
        ("new-table", "New Table…", Some("CmdOrCtrl+Shift+N")),
        ("new-database", "New Database…", None),
    ],
    &[
        ("save", "Save", Some("CmdOrCtrl+S")),
        ("export", "Export…", Some("CmdOrCtrl+Shift+E")),
        ("export-tables", "Export Tables…", None),
    ],
    &[
        ("import-data", "Import CSV or JSON…", None),
        ("import-new-table", "Import into New Table…", None),
        ("import-sql", "Run SQL File…", None),
    ],
    &[
        ("close-tab", "Close Tab", Some("CmdOrCtrl+W")),
        ("disconnect", "Disconnect", Some("CmdOrCtrl+Shift+W")),
    ],
];

const VIEW: &[Section] = &[
    &[
        ("refresh", "Refresh", Some("CmdOrCtrl+R")),
        ("toggle-sidebar", "Toggle Sidebar", Some("CmdOrCtrl+\\")),
        (
            "toggle-detail",
            "Toggle Row Detail",
            Some("CmdOrCtrl+Shift+D"),
        ),
        (
            "toggle-console",
            "Toggle Console Log",
            Some("CmdOrCtrl+Shift+C"),
        ),
    ],
    &[
        ("font-bigger", "Larger Text", Some("CmdOrCtrl+=")),
        ("font-smaller", "Smaller Text", Some("CmdOrCtrl+-")),
        ("font-reset", "Actual Size", Some("CmdOrCtrl+0")),
    ],
];

const DATA: &[Section] = &[
    &[
        ("filter", "Filter", Some("CmdOrCtrl+F")),
        ("add-row", "Add Row", Some("CmdOrCtrl+I")),
        ("discard", "Discard Changes", None),
    ],
    &[
        ("prev-page", "Previous Page", Some("CmdOrCtrl+[")),
        ("next-page", "Next Page", Some("CmdOrCtrl+]")),
    ],
    &[(
        "open-structure",
        "Show Structure",
        Some("CmdOrCtrl+Shift+S"),
    )],
];

const QUERY: &[Section] = &[
    &[
        (
            "run-current",
            "Run Current Statement",
            Some("CmdOrCtrl+Enter"),
        ),
        ("run-all", "Run All", Some("CmdOrCtrl+Shift+Enter")),
        ("cancel-query", "Cancel Query", Some("CmdOrCtrl+.")),
    ],
    &[
        ("explain", "Explain Statement", Some("CmdOrCtrl+Alt+E")),
        ("format-sql", "Beautify SQL", Some("CmdOrCtrl+Alt+F")),
    ],
    &[("history", "Query History", Some("CmdOrCtrl+Y"))],
];

const TOOLS: &[Section] = &[
    &[
        ("process-list", "Process List", None),
        ("users", "Users & Privileges", None),
    ],
    &[("backup", "Backup…", None), ("restore", "Restore…", None)],
    &[("command-line", "Open in Command Line", None)],
];

const GO: &[Section] = &[
    &[
        ("open-anything", "Open Table or Query…", Some("CmdOrCtrl+P")),
        ("switch-database", "Switch Database…", Some("CmdOrCtrl+K")),
    ],
    &[
        ("next-tab", "Next Tab", Some("CmdOrCtrl+Shift+]")),
        ("prev-tab", "Previous Tab", Some("CmdOrCtrl+Shift+[")),
        ("home", "Connections", Some("CmdOrCtrl+Shift+H")),
    ],
    &[
        ("open-diagram", "ER Diagram", None),
        ("open-metrics", "Metrics", None),
    ],
];

pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let menus = [
        app_menu(app)?,
        custom(app, "File", FILE, |b| b)?,
        edit_menu(app)?,
        custom(app, "View", VIEW, |b| b.separator().fullscreen())?,
        custom(app, "Data", DATA, |b| b)?,
        custom(app, "Query", QUERY, |b| b)?,
        custom(app, "Go", GO, |b| b)?,
        custom(app, "Tools", TOOLS, |b| b)?,
        SubmenuBuilder::new(app, "Window")
            .minimize()
            .maximize()
            .build()?,
    ];
    let refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = menus
        .iter()
        .map(|m| m as &dyn tauri::menu::IsMenuItem<Wry>)
        .collect();
    MenuBuilder::new(app).items(&refs).build()
}

type Builder<'a> = SubmenuBuilder<'a, Wry, AppHandle>;

fn item(app: &AppHandle, (id, label, accel): Item) -> tauri::Result<MenuItem<Wry>> {
    let custom = OVERRIDES.read().unwrap().get(id).map(|k| accelerator(k));
    MenuItem::with_id(app, id, label, true, custom.as_deref().or(accel))
}

/// "Mod+Shift+K" (the UI's notation) → "CmdOrCtrl+Shift+K".
pub fn accelerator(keys: &str) -> String {
    keys.split('+')
        .map(|p| if p == "Mod" { "CmdOrCtrl" } else { p })
        .collect::<Vec<_>>()
        .join("+")
}

/// Applies custom shortcuts and rebuilds the menu so its accelerators match the UI.
pub fn apply_shortcuts(app: &AppHandle, shortcuts: &BTreeMap<String, String>) {
    *OVERRIDES.write().unwrap() = shortcuts.clone();
    if let Ok(menu) = build(app) {
        let _ = app.set_menu(menu);
    }
}

/// A submenu of custom items; `tail` appends predefined items at the end.
fn custom<'a>(
    app: &'a AppHandle,
    title: &str,
    sections: &[Section],
    tail: impl FnOnce(Builder<'a>) -> Builder<'a>,
) -> tauri::Result<Submenu<Wry>> {
    let mut b = SubmenuBuilder::new(app, title);
    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            b = b.separator();
        }
        for &it in section.iter() {
            b = b.item(&item(app, it)?);
        }
    }
    tail(b).build()
}

fn app_menu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let about = PredefinedMenuItem::about(app, Some("About LibraTable"), None)?;
    SubmenuBuilder::new(app, "LibraTable")
        .item(&about)
        .separator()
        .item(&item(app, ("settings", "Settings…", Some("CmdOrCtrl+,")))?)
        .item(&item(
            app,
            ("shortcuts", "Keyboard Shortcuts", Some("CmdOrCtrl+/")),
        )?)
        .item(&item(
            app,
            ("lock", "Lock LibraTable", Some("CmdOrCtrl+Shift+L")),
        )?)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()
}

fn edit_menu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()
}

#[cfg(test)]
mod tests {
    #[test]
    fn converts_ui_keys_to_accelerators() {
        assert_eq!(super::accelerator("Mod+Shift+K"), "CmdOrCtrl+Shift+K");
        assert_eq!(super::accelerator("F5"), "F5");
    }
}
