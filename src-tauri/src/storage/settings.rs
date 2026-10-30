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

/// "#rrggbb".
fn is_hex(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())
}

/// A color theme imported from a file: a base ("light" / "dark") and slot → "#rrggbb".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ColorTheme {
    pub id: String,
    pub name: String,
    pub base: String,
    pub colors: BTreeMap<String, String>,
}

/// A backup that runs on a schedule while LibraTable is open (TablePlus' backup
/// schedules): which databases, where to, how often, and how many files to keep.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct BackupSchedule {
    pub id: String,
    pub name: String,
    pub connection_id: String,
    pub databases: Vec<String>,
    pub folder: String,
    /// "hourly", "daily" or "weekly".
    pub every: String,
    /// "HH:MM" (daily, weekly); hourly runs at the minute of it.
    pub at: String,
    /// 0 = Sunday … 6 = Saturday (weekly).
    pub weekday: u8,
    /// Backups kept per database (0 keeps all).
    pub keep: u32,
    pub enabled: bool,
    /// Last run (ms since the epoch) and how it went.
    pub last_run: Option<i64>,
    pub last_error: Option<String>,
}

impl Default for BackupSchedule {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            connection_id: String::new(),
            databases: Vec::new(),
            folder: String::new(),
            every: "daily".into(),
            at: "02:00".into(),
            weekday: 0,
            keep: 7,
            enabled: true,
            last_run: None,
            last_error: None,
        }
    }
}

impl BackupSchedule {
    fn valid(&self) -> bool {
        let time_ok = self.at.len() == 5
            && self.at.as_bytes()[2] == b':'
            && matches!(
                (self.at[..2].parse::<u8>(), self.at[3..].parse::<u8>()),
                (Ok(h), Ok(m)) if h < 24 && m < 60
            );
        !self.id.is_empty()
            && !self.connection_id.is_empty()
            && !self.folder.trim().is_empty()
            && self.name.len() <= 80
            && self.databases.len() <= 100
            && ["hourly", "daily", "weekly"].contains(&self.every.as_str())
            && self.weekday < 7
            && time_ok
    }
}

/// A custom tag for connections (TablePlus' tag manager): a name and a "#rrggbb" color.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CustomTag {
    pub name: String,
    pub color: String,
}

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
    /// Tables open in a tab of their own; off, they replace the current table tab
    /// (Back / Forward step through what the tab showed).
    pub open_in_new_tab: bool,
    /// Grid: striped rows, the row-number column and the row height.
    pub alternating_rows: bool,
    pub show_row_numbers: bool,
    /// "compact", "normal" or "comfortable".
    pub row_density: String,
    /// Query editor: "default" or "vim" key bindings.
    pub editor_keymap: String,
    /// Query editor: SQL keywords become UPPERCASE as they are typed.
    pub uppercase_keywords: bool,
    /// Query editor: typing a bracket or quote adds the closing one.
    pub close_brackets: bool,
    /// Custom connection tags.
    pub custom_tags: Vec<CustomTag>,
    /// How query variables are written: "colon" (`:name`), "dollar", "dollar-braces",
    /// "double-braces" or "percent".
    pub variable_format: String,
    /// Variables inside quoted strings count too.
    pub variables_in_strings: bool,
    /// Id of the color theme ("" for the app's own light and dark).
    pub color_theme: String,
    /// Themes imported from files.
    pub custom_themes: Vec<ColorTheme>,
    /// The SQL assistant is on (the user agreed to send the schema to Anthropic).
    pub ai_enabled: bool,
    pub ai_model: String,
    pub backup_schedules: Vec<BackupSchedule>,
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
            open_in_new_tab: true,
            alternating_rows: true,
            show_row_numbers: true,
            row_density: "normal".into(),
            editor_keymap: "default".into(),
            uppercase_keywords: false,
            close_brackets: true,
            custom_tags: Vec::new(),
            variable_format: "colon".into(),
            variables_in_strings: false,
            color_theme: String::new(),
            custom_themes: Vec::new(),
            ai_enabled: false,
            ai_model: crate::ai::DEFAULT_MODEL.into(),
            backup_schedules: Vec::new(),
        }
    }
}

