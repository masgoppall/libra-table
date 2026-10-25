//! Schema change commands: preview the DDL, then apply it.

use std::sync::Arc;

use tauri::State;

use crate::db::ddl::{self, Ctx, Plan, SchemaChange};
use crate::error::AppError;
use crate::state::{AppState, Session};

use super::{confirm_production, record_history, Cmd, Outcome, Timer};

/// Database-level changes run on the connection's own session; the others on the
/// session of the database they are in.
async fn ddl_session(
    state: &AppState,
    id: &str,
    database: Option<&str>,
    change: &SchemaChange,
) -> Cmd<Arc<Session>> {
    match change {
        SchemaChange::CreateDatabase { .. } | SchemaChange::DropDatabase { .. } => {
            state.session(id).await
        }
        _ => state.session_in(id, database).await,
    }
}

/// The plan for `change` and the exact statements it runs.
async fn plan(s: &Session, change: &SchemaChange) -> Cmd<(Ctx, Plan, Vec<String>)> {
    let ctx = ddl::context(&s.pool, change).await?;
    let plan = ddl::build(s.driver(), change, &ctx)?;
    let statements = ddl::preview(&s.pool, &plan, change, &ctx).await?;
    Ok((ctx, plan, statements))
}

#[tauri::command]
pub async fn preview_schema_change(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    change: SchemaChange,
) -> Cmd<Vec<String>> {
    let s = ddl_session(&state, &id, database.as_deref(), &change).await?;
    Ok(plan(&s, &change).await?.2)
}

#[tauri::command]
pub async fn apply_schema_change(
    state: State<'_, AppState>,
    id: String,
    database: Option<String>,
    change: SchemaChange,
    confirmed: bool,
) -> Cmd<Vec<String>> {
    let s = ddl_session(&state, &id, database.as_deref(), &change).await?;
    let (ctx, plan, statements) = plan(&s, &change).await?;
    confirm_production(&state, &s, &statements, confirmed)?;
    if let SchemaChange::DropDatabase { name, .. } = &change {
        prepare_drop_database(&s, name).await?;
    }
    let timer = Timer::start();
    let result = ddl::apply(&s.pool, &plan, &change, &ctx).await;
    let outcome = Outcome {
        status: if result.is_ok() { "ok" } else { "error" },
        row_count: None,
        error: result.as_ref().err().map(|e| e.message.as_str()),
    };
    record_history(&state, &s, &statements.join(";\n"), &timer, outcome).await;
    if let Ok(primary) = state.session(&id).await {
        primary.clear_all_caches().await;
    }
    result?;
    Ok(statements)
}

/// A database can't be dropped while LibraTable is connected to it: refuse the one the
/// connection itself uses, and close our own pool to any other.
async fn prepare_drop_database(s: &Session, name: &str) -> Cmd<()> {
    if name == s.database {
        return Err(AppError::validation(format!(
            "LibraTable is connected to \"{name}\" itself, so it can't drop it."
        ))
        .hint("Set another database in the connection settings, reconnect, then drop this one."));
    }
    s.close_database(name).await;
    Ok(())
}
