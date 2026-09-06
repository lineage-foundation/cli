//! Passphrase resolution for the `lineage` CLI.
//!
//! Never prompts interactively: the passphrase comes from the
//! `LINEAGE_PASSPHRASE` environment variable, else the OS keyring (service
//! `"lineage"`, account named after the profile), else a typed error.

use std::fmt;

use crate::config::Profile;

/// Errors resolving a passphrase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassphraseError {
    /// Neither `LINEAGE_PASSPHRASE` nor an OS keyring entry was found.
    NotFound,
}

impl fmt::Display for PassphraseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PassphraseError::NotFound => write!(
                f,
                "no passphrase available: set LINEAGE_PASSPHRASE or store one in the OS keyring"
            ),
        }
    }
}

impl std::error::Error for PassphraseError {}

/// The OS keyring service name under which passphrases are stored.
const SERVICE: &str = "lineage";

/// Resolve the passphrase for `profile`: `LINEAGE_PASSPHRASE` first, then
/// the OS keyring entry for this profile's name.
pub fn passphrase(profile: &Profile) -> Result<String, PassphraseError> {
    resolve(profile, keyring_lookup)
}

/// Core resolution logic, parameterized over the keyring lookup so tests
/// never have to touch the real OS keyring.
fn resolve(
    profile: &Profile,
    keyring_lookup: impl Fn(&str) -> Option<String>,
) -> Result<String, PassphraseError> {
    if let Ok(value) = std::env::var("LINEAGE_PASSPHRASE") {
        return Ok(value);
    }
    keyring_lookup(&profile.name).ok_or(PassphraseError::NotFound)
}

/// Look up the passphrase in the OS keyring. Any failure (no backend
/// available, no such entry, permission denied, ...) is treated as "not
/// found" rather than propagated or panicking.
fn keyring_lookup(account: &str) -> Option<String> {
    keyring::Entry::new(SERVICE, account)
        .ok()?
        .get_password()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Profile;
    use std::sync::Mutex;

    // Serializes tests that mutate the process-wide LINEAGE_PASSPHRASE env
    // var so they don't race each other under `cargo test`'s default
    // parallel test threads.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn profile_named(name: &str) -> Profile {
        let mut profile = Profile::testnet();
        profile.name = name.to_string();
        profile
    }

    #[test]
    fn env_passphrase_is_read() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var("LINEAGE_PASSPHRASE", "from-env");

        let profile = profile_named("testnet");
        let result = resolve(&profile, |_| panic!("keyring should not be consulted"));

        std::env::remove_var("LINEAGE_PASSPHRASE");
        assert_eq!(result, Ok("from-env".to_string()));
    }

    #[test]
    fn missing_env_and_keyring_is_a_typed_error() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var("LINEAGE_PASSPHRASE");

        let profile = profile_named("testnet");
        // Stub keyring lookup instead of the real OS keyring: absence of a
        // secret must yield the typed error, never a panic.
        let result = resolve(&profile, |_| None);

        assert_eq!(result, Err(PassphraseError::NotFound));
    }

    #[test]
    fn keyring_hit_is_used_when_env_is_unset() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var("LINEAGE_PASSPHRASE");

        let profile = profile_named("dev");
        let result = resolve(&profile, |account| {
            assert_eq!(account, "dev");
            Some("from-keyring".to_string())
        });

        assert_eq!(result, Ok("from-keyring".to_string()));
    }

    #[test]
    fn public_passphrase_fn_never_touches_real_keyring_when_env_is_set() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::set_var("LINEAGE_PASSPHRASE", "from-env-public");

        let profile = profile_named("testnet");
        let result = passphrase(&profile);

        std::env::remove_var("LINEAGE_PASSPHRASE");
        assert_eq!(result, Ok("from-env-public".to_string()));
    }
}
