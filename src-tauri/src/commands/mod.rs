//! The IPC surface: an explicit list of commands (NFR "IPC Tauri"). Secrets only flow
//! UI → Rust (typed into a form or prompt); no command returns one.
//!
//! Commands only unpack arguments, find the session and call into the domain modules;
//! the checks every command shares (production confirmation, history) live here.

pub mod app;
pub mod connections;
pub mod data;
pub mod diagram;
pub mod export;
pub mod export_tables;
pub mod history;
pub mod import;
pub mod metadata;
pub mod metrics;
pub mod objects;
pub mod query;
pub mod redis;
pub mod schema;
pub mod server;
pub mod sidebar;
pub mod tools;

use std::time::Instant;

use serde_json::{json, Value};

use crate::error::{AppError, ErrorKind, Layer};
use crate::safe_mode;
use crate::state::{now_ms, AppState, Session};
use crate::storage::history::NewEntry;

type Cmd<T> = Result<T, AppError>;

fn confirmation_required(statements: Vec<Value>, level: safe_mode::Level) -> AppError {
    AppError::new(
        ErrorKind::ConfirmationRequired,
        Layer::App,
        "This connection asks for confirmation before these statements run.",
    )
    .data(json!({ "statements": statements, "level": level }))
}

fn read_only() -> AppError {
    AppError::new(
        ErrorKind::ReadOnly,
        Layer::App,
        "This connection is read-only, so it can't change data or structure.",
    )
    .hint("Turn off Read-only in the connection settings to make changes.")
}

/// Grid commits and schema changes: every statement is a write. Read-only connections
/// refuse them; otherwise the connection's safe mode decides whether to confirm.
fn confirm_production(
    state: &AppState,
    s: &Session,
    statements: &[String],
    confirmed: bool,
) -> Cmd<()> {
    let conn = state.live_connection(s);
    if conn.options.read_only {
        return Err(read_only());
    }
    let (scope, level) = safe_mode::policy(&conn);
    if scope == safe_mode::Scope::Nothing || confirmed {
        return Ok(());
    }
    Err(confirmation_required(
        statements
            .iter()
            .map(|p| json!({ "sql": p, "understood": true }))
            .collect(),
        level,
    ))
}

/// Safe mode (SAFE-1, SAFE-2) for SQL the user wrote: read-only connections refuse
/// writes, and the safe mode decides which statements need confirmation.
fn confirm_safe_mode(
    state: &AppState,
    s: &Session,
    statements: &[String],
    confirmed: bool,
) -> Cmd<()> {
    let conn = state.live_connection(s);
    if !safe_mode::read_only_violations(&conn, statements).is_empty() {
        return Err(read_only());
    }
    let need = safe_mode::needing_confirmation(&conn, statements);
    if need.is_empty() || confirmed {
        return Ok(());
    }
    let (_, level) = safe_mode::policy(&conn);
    Err(confirmation_required(
        need.iter()
            .map(|c| json!({ "sql": c.sql, "understood": c.understood }))
            .collect(),
        level,
    ))
}

/// When a run started, for its history entry.
struct Timer {
    started_at: i64,
    started: Instant,
}

impl Timer {
    fn start() -> Self {
        Self {
            started_at: now_ms(),
            started: Instant::now(),
        }
    }
}

/// How a run ended, for its history entry.
struct Outcome<'a> {
    /// "ok" | "error" | "cancelled"
    status: &'a str,
    row_count: Option<i64>,
    error: Option<&'a str>,
}

/// Records one run in the query history. History is best effort: a failure to write
/// it never fails the command.
async fn record_history(
    state: &AppState,
    s: &Session,
    sql: &str,
    timer: &Timer,
    outcome: Outcome<'_>,
) {
    let _ = state
        .history
        .add(NewEntry {
            connection_id: &s.connection.id,
            connection_name: &s.connection.name,
            database: &s.database,
            sql,
            started_at: timer.started_at,
            duration_ms: timer.started.elapsed().as_millis() as i64,
            row_count: outcome.row_count,
            status: outcome.status,
            error: outcome.error,
        })
        .await;
}

