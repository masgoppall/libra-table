//! App settings and per-table grid state, as stored in `settings.json` and
//! `table_state.json`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::db::filter::{FilterRow, SortSpec};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct TableState {
    pub column_order: Vec<String>,
    pub column_widths: BTreeMap<String, f64>,
    pub filters: Vec<FilterRow>,
    pub sort: Vec<SortSpec>,
    /// Columns hidden in the grid (TablePlus "Select columns").
    pub hidden_columns: Vec<String>,
}

const MAX_LOCK_MINUTES: u32 = 24 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub history_retention_days: u32,
    pub default_filter_operator: String,
    /// Column a new filter row starts on: "first", "any" (Any column) or "pk".
    pub default_filter_column: String,
    pub theme: String,
    pub editor_font_size: u32,
    /// Text size of the grid and sidebar, in px.
    pub ui_font_size: u32,
    /// With a passcode set, lock after this many idle minutes (0 = only at start).
    pub lock_after_minutes: u32,
    /// Custom keyboard shortcuts: action id → keys ("Mod+Shift+K").
    pub shortcuts: BTreeMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            history_retention_days: 365,
            default_filter_operator: "eq".into(),
            default_filter_column: "first".into(),
            theme: "system".into(),
            editor_font_size: 13,
            ui_font_size: 13,
            lock_after_minutes: 0,
            shortcuts: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// Checks values the UI sends before they are saved.
    pub fn validate(&self) -> AppResult<()> {
        if self.history_retention_days == 0 || self.history_retention_days > 3650 {
            return Err(AppError::validation(
                "History retention must be between 1 and 3650 days.",
            ));
        }
        let editor_ok = (9..=32).contains(&self.editor_font_size);
        let ui_ok = (11..=18).contains(&self.ui_font_size);
        if !editor_ok || !ui_ok {
            return Err(AppError::validation(
                "Text sizes must be 9–32 px (editor) and 11–18 px (interface).",
            ));
        }
        if !["first", "any", "pk"].contains(&self.default_filter_column.as_str()) {
            return Err(AppError::validation("Unknown default filter column."));
        }
        if self.lock_after_minutes > MAX_LOCK_MINUTES {
            return Err(AppError::validation(
                "Lock after at most 24 hours of inactivity.",
            ));
        }
        let bad_key = |(a, k): (&String, &String)| a.len() > 40 || k.is_empty() || k.len() > 40;
        if self.shortcuts.len() > 200 || self.shortcuts.iter().any(bad_key) {
            return Err(AppError::validation("A keyboard shortcut is not valid."));
        }
        Ok(())
    }
}
