//! App passcode (TablePlus "Passcode lock"): only a salted hash is kept, in the OS
//! keychain. The UI locks itself at start and after the idle time set in Settings.

use std::time::Duration;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::storage::crypto;

const SECRET: &str = "passcode";
const MIN_LENGTH: usize = 4;
/// Slows down guessing: every wrong attempt waits this long.
const WRONG_DELAY: Duration = Duration::from_millis(600);

pub fn is_set(state: &AppState) -> bool {
    state.secrets.has_app(SECRET)
}

pub async fn verify(state: &AppState, code: &str) -> AppResult<bool> {
    let ok = match state.secrets.get_app(SECRET)? {
        Some(stored) => crypto::verify_secret(code, &stored),
        None => true,
    };
    if !ok {
        tokio::time::sleep(WRONG_DELAY).await;
    }
    Ok(ok)
}

/// Sets, changes (`next` = Some) or removes (`next` = None) the passcode. Changing or
/// removing an existing one needs it.
pub async fn change(state: &AppState, current: Option<&str>, next: Option<&str>) -> AppResult<()> {
    if is_set(state) && !verify(state, current.unwrap_or_default()).await? {
        return Err(AppError::validation("The current passcode is wrong."));
    }
    match next {
        Some(code) if code.chars().count() < MIN_LENGTH => Err(AppError::validation(
            "Use at least 4 characters for the passcode.",
        )),
        Some(code) => state.secrets.set_app(SECRET, &crypto::hash_secret(code)),
        None => state.secrets.delete_app(SECRET),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::test_state;

    #[tokio::test]
    async fn set_verify_change_remove() {
        let dir = tempfile::tempdir().unwrap();
        let s = test_state(dir.path()).await;
        assert!(!is_set(&s));
        assert!(change(&s, None, Some("12")).await.is_err());
        change(&s, None, Some("2468")).await.unwrap();
        assert!(is_set(&s) && verify(&s, "2468").await.unwrap());
        assert!(!verify(&s, "1357").await.unwrap());
        assert!(change(&s, Some("bad"), None).await.is_err());
        change(&s, Some("2468"), None).await.unwrap();
        assert!(!is_set(&s));
    }
}
