//! A change to the data as a plan: checks that must hold (the key exists, has this
//! type, the new name is free…) and the commands to send. Plans run on a connection of
//! their own: `WATCH` the checked keys, check, then `MULTI … EXEC`, so either every
//! command runs or none does (EXEC aborts when a watched key changed meanwhile).

use redis::aio::MultiplexedConnection;
use redis::Value as Reply;

use crate::error::{AppError, AppResult};

use super::args;
use super::conn::command_error;
use super::keys::replies;
use super::values::{reply_bytes, scalar_text};

#[derive(Debug, Clone, PartialEq)]
pub enum Check {
    Exists(Vec<u8>),
    Missing(Vec<u8>),
    Type(Vec<u8>, &'static str),
    /// The list element at the index still holds this value.
    ListItem(Vec<u8>, i64, Vec<u8>),
}

impl Check {
    fn key(&self) -> &[u8] {
        match self {
            Check::Exists(k) | Check::Missing(k) | Check::Type(k, _) | Check::ListItem(k, ..) => k,
        }
    }

    fn command(&self) -> redis::Cmd {
        match self {
            Check::Exists(k) | Check::Missing(k) => redis::cmd("EXISTS").arg(k).clone(),
            Check::Type(k, _) => redis::cmd("TYPE").arg(k).clone(),
            Check::ListItem(k, i, _) => redis::cmd("LINDEX").arg(k).arg(*i).clone(),
        }
    }

    fn verify(&self, r: &Reply) -> AppResult<()> {
        let name = || String::from_utf8_lossy(self.key()).into_owned();
        let text = scalar_text(r).unwrap_or_default();
        match self {
            Check::Exists(_) if text != "1" => Err(gone(&name())),
            Check::Missing(_) if text != "0" => Err(AppError::validation(format!(
                "A key named \"{}\" already exists.",
                name()
            ))),
            Check::Type(_, _) if text == "none" => Err(gone(&name())),
            Check::Type(_, want) if text != *want => Err(AppError::validation(format!(
                "\"{}\" holds a {text}, not a {want}.",
                name()
            ))),
            Check::ListItem(_, _, want) if reply_bytes(r) != *want || matches!(r, Reply::Nil) => {
                Err(AppError::validation(
                    "The list changed meanwhile; reload it and try again.",
                ))
            }
            _ => Ok(()),
        }
    }
}

fn gone(key: &str) -> AppError {
    AppError::not_found(format!("The key \"{key}\" no longer exists."))
        .hint("Someone else may have deleted or renamed it, or it expired. Refresh the list.")
}

/// Checks plus commands (each command is its arguments).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plan {
    pub checks: Vec<Check>,
    pub commands: Vec<Vec<Vec<u8>>>,
}

impl Plan {
    pub fn check(&mut self, c: Check) {
        if !self.checks.contains(&c) {
            self.checks.push(c);
        }
    }

    pub fn push<I, A>(&mut self, parts: I)
    where
        I: IntoIterator<Item = A>,
        A: AsRef<[u8]>,
    {
        self.commands
            .push(parts.into_iter().map(|a| a.as_ref().to_vec()).collect());
    }

    /// The commands as redis-cli lines (confirmation dialog, history).
    pub fn previews(&self) -> Vec<String> {
        self.commands.iter().map(|c| args::preview(c)).collect()
    }
}

fn conflict() -> AppError {
    AppError::validation("The key changed while saving, so nothing was changed.")
        .hint("Reload the key and make the change again.")
}

/// Runs `plan` atomically on a connection nobody else uses.
pub async fn execute(conn: &mut MultiplexedConnection, plan: &Plan) -> AppResult<Vec<Reply>> {
    if plan.commands.is_empty() {
        return Ok(Vec::new());
    }
    let watched: Vec<&[u8]> = plan.checks.iter().map(Check::key).collect();
    if !watched.is_empty() {
        let mut watch = redis::cmd("WATCH");
        for k in &watched {
            watch.arg(*k);
        }
        let _: Reply = watch.query_async(conn).await.map_err(command_error)?;
        let mut pipe = redis::pipe();
        for c in &plan.checks {
            pipe.add_command(c.command());
        }
        let found = replies(conn, &pipe, plan.checks.len()).await?;
        let failed = plan
            .checks
            .iter()
            .zip(&found)
            .find_map(|(c, r)| c.verify(r).err());
        if let Some(e) = failed {
            let _: Result<Reply, _> = redis::cmd("UNWATCH").query_async(conn).await;
            return Err(e);
        }
    }
    let mut tx = redis::pipe();
    tx.atomic();
    for parts in &plan.commands {
        tx.add_command(super::run::command(parts));
    }
    match tx.query_async::<Reply>(conn).await.map_err(command_error)? {
        Reply::Nil => Err(conflict()),
        Reply::Array(items) => Ok(items),
        other => Ok(vec![other]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_explain_what_went_wrong() {
        let k = b"user:1".to_vec();
        assert!(Check::Exists(k.clone()).verify(&Reply::Int(1)).is_ok());
        assert!(Check::Exists(k.clone()).verify(&Reply::Int(0)).is_err());
        assert!(Check::Missing(k.clone()).verify(&Reply::Int(1)).is_err());
        let t = Check::Type(k.clone(), "hash");
        assert!(t.verify(&Reply::SimpleString("hash".into())).is_ok());
        let e = t.verify(&Reply::SimpleString("list".into())).unwrap_err();
        assert!(e.message.contains("holds a list"));
        let item = Check::ListItem(k, 0, b"a".to_vec());
        assert!(item.verify(&Reply::BulkString(b"a".to_vec())).is_ok());
        assert!(item.verify(&Reply::Nil).is_err());
    }

    #[test]
    fn previews_quote_arguments() {
        let mut p = Plan::default();
        p.push(["SET", "my key", "v"]);
        p.check(Check::Exists(b"a".to_vec()));
        p.check(Check::Exists(b"a".to_vec()));
        assert_eq!(p.previews(), ["SET \"my key\" v"]);
        assert_eq!(p.checks.len(), 1);
    }
}
