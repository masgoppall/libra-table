//! ER diagram commands: the schema's tables, columns and foreign keys in one call, the
//! saved table positions, and writing an exported image.

use std::path::Path;

use base64::Engine;
use tauri::State;

use crate::diagram::{self, Layout, SchemaDiagram};
use crate::error::AppError;
use crate::state::AppState;
use crate::storage::atomic_write;

use super::Cmd;

#[tauri::command]
pub async fn schema_diagram(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    schema: String,
) -> Cmd<SchemaDiagram> {
    let s = state.session_in(&id, database.as_deref()).await?;
    diagram::schema_diagram(&s.pool, &schema).await
}

/// `key` names the diagram: `<database>/<schema>`.
#[tauri::command]
pub fn get_diagram_layout(state: State<'_, AppState>, id: String, key: String) -> Cmd<Layout> {
    diagram::load_layout(state.storage.dir(), &id, &key)
}

#[tauri::command]
pub fn save_diagram_layout(
    state: State<'_, AppState>,
    id: String,
    key: String,
    layout: Layout,
) -> Cmd<()> {
    diagram::save_layout(state.storage.dir(), &id, &key, layout)
}

/// Writes an exported diagram (`.svg` text or `.png` as base64) to the path the save
/// dialog returned.
#[tauri::command]
pub fn save_diagram_file(path: String, svg: Option<String>, png_base64: Option<String>) -> Cmd<()> {
    let ext = Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let bytes = match (ext.as_deref(), svg, png_base64) {
        (Some("svg"), Some(svg), _) => svg.into_bytes(),
        (Some("png"), _, Some(b64)) => base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|_| AppError::validation("The image data is not valid."))?,
        _ => return Err(AppError::validation("Save the diagram as .svg or .png.")),
    };
    atomic_write(Path::new(&path), &bytes)
}
