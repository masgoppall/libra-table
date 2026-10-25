//! Metrics boards (MetricsBoardWindow): dashboards of widgets that each run a read-only
//! query. Boards are kept per connection in `metrics/<connectionId>.json`.

mod run;
mod vars;

pub use run::{read_statements, run_widget_query};
pub use vars::{substitute, VarKind, Variable};

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::storage::conn_files::ConnFiles;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WidgetKind {
    Scoreboard,
    Table,
    Bar,
    Line,
    Pie,
    Input,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct WidgetOptions {
    /// Charts: the x axis / slice label column (empty: the first column).
    pub label_column: String,
    /// Charts: the value columns (empty: every numeric column but the label).
    pub value_columns: Vec<String>,
    /// Scoreboard: text under the value.
    pub label: String,
    /// "auto" | "number" | "percent" | "currency" | "bytes"
    pub format: String,
    /// Input: the variable other widgets use as `{{name}}`.
    pub variable: String,
    pub default_value: String,
    /// Input: "text" | "number"
    pub input_type: String,
}

impl Default for WidgetOptions {
    fn default() -> Self {
        Self {
            label_column: String::new(),
            value_columns: Vec::new(),
            label: String::new(),
            format: "auto".into(),
            variable: String::new(),
            default_value: String::new(),
            input_type: "text".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Widget {
    pub id: String,
    pub title: String,
    pub kind: WidgetKind,
    #[serde(default)]
    pub sql: String,
    /// Grid columns (1–4) and rows (1–3) the widget spans.
    #[serde(default = "one")]
    pub width: u8,
    #[serde(default = "one")]
    pub height: u8,
    #[serde(default)]
    pub options: WidgetOptions,
}

fn one() -> u8 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    pub id: String,
    pub name: String,
    /// Seconds between automatic refreshes; 0 is off.
    #[serde(default)]
    pub refresh_seconds: u32,
    #[serde(default)]
    pub widgets: Vec<Widget>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct BoardsFile {
    #[serde(default)]
    boards: Vec<Board>,
}

fn files(data_dir: &Path) -> ConnFiles {
    ConnFiles::new(data_dir, "metrics")
}

pub fn list_boards(data_dir: &Path, connection_id: &str) -> AppResult<Vec<Board>> {
    let file: BoardsFile = files(data_dir).load(connection_id)?;
    Ok(file.boards)
}

/// Adds the board, or replaces the one with the same id. Returns it as stored.
pub fn save_board(data_dir: &Path, connection_id: &str, board: Board) -> AppResult<Board> {
    let board = validate(board)?;
    let files = files(data_dir);
    let mut file: BoardsFile = files.load(connection_id)?;
    match file.boards.iter_mut().find(|b| b.id == board.id) {
        Some(b) => *b = board.clone(),
        None => file.boards.push(board.clone()),
    }
    files.save(connection_id, &file)?;
    Ok(board)
}

pub fn delete_board(data_dir: &Path, connection_id: &str, board_id: &str) -> AppResult<()> {
    let files = files(data_dir);
    let mut file: BoardsFile = files.load(connection_id)?;
    file.boards.retain(|b| b.id != board_id);
    files.save(connection_id, &file)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn validate(mut board: Board) -> AppResult<Board> {
    board.name = board.name.trim().to_string();
    if !valid_id(&board.id) || board.widgets.iter().any(|w| !valid_id(&w.id)) {
        return Err(AppError::validation("Invalid board or widget id."));
    }
    if board.name.is_empty() {
        return Err(AppError::validation("Give the board a name."));
    }
    for w in &mut board.widgets {
        w.width = w.width.clamp(1, 4);
        w.height = w.height.clamp(1, 3);
        let var = &w.options.variable;
        if w.kind == WidgetKind::Input && !vars::is_name(var) {
            return Err(AppError::validation(format!(
                "\"{var}\" is not a variable name: use letters, digits and _."
            )));
        }
    }
    Ok(board)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn board(id: &str) -> Board {
        serde_json::from_value(json!({
            "id": id, "name": " Sales ", "refreshSeconds": 60,
            "widgets": [
                {"id": "w1", "title": "Orders", "kind": "scoreboard", "sql": "select 1",
                 "width": 9},
                {"id": "w2", "title": "Region", "kind": "input",
                 "options": {"variable": "region", "defaultValue": "EU"}}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn boards_are_saved_validated_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let saved = save_board(dir.path(), "c1", board("b1")).unwrap();
        assert_eq!(saved.name, "Sales");
        assert_eq!((saved.widgets[0].width, saved.widgets[0].height), (4, 1));
        assert_eq!(saved.widgets[1].options.input_type, "text");
        let mut renamed = saved.clone();
        renamed.name = "Ops".into();
        save_board(dir.path(), "c1", renamed).unwrap();
        save_board(dir.path(), "c1", board("b2")).unwrap();
        let all = list_boards(dir.path(), "c1").unwrap();
        assert_eq!(
            all.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
            ["Ops", "Sales"]
        );
        delete_board(dir.path(), "c1", "b1").unwrap();
        assert_eq!(list_boards(dir.path(), "c1").unwrap().len(), 1);
    }

    #[test]
    fn rejects_bad_names() {
        let dir = tempfile::tempdir().unwrap();
        let mut b = board("b1");
        b.widgets[1].options.variable = "bad name".into();
        assert!(save_board(dir.path(), "c1", b).is_err());
        assert!(save_board(dir.path(), "c1", board("../x")).is_err());
    }
}
