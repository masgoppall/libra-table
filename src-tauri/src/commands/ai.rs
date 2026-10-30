//! SQL assistant commands: the API key in the Keychain, and asking for a query.

use tauri::State;

use crate::ai::{self, AskAnswer, AskRequest, KEY_NAME};
use crate::error::AppError;
use crate::state::AppState;

use super::Cmd;

/// Whether a key is saved (checked without reading it, so the Keychain doesn't ask).
#[tauri::command]
pub fn ai_key_status(state: State<'_, AppState>) -> bool {
    state.secrets.has_app(KEY_NAME)
}

#[tauri::command]
pub fn ai_set_key(state: State<'_, AppState>, key: String) -> Cmd<()> {
    let key = key.trim();
    if key.is_empty() || key.len() > 400 || key.chars().any(char::is_whitespace) {
        return Err(AppError::validation("That doesn't look like an API key."));
    }
    state.secrets.set_app(KEY_NAME, key)
}

#[tauri::command]
pub fn ai_clear_key(state: State<'_, AppState>) -> Cmd<()> {
    state.secrets.delete_app(KEY_NAME)
}

#[tauri::command]
pub async fn ai_ask(state: State<'_, AppState>, request: AskRequest) -> Cmd<AskAnswer> {
    if request.question.trim().is_empty() {
        return Err(AppError::validation("Ask a question first."));
    }
    let key = state.secrets.get_app(KEY_NAME)?.ok_or_else(|| {
        AppError::validation("The SQL assistant has no API key.")
            .hint("Add an Anthropic API key in Settings › SQL assistant.")
    })?;
    ai::ask(&key, &request).await
}
