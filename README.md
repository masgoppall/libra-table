# LibraTable

Desktop database client for **PostgreSQL, MySQL/MariaDB, SQLite, Snowflake, BigQuery, DynamoDB and Redis**, built with Tauri 2 (Rust) and React. The driver picker lists every TablePlus driver; the ones not built yet show as "Coming soon". Connect directly or through an SSH tunnel, work in several databases of a server at once, create and change databases, schemas and tables, filter and edit table data, run SQL, search history, import CSV/JSON and SQL files, export to CSV/JSON/SQL, back up and restore, browse functions, triggers and other schema objects, draw an ER diagram and build metrics boards. Everything has a keyboard shortcut (⌘/ lists them, and they can be changed).

The product spec is the PRD in `../reverse-tableplus/docs/prd-database-client.md`; requirement IDs below (CON-1, FLT-4, …) refer to it.

## Run

```bash
pnpm install
pnpm tauri dev        # app with hot reload
pnpm tauri build --bundles app   # release LibraTable.app in src-tauri/target/release/bundle/macos (16 MB)
pnpm tauri build                 # .app + .dmg (the DMG step drives Finder, so run it from a desktop session)
```

Requires Rust ≥ 1.82, Node 22, pnpm, and the Xcode command line tools on macOS.

## Test

```bash
pnpm check                             # frontend: tsc + eslint + prettier --check + vitest
pnpm format                            # prettier --write (100 columns)
cd src-tauri && cargo test             # backend: unit, SQLite end-to-end, SSH tunnel end-to-end
cargo fmt --check && cargo clippy --all-targets -- -D warnings   # both clean
```

Lines are at most 100 characters on both sides: Prettier and ESLint `max-len` for TypeScript, `rustfmt.toml` (`max_width = 100`) for Rust. ESLint also forbids `any` and warns on files over 300 lines.

The Postgres and MySQL end-to-end tests run when admin URLs are set; without them they test SQLite only. Each suite creates its own `libratable_*` databases (and `libratable_*` roles/users for the server tools), uses them, and drops them even when a check fails. Nothing else on the server is touched.

```bash
LIBRATABLE_TEST_PG_URL=postgres://postgres@127.0.0.1:5432/postgres \
LIBRATABLE_TEST_MYSQL_URL=mysql://root@127.0.0.1:3306/mysql \
cargo test
```

| Suite | What it covers |
| --- | --- |
| `src-tauri/src/**` unit tests (148) | DDL generation for all three drivers (create/alter/rename/duplicate/drop, SQLite table rebuild keeping constraints), filter builder, statement splitter (incl. trigger/procedure bodies, MySQL comment rules), safe-mode classifier, preview rendering, value decoding, storage + migration, known_hosts, `~/.ssh/config`, URL import, history FTS, export sinks, edit statements, safe-mode levels and read-only, console log masking, AES-GCM sealing and passcode hashing, connections export/import, CSV/JSON parsing and type inference, import batching under each driver's parameter limit, SQL-file streaming (DELIMITER, COPY), metrics variable quoting, diagram/board storage, object DDL, process/user statements, dump commands |
| `tests/integration/` (3 drivers) | Schema changes end to end (create tables with FKs and indexes, change types/NULL/defaults, rename/add/drop columns in one go, SQLite rebuild with a dependent view, FK add with violating rows rolls back, primary key, rename/duplicate/truncate/drop), a second database opened next to the first (create, use, drop), metadata, every filter type (wildcards, quotes, injection attempts, Any column, Raw SQL), paging, counts, edits + rollback, multi-statement runs, server-side cancel, exports |
| `tests/connection_options.rs` | Read-only connections refused by the server, startup query, per driver |
| `tests/grid_cells.rs` | Enum choices (PG `pg_enum`, MySQL `COLUMN_TYPE`), full cell values for Quick Look |
| `tests/objects/` | Functions, procedures, sequences, materialized views, triggers and types: list, definition, drop |
| `tests/server_tools/` | Process list and kill/cancel, users and grants (create, change, drop), `pg_dump`/`mysqldump` backup and restore |
| `tests/visual/` | CSV/JSON import (skip vs stop, per-run vs per-batch transactions, cancel), SQL file with an error, PG `COPY`, MySQL `DELIMITER` trigger, multi-table export, diagram catalog, widget queries with variables and an injection attempt |
| `tests/ssh_tunnel.rs` (in-process SSH server) | Unknown host key → trust, wrong password, forwarding, unreachable target, changed host key (incl. a different key type) → replace, encrypted key passphrase |
| `tests/perf.rs` (`cargo test --release --test perf -- --ignored`) | PRD acceptance: history search over 100k entries, memory while exporting 5M rows |
| Frontend unit tests (86) | `lib/` (cells, copy as TSV/CSV/JSON/SQL/Markdown, quoting, fuzzy search), action layers and key/menu de-duplication, `store/workspaceModel` (tabs open/close/move/restore), `features/data/pending` (edits → changes), `features/structure/designerModel` (designer → ALTER ops, validation), grid keyboard navigation and column order, header sort, custom shortcuts and conflicts, safe-mode levels, cell editors, diagram layout, chart scales, import and board models |
| `src/features/*/*.test.tsx` (45) | UI flows on the whole app against `src/test/fakeBackend.ts`: connection list, new connection form, password prompt, host key dialog, table + filters (Enter applies), production commit, NULL cells stay NULL, ⌘Z, ⌘T/⌘W/⌘1, ⌘P palette, ⌘K database switch with tabs in two databases, create database, create table, alter a column with ⌘S, type-to-confirm drop, read-only and "safe" connections, Quick Look, enum/date editors, hidden columns, FK jump, console log, passcode lock, connections export, process list, users, backup/restore, functions and triggers, query variables, import wizard, SQL file, export tables, ER diagram, metrics board |

