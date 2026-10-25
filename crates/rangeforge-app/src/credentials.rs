//! API keys, kept in the operating system's own credential store: Credential Manager on
//! Windows, Keychain on macOS. The system encrypts them and ties them to the signed-in user,
//! so they are not in any file of ours and cannot be copied along with the settings.
//!
//! On macOS the Keychain may ask the first time a new version of the app reads a key; choose
//! Always Allow.

use anyhow::{Context, Result};
use rangeforge_session::secrets::SecretStore;

const SERVICE: &str = "RangeForge";

/// The system's credential store.
#[derive(Debug, Default)]
pub struct OsStore;

fn entry(name: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, name).context("opening the system credential store")
}

impl SecretStore for OsStore {
    fn get(&self, name: &str) -> Result<Option<String>> {
        match entry(name)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(error).context("reading from the system credential store"),
        }
    }

    fn set(&self, name: &str, value: &str) -> Result<()> {
        entry(name)?
            .set_password(value)
            .context("writing to the system credential store")
    }

    fn delete(&self, name: &str) -> Result<()> {
        match entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error).context("removing from the system credential store"),
        }
    }
}
