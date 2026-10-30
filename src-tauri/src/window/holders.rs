//! Which windows show which connection. Windows share one session per connection
//! (`connect` is idempotent); it is closed once the last window holding it lets go.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

static HOLDERS: Mutex<Option<HashMap<String, HashSet<String>>>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut HashMap<String, HashSet<String>>) -> T) -> T {
    let mut guard = HOLDERS.lock().unwrap();
    f(guard.get_or_insert_with(HashMap::new))
}

pub fn hold(connection: &str, window: &str) {
    with(|h| {
        h.entry(connection.to_string())
            .or_default()
            .insert(window.to_string());
    });
}

/// True when no window holds the connection any more (it can be closed).
pub fn release(connection: &str, window: &str) -> bool {
    with(|h| {
        let Some(set) = h.get_mut(connection) else {
            return true;
        };
        set.remove(window);
        if set.is_empty() {
            h.remove(connection);
            true
        } else {
            false
        }
    })
}

/// A closed window lets go of everything; returns the connections nobody holds now.
pub fn release_window(window: &str) -> Vec<String> {
    with(|h| {
        let mut orphaned = Vec::new();
        h.retain(|conn, set| {
            set.remove(window);
            if set.is_empty() {
                orphaned.push(conn.clone());
            }
            !set.is_empty()
        });
        orphaned.sort();
        orphaned
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closes_when_the_last_window_lets_go() {
        hold("c-h1", "main");
        hold("c-h1", "win-1");
        assert!(!release("c-h1", "main"));
        hold("c-h2", "win-1");
        assert_eq!(release_window("win-1"), ["c-h1", "c-h2"]);
        assert!(release("c-h1", "main"));
    }
}
