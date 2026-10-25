//! Creating an empty SQLite database file for a new connection.

use crate::error::{AppError, AppResult};

/// Creates the file. Refuses to touch a file that already exists.
pub async fn create_sqlite_file(path: &str) -> AppResult<()> {
    let path = path.trim();
    if path.is_empty() {
        return Err(AppError::validation(
            "Choose where to create the database file.",
        ));
    }
    if std::path::Path::new(path).exists() {
        return Err(
            AppError::validation(format!("A file already exists at {path}."))
                .hint("Choose another name, or open the existing file."),
        );
    }
    let o = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let mut conn = sqlx::ConnectOptions::connect(&o)
        .await
        .map_err(|e| AppError::from_connect(e, false))?;
    // Writing the header makes it a valid database file even before the first table.
    sqlx::query("PRAGMA user_version = 0")
        .execute(&mut conn)
        .await?;
    sqlx::Connection::close(conn).await?;
    Ok(())
}
