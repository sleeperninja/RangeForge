//! Where API keys are kept.
//!
//! Keys never go in `settings.json`. The program that uses this crate supplies a
//! [`SecretStore`]: on Windows and macOS the operating system's own credential store
//! (Credential Manager, Keychain), which encrypts them and ties them to the user. This crate
//! only knows the interface, so the rules can be tested with the in-memory
//! [`MemoryStore`] and never touch a real credential store.

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::Result;

/// The name the Last.fm key is kept under.
pub const LASTFM: &str = "lastfm-api-key";

/// The environment variable that can supply the Last.fm key when none is saved. It is meant
/// for development (a `.env` file in a debug build) and for scripts.
pub const LASTFM_ENV: &str = "LASTFM_API";

/// Somewhere to keep secrets.
pub trait SecretStore: Send + Sync {
    fn get(&self, name: &str) -> Result<Option<String>>;
    fn set(&self, name: &str, value: &str) -> Result<()>;
    /// Removes a secret. Removing one that is not there is not an error.
    fn delete(&self, name: &str) -> Result<()>;
}

/// Where a key in use came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    /// Saved from the Settings window.
    Saved,
    /// Taken from the environment.
    Environment,
    None,
}

/// A key and where it came from.
#[derive(Clone, PartialEq, Eq)]
pub struct Key {
    pub value: String,
    pub source: KeySource,
}

// Never print the value, even by accident in a log or a test failure.
impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Key")
            .field("value", &"<hidden>")
            .field("source", &self.source)
            .finish()
    }
}

/// The Last.fm key to use: the saved one, or else the one in the environment.
pub fn lastfm_key(store: &dyn SecretStore, environment: Option<String>) -> Result<Option<Key>> {
    let saved = store.get(LASTFM)?.filter(|value| !value.trim().is_empty());
    Ok(
        match (saved, environment.filter(|v| !v.trim().is_empty())) {
            (Some(value), _) => Some(Key {
                value,
                source: KeySource::Saved,
            }),
            (None, Some(value)) => Some(Key {
                value: value.trim().to_string(),
                source: KeySource::Environment,
            }),
            (None, None) => None,
        },
    )
}

/// Keeps secrets in memory only. For tests, and as a stand-in when no credential store works.
#[derive(Debug, Default)]
pub struct MemoryStore {
    secrets: Mutex<HashMap<String, String>>,
}

impl SecretStore for MemoryStore {
    fn get(&self, name: &str) -> Result<Option<String>> {
        Ok(self
            .secrets
            .lock()
            .map_err(|_| anyhow::anyhow!("the secret store is unavailable"))?
            .get(name)
            .cloned())
    }

    fn set(&self, name: &str, value: &str) -> Result<()> {
        self.secrets
            .lock()
            .map_err(|_| anyhow::anyhow!("the secret store is unavailable"))?
            .insert(name.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<()> {
        self.secrets
            .lock()
            .map_err(|_| anyhow::anyhow!("the secret store is unavailable"))?
            .remove(name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_key_beats_the_environment() {
        let store = MemoryStore::default();
        store.set(LASTFM, "saved-key").unwrap();
        let key = lastfm_key(&store, Some("env-key".to_string()))
            .unwrap()
            .unwrap();
        assert_eq!(key.value, "saved-key");
        assert_eq!(key.source, KeySource::Saved);
    }

    #[test]
    fn the_environment_is_the_fallback() {
        let store = MemoryStore::default();
        let key = lastfm_key(&store, Some(" env-key ".to_string()))
            .unwrap()
            .unwrap();
        assert_eq!(key.value, "env-key");
        assert_eq!(key.source, KeySource::Environment);
    }

    #[test]
    fn no_key_anywhere_is_none_and_blanks_count_as_none() {
        let store = MemoryStore::default();
        assert_eq!(lastfm_key(&store, None).unwrap(), None);
        store.set(LASTFM, "   ").unwrap();
        assert_eq!(lastfm_key(&store, Some(String::new())).unwrap(), None);
    }

    #[test]
    fn deleting_forgets_the_key_and_deleting_nothing_is_fine() {
        let store = MemoryStore::default();
        store.set(LASTFM, "k").unwrap();
        store.delete(LASTFM).unwrap();
        assert_eq!(store.get(LASTFM).unwrap(), None);
        store.delete(LASTFM).unwrap();
    }

    #[test]
    fn a_key_never_shows_in_debug_output() {
        let key = Key {
            value: "super-secret-value".to_string(),
            source: KeySource::Saved,
        };
        let shown = format!("{key:?}");
        assert!(!shown.contains("super-secret-value"), "{shown}");
        assert!(shown.contains("hidden"));
    }
}