/// Lists commands for `generate_handler!` in one place.
#[macro_export]
macro_rules! all_commands {
    () => {
        tauri::generate_handler![
            $crate::commands::app::app_info,
            $crate::commands::app::get_settings,
            $crate::commands::app::save_settings,
            $crate::commands::app::console_entries,
            $crate::commands::app::clear_console,
            $crate::commands::app::passcode_set,
            $crate::commands::app::verify_passcode,
            $crate::commands::app::change_passcode,
            $crate::commands::app::set_window_view,
            $crate::commands::app::get_workspace,
            $crate::commands::app::save_workspace,
            $crate::commands::app::get_table_state,
            $crate::commands::app::save_table_state,
            $crate::commands::connections::list_connections,
            $crate::commands::connections::save_connection,
            $crate::commands::connections::delete_connection,
            $crate::commands::connections::duplicate_connection,
            $crate::commands::connections::save_group,
            $crate::commands::connections::delete_group,
            $crate::commands::connections::import_ssh_key,
            $crate::commands::connections::test_connection,
            $crate::commands::connections::connect,
            $crate::commands::connections::disconnect,
            $crate::commands::connections::open_sessions,
            $crate::commands::connections::trust_host_key,
            $crate::commands::connections::create_sqlite_file,
            $crate::commands::connections::export_connections,
            $crate::commands::connections::inspect_connections_file,
            $crate::commands::connections::import_connections,
            $crate::commands::connections::close_database,
            $crate::commands::connections::connection_url,
            $crate::commands::connections::parse_connection_url,
            $crate::commands::metadata::driver_capabilities,
            $crate::commands::metadata::list_drivers,
            $crate::commands::metadata::list_databases,
            $crate::commands::metadata::list_schemas,
            $crate::commands::metadata::list_objects,
            $crate::commands::metadata::list_columns,
            $crate::commands::metadata::table_structure,
            $crate::commands::metadata::completion_metadata,
            $crate::commands::metadata::refresh_metadata,
            $crate::commands::data::fetch_page,
            $crate::commands::data::count_rows,
            $crate::commands::data::preview_filter,
            $crate::commands::data::preview_changes,
            $crate::commands::data::commit_changes,
            $crate::commands::data::fetch_cell,
            $crate::commands::data::column_choices,
            $crate::commands::objects::list_schema_objects,
            $crate::commands::objects::object_definition,
            $crate::commands::objects::sequence_info,
            $crate::commands::sidebar::get_sidebar_state,
            $crate::commands::sidebar::save_sidebar_state,
            $crate::commands::schema::preview_schema_change,
            $crate::commands::schema::apply_schema_change,
            $crate::commands::query::split_statements,
            $crate::commands::query::run_query,
            $crate::commands::query::cancel_query,
            $crate::commands::redis::redis_databases,
            $crate::commands::redis::redis_scan,
            $crate::commands::redis::redis_read_key,
            $crate::commands::redis::redis_edit,
            $crate::commands::history::search_history,
            $crate::commands::history::delete_history,
            $crate::commands::history::purge_history,
            $crate::commands::history::list_saved_queries,
            $crate::commands::history::read_saved_query,
            $crate::commands::history::write_saved_query,
            $crate::commands::history::rename_saved_query,
            $crate::commands::history::delete_saved_query,
            $crate::commands::export::start_export,
            $crate::commands::export::cancel_export,
            $crate::commands::export_tables::start_export_tables,
            $crate::commands::import::preview_import_file,
            $crate::commands::import::preview_import,
            $crate::commands::import::start_import,
            $crate::commands::import::start_sql_file,
            $crate::commands::import::cancel_import,
            $crate::commands::diagram::schema_diagram,
            $crate::commands::diagram::get_diagram_layout,
            $crate::commands::diagram::save_diagram_layout,
            $crate::commands::diagram::save_diagram_file,
            $crate::commands::metrics::list_metric_boards,
            $crate::commands::metrics::save_metric_board,
            $crate::commands::metrics::delete_metric_board,
            $crate::commands::metrics::run_metric_query,
            $crate::commands::server::list_processes,
            $crate::commands::server::preview_process_action,
            $crate::commands::server::run_process_action,
            $crate::commands::server::list_users,
            $crate::commands::server::user_grants,
            $crate::commands::server::preview_user_change,
            $crate::commands::server::apply_user_change,
            $crate::commands::tools::plan_backup,
            $crate::commands::tools::plan_restore,
            $crate::commands::tools::set_tool_path,
            $crate::commands::tools::start_backup,
            $crate::commands::tools::start_restore,
            $crate::commands::tools::cancel_tool_job,
            $crate::commands::tools::command_line,
            $crate::commands::tools::open_in_terminal,
        ]
    };
}