## Architecture

```
React UI (webview)  ──invoke/events──▶  Rust core (Tokio)
```

**Frontend (`src/`)** — dependencies point one way: `features → services/store → api → types`; `lib/` and `domain/` are pure and import nothing from React or Tauri. Imports use the `@/` alias.

```
app/          shell: App, TopBar, global keys + native menu, appearance, app-level actions
features/     one folder per screen, each split into small components + hooks
  connections/   Home, list/rows, driver picker, URL import, form/ (one file per section)
  workspace/     WorkspaceView, TabBar, TabPanes, welcome, workspace actions, tab helpers
  sidebar/       data loading, sections, object items, menus, resizable width
  data/          DataView, toolbar, status bar, row detail, filters/, pending edits (+ undo)
  query/         QueryView, SQL editor, results pane, runner, saved queries
  structure/     TableDesigner, columns/indexes/foreign keys editors, designerModel (pure)
  schema/        database/schema/table flows, "New database" dialog
  objects/       functions, procedures, sequences, triggers, types: lists, definition tab
  server/        process list, users & privileges, backup/restore, open in command line
  import/        import wizard (CSV/JSON), run SQL file, tool dialog host
  diagram/       ER diagram tab (SVG, layout, pan/zoom, SVG/PNG export)
  metrics/       metrics boards: widgets, charts, input variables
  console/ lock/ palette/ history/ export/ settings/
components/   shared UI: Modal, ModalActions, ContextMenu, dialogs/, grid/ (virtualized DataGrid)
hooks/        useTauriEvent, useDebouncedEffect, useDarkMode, useWindowKeys
store/        zustand, one slice per concern (connections, settings, workspaces, ui, dialogs,
              toasts, tools); workspaceModel.ts holds the pure tab rules; selectors for components
services/     app flows without UI: connect with prompts, safe mode, schema change preview/apply
api/          typed wrappers per domain around `invoke`, one `api` facade
domain/       driver/tag defaults, schema defaults, DDL suggestions, filter defaults
types/        the IPC types, split by domain; AppError is a union typed per error kind
lib/          pure helpers: actions/ (shortcuts, keys, dispatch), cells, copyRows, sql, format…
styles/       tokens, base, primitives, utilities, then one file per screen
test/         jsdom shims, the fake backend, the app harness
```

**Backend (`src-tauri/src/`)**

