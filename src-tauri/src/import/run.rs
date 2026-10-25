//! Running an import: the file is parsed on a blocking thread and records arrive over
//! a bounded channel; `job` inserts them.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::state::Session;

use super::conn::Conn;
use super::job::Job;
use super::source::{self, CsvEvent, Record};
use super::{FileFormat, ImportRequest, SourceSpec};

pub(super) const PROGRESS_EVERY: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RowError {
    pub row: u64,
    pub line: Option<u64>,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub rows_read: u64,
    pub inserted: u64,
    pub skipped: u64,
    pub bytes_read: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    /// Rows committed.
    pub inserted: u64,
    pub skipped: u64,
    pub rows_read: u64,
    /// The first row errors (all of them up to 200).
    pub errors: Vec<RowError>,
    pub error_count: u64,
    pub cancelled: bool,
    /// Stopped at a bad row (on error: stop).
    pub stopped: bool,
    /// Rows inserted in the open transaction were rolled back.
    pub rolled_back: bool,
    /// A failure that ended the run (unreadable file, lost connection).
    pub fatal: Option<AppError>,
}

pub(super) enum Item {
    Row(Record),
    Fatal(AppError),
}

/// How the consuming loop ended.
pub(super) enum End {
    Done,
    Stopped,
    Cancelled,
    Failed(AppError),
}

impl From<AppError> for End {
    fn from(e: AppError) -> Self {
        End::Failed(e)
    }
}

/// Reads the file and sends its records until the receiver goes away.
fn produce(spec: SourceSpec, columns: Vec<String>, tx: mpsc::Sender<Item>, read: Arc<AtomicU64>) {
    let send = |item: Item| tx.blocking_send(item).is_ok();
    let result = source::open(&spec.path, read).and_then(|(reader, _)| match spec.format {
        FileFormat::Csv => source::read_csv(reader, &spec.csv, |ev| match ev {
            CsvEvent::Columns(_) => true,
            CsvEvent::Row(r) => send(Item::Row(r)),
        }),
        FileFormat::Json => source::read_json(reader, |row, obj| {
            let values = obj.map(|o| source::json_values(&o, &columns));
            send(Item::Row(Record {
                row,
                line: None,
                values,
            }))
        }),
    });
    if let Err(e) = result {
        send(Item::Fatal(e));
    }
}

/// Runs the import into an existing table. `progress` is called every ~150 ms.
pub async fn run(
    session: &Session,
    req: &ImportRequest,
    cancel: &CancellationToken,
    mut progress: impl FnMut(&ImportProgress),
) -> AppResult<ImportSummary> {
    let target = super::target(session, req, None).await?;
    let total_bytes = std::fs::metadata(&req.source.path)?.len();
    let read = Arc::new(AtomicU64::new(0));
    let (tx, mut rx) = mpsc::channel(1024);
    let (spec, cols, counter) = (req.source.clone(), req.source_columns.clone(), read.clone());
    let producer = tokio::task::spawn_blocking(move || produce(spec, cols, tx, counter));
    let mut job = Job::new(Conn::acquire(&session.pool).await?, &target, req);
    let mut report = |job: &Job| {
        progress(&ImportProgress {
            rows_read: job.s.rows_read,
            inserted: job.s.inserted + job.uncommitted,
            skipped: job.s.skipped,
            bytes_read: read.load(Ordering::Relaxed),
            total_bytes,
        })
    };
    let end = job.consume(&mut rx, cancel, &mut report).await;
    drop(rx);
    let _ = producer.await;
    let summary = job.finish(end).await;
    report_final(&summary, &mut progress, total_bytes);
    Ok(summary)
}

fn report_final(s: &ImportSummary, progress: &mut impl FnMut(&ImportProgress), total: u64) {
    progress(&ImportProgress {
        rows_read: s.rows_read,
        inserted: s.inserted,
        skipped: s.skipped,
        bytes_read: total,
        total_bytes: total,
    });
}