impl Settings {
    fn validate_themes(&self) -> AppResult<()> {
        let bad = |t: &ColorTheme| {
            t.id.is_empty()
                || t.id.len() > 60
                || t.name.trim().is_empty()
                || t.name.len() > 40
                || !["light", "dark"].contains(&t.base.as_str())
                || t.colors.len() > 40
                || t.colors.iter().any(|(k, v)| k.len() > 40 || !is_hex(v))
        };
        if self.custom_themes.len() > 50 || self.custom_themes.iter().any(bad) {
            return Err(AppError::validation("A color theme is not valid."));
        }
        if self.color_theme.len() > 60 {
            return Err(AppError::validation("Unknown color theme."));
        }
        Ok(())
    }

    fn validate_tags(&self) -> AppResult<()> {
        let hex = is_hex;
        let mut seen = std::collections::HashSet::new();
        for t in &self.custom_tags {
            let name = t.name.trim();
            if name.is_empty() || name.len() > 40 || !hex(&t.color) {
                return Err(AppError::validation(
                    "A tag needs a name (up to 40 characters) and a color.",
                ));
            }
            if !seen.insert(name.to_lowercase()) {
                return Err(AppError::validation(format!(
                    "There are two tags named \"{name}\"."
                )));
            }
        }
        if self.custom_tags.len() > 100 {
            return Err(AppError::validation("Use at most 100 tags."));
        }
        Ok(())
    }

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
        if !["compact", "normal", "comfortable"].contains(&self.row_density.as_str()) {
            return Err(AppError::validation("Unknown row height."));
        }
        if !["default", "vim"].contains(&self.editor_keymap.as_str()) {
            return Err(AppError::validation("Unknown editor key bindings."));
        }
        self.validate_tags()?;
        self.validate_themes()?;
        if self.backup_schedules.len() > 50 || self.backup_schedules.iter().any(|b| !b.valid()) {
            return Err(AppError::validation(
                "A backup schedule needs a connection, a folder and a valid time.",
            ));
        }
        if !crate::ai::MODELS.contains(&self.ai_model.as_str()) {
            return Err(AppError::validation("Unknown assistant model."));
        }
        let formats = [
            "colon",
            "dollar",
            "dollar-braces",
            "double-braces",
            "percent",
        ];
        if !formats.contains(&self.variable_format.as_str()) {
            return Err(AppError::validation("Unknown variable format."));
        }
        let bad_key = |(a, k): (&String, &String)| a.len() > 40 || k.is_empty() || k.len() > 40;
        if self.shortcuts.len() > 200 || self.shortcuts.iter().any(bad_key) {
            return Err(AppError::validation("A keyboard shortcut is not valid."));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_custom_tags() {
        let tag = |name: &str, color: &str| CustomTag {
            name: name.into(),
            color: color.into(),
        };
        let mut s = Settings {
            custom_tags: vec![tag("Billing", "#30a46c"), tag("ops", "#3E63DD")],
            ..Settings::default()
        };
        assert!(s.validate().is_ok());
        s.custom_tags.push(tag("billing", "#000000"));
        assert!(s.validate().is_err());
        s.custom_tags = vec![tag("x", "red")];
        assert!(s.validate().is_err());
        s.custom_tags = vec![tag(" ", "#000000")];
        assert!(s.validate().is_err());
    }

    #[test]
    fn checks_backup_schedules() {
        let ok = BackupSchedule {
            id: "b1".into(),
            connection_id: "c1".into(),
            folder: "/tmp/backups".into(),
            ..BackupSchedule::default()
        };
        let mut s = Settings {
            backup_schedules: vec![ok.clone()],
            ..Settings::default()
        };
        assert!(s.validate().is_ok());
        s.backup_schedules[0].at = "25:00".into();
        assert!(s.validate().is_err());
        s.backup_schedules[0] = BackupSchedule {
            folder: " ".into(),
            ..ok
        };
        assert!(s.validate().is_err());
    }
}