```
lib.rs, setup.rs, menu.rs   builder, startup state, native menu (as data tables)
commands/                   thin IPC commands per domain (app, connections, metadata, data,
                            objects, sidebar, schema, query, history, export, export_tables,
                            import, diagram, metrics, server, tools) + shared confirmation
                            helpers
state/                      AppState, Session (pool, tunnel, cache, per-database children)
connections/                store, secrets per attempt, connect/test/tunnel, keys, URL import,
                            keep-alive, encrypted export/import of connections
db/                         pool, sql, split, data, changes, exec, value/, filter/, meta/ (per
                            driver), ddl/ (spec, plan, clauses, create, alter/, SQLite rebuild,
                            run: the preview is what runs), objects/, cell (Quick Look),
                            choices (enum values)
ssh/                        open, auth, host key, tunnel, known_hosts, ~/.ssh/config
storage/                    JSON config (atomic writes, schemaVersion), settings, history
                            (SQLite + FTS5), saved queries, OS keychain, crypto (AES-GCM),
                            sidebar pins/recent, tool paths, per-connection files
export/                     streaming plan + CSV / JSON / SQL sinks, several tables at once
import/                     CSV/JSON source, type inference, batched inserts, SQL-file runner
diagram/                    FK catalog per driver for the ER diagram
metrics/                    widget queries (read-only, variables quoted per driver)
server/                     processes, users and grants, pg_dump/mysqldump/psql jobs
console.rs                  tracing layer → console log (ring buffer, secrets masked)
app_lock.rs                 passcode (hash in the keychain)
safe_mode.rs, error.rs, models.rs
```

- **Secrets** live only in the OS keychain (service `com.libratable.app`, account `<connectionId>_<db_password|ssh_password|ssh_key|ssh_key_passphrase>`). They travel UI → Rust when typed and never come back. Config writes refuse secret-like keys.
- **Values are bind parameters.** Filters and edits never paste values into SQL; Postgres binds are cast to the column type (`$1::integer`). LIKE wildcards typed by the user are escaped (`ESCAPE '!'`).
- **Safe mode is enforced in Rust.** Each connection has a safe mode (auto = confirm on `production`, silent, alert or "safe" for writes or for everything; "safe" asks to type the connection name). Until the UI resends with `confirmed: true`, `run_query`, `commit_changes`, schema changes and imports return `confirmation_required`. A **read-only** connection refuses writes in the app and in the server session (`READ ONLY` transactions, SQLite `read_only`).
- **No auto-connect.** Open tabs are restored per connection, but nothing connects until you click.
- **Several databases at once.** Each tab remembers its database; Rust keeps one session per database it has seen, all sharing the connection settings and SSH tunnel (`Session::in_database`).
- **Schema changes are previewed.** Creating/altering/dropping databases, schemas, tables, columns, indexes and foreign keys goes through `db/ddl/`; the confirmation dialog shows the exact statements. Postgres and SQLite run them in one transaction; on SQLite, column changes rebuild the table from its original `CREATE TABLE` so constraints, indexes and triggers survive.
- **Shortcuts and the menu are one system.** `src/lib/actions/` maps keys to actions; the native menu (`menu.rs`) sends the same action ids, and the active tab, then the workspace, then the app gets the first chance to handle them.

Data folder: `~/Library/Application Support/com.libratable.app/` (`connections.json`, `groups.json`, `table_state.json`, `workspace.json`, `settings.json`, `sidebar.json`, `tools.json`, `history.db`, `queries/<connectionId>/*.sql`, `diagrams/<connectionId>.json`, `metrics/<connectionId>.json`).

## PRD coverage (MVP)

| Module | Status | Notes |
| --- | --- | --- |
| Connections CON-1…9 | Done | Driver picker, form for 3 drivers, groups (one level), tags/colors, duplicate (optionally with secrets), copy as URL without password, URL import incl. `postgresql+ssh://`, deep links |
| SSH & TLS SSH-1…7, TLS-1 | Done | Password and key auth, `~/.ssh/config` fallback, known_hosts prompt/trust/replace, local forward on 127.0.0.1, layered errors, SSL modes + CA/cert/key |
| Credentials & storage STO-1…6 | Done | |
| Schema browser SCH-1…6 | Done | |
| Data grid GRD-1…6 | Done | Row virtualization (columns are not virtualized) |
| Filters FLT-1…6 | Done | Default column is a setting (first column, primary key or Any column) |
| Query editor QRY-1…7 | Done | |
| Export EXP-1…4 | Done | |
| Safe mode SAFE-1…2 | Done | |

## Beyond the MVP (TablePlus parity)

