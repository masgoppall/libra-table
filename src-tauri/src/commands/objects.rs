//! Schema object commands: the functions, sequences, triggers… of a schema, an object's
//! definition, and a sequence's current settings. Changes go through `schema`.

use tauri::State;

use crate::db::objects::{self, ObjectDefinition, ObjectRef, SchemaObject, SequenceInfo};
use crate::state::AppState;

use super::Cmd;

#[tauri::command]
pub async fn list_schema_objects(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    schema: String,
) -> Cmd<Vec<SchemaObject>> {
    let s = state.session_in(&id, database.as_deref()).await?;
    Ok(s.schema_objects(&schema).await?.as_ref().clone())
}

#[tauri::command]
pub async fn object_definition(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    schema: String,
    object: ObjectRef,
) -> Cmd<ObjectDefinition> {
    let s = state.session_in(&id, database.as_deref()).await?;
    objects::definition(&s.pool, &schema, &object).await
}

#[tauri::command]
pub async fn sequence_info(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    schema: String,
    name: String,
) -> Cmd<SequenceInfo> {
    let s = state.session_in(&id, database.as_deref()).await?;
    objects::sequence(&s.pool, &schema, &name).await
}
