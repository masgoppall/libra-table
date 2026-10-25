//! Runs one native tool as a child process: feeds it a file on stdin (restores),
//! reports its stderr lines and the bytes written or read, and kills it on cancel.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult, ErrorKind, Layer};

const TICK: Duration = Duration::from_millis(400);
/// Lines kept to explain a failure.
const TAIL: usize = 6;

pub struct Job {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Secrets go here, never in `args`.
    pub env: Vec<(String, String)>,
    /// Streamed to the child's stdin.
    pub stdin: Option<PathBuf>,
    /// File the tool writes, measured for progress.
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    Line(String),
    /// Bytes written (backup) or read (restore), of `total` when known.
    Bytes {
        done: u64,
        total: Option<u64>,
    },
}

/// Runs the job to the end. A non-zero exit becomes an error carrying the last
/// stderr lines; a cancel kills the child and returns `Cancelled`.
pub async fn run(
    job: Job,
    token: &CancellationToken,
    mut on: impl FnMut(Progress),
) -> AppResult<()> {
    let mut child = spawn(&job)?;
    let (tx, mut lines) = mpsc::unbounded_channel::<String>();
    if let Some(err) = child.stderr.take() {
        tokio::spawn(read_lines(err, tx));
    }
    let fed = Arc::new(AtomicU64::new(0));
    let total = match (&job.stdin, child.stdin.take()) {
        (Some(path), Some(stdin)) => {
            let size = std::fs::metadata(path).map(|m| m.len()).ok();
            tokio::spawn(feed(path.clone(), stdin, fed.clone()));
            size
        }
        _ => None,
    };
    let bytes = || match &job.output {
        Some(out) => std::fs::metadata(out).map(|m| m.len()).unwrap_or(0),
        None => fed.load(Ordering::Relaxed),
    };
    let mut tail: Vec<String> = Vec::new();
    let mut tick = tokio::time::interval(TICK);
    let status = loop {
        tokio::select! {
            Some(line) = lines.recv() => keep(&mut tail, &line, &mut on),
            _ = tick.tick() => on(Progress::Bytes { done: bytes(), total }),
            _ = token.cancelled() => {
                let _ = child.kill().await;
                return Err(AppError::cancelled());
            }
            status = child.wait() => break status?,
        }
    };
    while let Some(line) = lines.recv().await {
        keep(&mut tail, &line, &mut on);
    }
    on(Progress::Bytes {
        done: bytes(),
        total,
    });
    if status.success() {
        return Ok(());
    }
    let name = job
        .program
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let code = status
        .code()
        .map(|c| c.to_string())
        .unwrap_or_else(|| "signal".into());
    Err(AppError::new(
        ErrorKind::Query,
        Layer::Database,
        format!("{name} failed (exit {code}): {}", tail.join("\n")),
    ))
}

fn spawn(job: &Job) -> AppResult<Child> {
    let mut cmd = Command::new(&job.program);
    cmd.args(&job.args)
        // Only our values: an inherited password must not reach another server.
        .env_remove("PGPASSWORD")
        .env_remove("MYSQL_PWD")
        .envs(job.env.iter().map(|(k, v)| (k, v)))
        .stdin(if job.stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    cmd.spawn().map_err(|e| {
        AppError::new(
            ErrorKind::Io,
            Layer::App,
            format!("Can't start {}: {e}", job.program.display()),
        )
        .hint("Check the tool path in the dialog.")
    })
}

fn keep(tail: &mut Vec<String>, line: &str, on: &mut impl FnMut(Progress)) {
    let line = line.trim_end().to_string();
    if line.is_empty() {
        return;
    }
    if tail.len() == TAIL {
        tail.remove(0);
    }
    tail.push(line.clone());
    on(Progress::Line(line));
}

async fn read_lines(err: tokio::process::ChildStderr, tx: mpsc::UnboundedSender<String>) {
    let mut reader = BufReader::new(err).lines();
    while let Ok(Some(line)) = reader.next_line().await {
        if tx.send(line).is_err() {
            break;
        }
    }
}

/// Copies the file into the child's stdin, counting bytes. Closing stdin at the end
/// tells the tool the script is over; a tool that stops early just breaks the pipe.
async fn feed(path: PathBuf, mut stdin: tokio::process::ChildStdin, fed: Arc<AtomicU64>) {
    let Ok(mut file) = tokio::fs::File::open(&path).await else {
        return;
    };
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match file.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        if stdin.write_all(&buf[..n]).await.is_err() {
            break;
        }
        fed.fetch_add(n as u64, Ordering::Relaxed);
    }
    let _ = stdin.shutdown().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(script: &str) -> Job {
        Job {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), script.into()],
            env: vec![("LT_SECRET".into(), "pw".into())],
            stdin: None,
            output: None,
        }
    }

    #[tokio::test]
    async fn reports_lines_and_failures() {
        let token = CancellationToken::new();
        let mut lines = Vec::new();
        let job = sh("echo one >&2; echo \"$LT_SECRET\" | wc -c >&2; exit 3");
        let r = run(job, &token, |p| {
            if let Progress::Line(l) = p {
                lines.push(l.trim().to_string());
            }
        })
        .await;
        assert_eq!(lines, vec!["one", "3"]);
        let e = r.unwrap_err();
        assert!(e.message.contains("exit 3"), "{}", e.message);
    }

    #[tokio::test]
    async fn feeds_stdin_and_cancels() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.sql");
        std::fs::write(&input, "hello").unwrap();
        let mut job = sh("cat >&2");
        job.stdin = Some(input);
        let token = CancellationToken::new();
        let mut last = None;
        run(job, &token, |p| {
            if let Progress::Bytes { done, total } = p {
                last = Some((done, total));
            }
        })
        .await
        .unwrap();
        assert_eq!(last, Some((5, Some(5))));

        let token = CancellationToken::new();
        let cancel = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            cancel.cancel();
        });
        let r = run(sh("sleep 10"), &token, |_| {}).await;
        assert_eq!(r.unwrap_err().kind, ErrorKind::Cancelled);
    }
}