| Area | What is there |
| --- | --- |
| Connection safety | Safe mode levels, read-only connections, startup query, keep-alive ping, `READ ONLY` badge |
| Grid | Quick Look (Space) with JSON tree, image and hex preview; enum dropdown and date picker editors; hide/show columns; FK icon and "open referenced row"; filter by a column from its header |
| Schema objects | Functions, procedures, sequences, materialized views, triggers, types: list, definition tab, drop; pinned and recent items in the sidebar |
| Query editor | Beautify (⌥⌘F), Explain (⌥⌘E), `:name` variables with a prompt, append-results mode, run marks |
| Server tools | Process list (cancel/kill), users & privileges, backup/restore with `pg_dump`/`mysqldump`, open in `psql`/`mysql` |
| Import & export | CSV/JSON import wizard into an existing or new table, run SQL file (streams large dumps), export several tables at once |
| Visual | ER diagram tab (drag, pan/zoom, saved layout, SVG/PNG export), metrics boards (scoreboard, table, bar, line, pie, input widgets; auto refresh) |
| App | Console log of every statement the app sends (⇧⌘C), custom keyboard shortcuts (menu follows), passcode lock (⇧⌘L, auto lock after idle), encrypted export/import of connections |

Drivers not built yet (shown as "Coming soon"): SQL Server, Oracle, ClickHouse, DuckDB, Cassandra, MongoDB, Elasticsearch, LibSQL/Turso, Cloudflare D1 and Vertica; CockroachDB, Redshift and Greenplum use the PostgreSQL code without per-flavor adjustments yet. Snowflake, BigQuery and DynamoDB are tested against mocks and emulators, not the real services. Also not built: AI assistant and MCP server, plugins and custom scripts, themes from git, several windows, and a split query editor.

## Measured

| PRD criterion | Target | Result (M-series Mac, release build) |
| --- | --- | --- |
| Search one word in 100,000 history entries | < 200 ms | 11 ms |
| Export 5,000,000 rows to CSV | memory grows < 200 MB | +2 MB (264 MB file in 10.2 s) |
| Main JS bundle | — | 467 kB (140 kB gzip); the SQL editor (562 kB, with the formatter), ER diagram and metrics load with their first tab |

## Security review

An independent review of the backend found 12 issues; all are fixed and covered by tests where testable: safe mode now catches `EXPLAIN (ANALYZE) <write>`, `SET GLOBAL/PERSIST/PASSWORD`, MySQL-only comment tricks (`--x`, `/*! */`), exports of write queries, and tag changes on open sessions; changed host keys of a different type show the "changed" warning; SQLite interrupts happen while the connection is guaranteed alive; late cancels can't hit the next query; previews no longer re-substitute `$n`/`?` inside values or identifiers; disconnect can't hang on a running query.

## Known limitations

- Verified on macOS 27 (arm64) only; Windows/Linux builds are untested.
- Not measured yet: startup time, grid frame rate, sidebar with 2,000 tables.
- The UI was verified by automated flows (jsdom + mocked IPC), by screenshots of every main screen in light and dark (Chromium with a fake backend), and by launching the app. It has not been reviewed inside the real macOS window by a person yet.
- Postgres `verify-full` through an SSH tunnel fails hostname verification, because the driver connects to 127.0.0.1.
- MySQL's `SLEEP()` returns 1 instead of an error when cancelled; other queries report the cancel as an error.
- Editing binary values is not supported; they display as `<binary N bytes>`, open in Quick Look (up to 16 MiB) and export in full.
- Import reads UTF-8 and Latin-1 only; SQL files must be UTF-8. A table created by "Import into new table" stays if the import fails.
- Imports are cancelled between chunks; a running SQL-file statement is dropped with its connection rather than cancelled on the server. Metrics widget queries can't be cancelled.
- The ER diagram shows base tables only (up to 1,000); a composite foreign key is drawn from its first column.
- Several tables are selected for export through a checkbox list; the sidebar itself has no multi-select.
- Updater and code signing/notarization are not configured.
- MySQL needs a prefix length to index TEXT/BLOB columns; the index editor doesn't offer one yet (use varchar, or a query).
- SQLite has no comments; the comment fields are hidden for it. Table comments of existing tables aren't shown or edited yet.
- Column order can't be changed from the structure editor (only MySQL supports it in place).
