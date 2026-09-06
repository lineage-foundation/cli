//! Spend guardrails for the `lineage` CLI: allowlist, per-transaction cap,
//! daily cap, and confirmation.
//!
//! `check` enforces, in order: the allowlist (if non-empty, `to` must be
//! listed), `max_amount`, confirmation (`--yes` or `confirm = "auto"`),
//! and `daily_cap` (a per-day spend tally persisted as JSON under the
//! config directory). A denial never has a side effect: the daily tally is
//! only updated once every other check has already passed.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{Confirm, Profile};
use crate::exit::Code;

/// A guardrail rejected the requested spend.
#[derive(Debug, Clone, PartialEq)]
pub struct Denied {
    pub reason: String,
}

impl fmt::Display for Denied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for Denied {}

impl From<Denied> for Code {
    fn from(_: Denied) -> Self {
        Code::Denied
    }
}

fn denied(reason: impl Into<String>) -> Denied {
    Denied {
        reason: reason.into(),
    }
}

/// Check a proposed payment of `amount_lngx` LNGX to `to` against
/// `profile`'s guardrails. `confirm_flag` is `true` when `--yes` was
/// passed on the command line.
pub fn check(
    profile: &Profile,
    to: &str,
    amount_lngx: f64,
    confirm_flag: bool,
) -> Result<(), Denied> {
    let dir = tally_dir();
    check_in(&dir, profile, to, amount_lngx, confirm_flag)
}

/// The directory the daily spend tally is persisted under:
/// `dirs::config_dir()/lineage`. Falls back to the current directory if
/// the OS has no notion of a config directory.
fn tally_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("lineage")
}

/// Core guardrail logic, parameterized over the tally directory so tests
/// never touch the real config directory.
fn check_in(
    dir: &Path,
    profile: &Profile,
    to: &str,
    amount_lngx: f64,
    confirm_flag: bool,
) -> Result<(), Denied> {
    if !profile.allowlist.is_empty() && !profile.allowlist.iter().any(|allowed| allowed == to) {
        return Err(denied(format!("{to} is not in the profile allowlist")));
    }

    if let Some(max_amount) = profile.max_amount {
        if amount_lngx > max_amount {
            return Err(denied(format!(
                "amount {amount_lngx} LNGX exceeds the profile max_amount of {max_amount} LNGX"
            )));
        }
    }

    if !confirm_flag && profile.confirm != Confirm::Auto {
        return Err(denied(
            "confirmation required: pass --yes or set confirm = \"auto\" on the profile",
        ));
    }

    if let Some(daily_cap) = profile.daily_cap {
        let mut tally = load_tally(dir);
        let key = today_key();
        let spent_today = tally.get(&key).copied().unwrap_or(0.0);
        let projected = spent_today + amount_lngx;
        if projected > daily_cap {
            return Err(denied(format!(
                "today's spend of {spent_today} LNGX plus {amount_lngx} LNGX would exceed the profile daily_cap of {daily_cap} LNGX"
            )));
        }
        tally.insert(key, projected);
        save_tally(dir, &tally);
    }

    Ok(())
}

/// A whole day, computed without a timezone dependency: days since the
/// Unix epoch in the local process's clock, used purely as a stable daily
/// bucket key.
fn today_key() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (elapsed.as_secs() / 86_400).to_string()
}

fn tally_path(dir: &Path) -> PathBuf {
    dir.join("spent.json")
}

fn load_tally(dir: &Path) -> HashMap<String, f64> {
    let path = tally_path(dir);
    fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

fn save_tally(dir: &Path, tally: &HashMap<String, f64>) {
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    if let Ok(contents) = serde_json::to_string(tally) {
        let _ = fs::write(tally_path(dir), contents);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_with(mutate: impl FnOnce(&mut Profile)) -> Profile {
        let mut profile = Profile::testnet();
        mutate(&mut profile);
        profile
    }

    #[test]
    fn allowlist_rejects_an_unlisted_address() {
        let profile = profile_with(|p| p.allowlist = vec!["addr-a".to_string()]);
        let dir = tempfile::tempdir().unwrap();

        let result = check_in(dir.path(), &profile, "addr-b", 1.0, true);

        assert!(matches!(result, Err(Denied { .. })));
    }

    #[test]
    fn allowlist_accepts_a_listed_address() {
        let profile = profile_with(|p| p.allowlist = vec!["addr-a".to_string()]);
        let dir = tempfile::tempdir().unwrap();

        let result = check_in(dir.path(), &profile, "addr-a", 1.0, true);

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn empty_allowlist_allows_any_address() {
        let profile = Profile::testnet();
        let dir = tempfile::tempdir().unwrap();

        let result = check_in(dir.path(), &profile, "anyone", 1.0, true);

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn max_amount_rejects_an_over_cap_amount() {
        let profile = profile_with(|p| p.max_amount = Some(10.0));
        let dir = tempfile::tempdir().unwrap();

        let result = check_in(dir.path(), &profile, "addr-a", 10.5, true);

        assert!(matches!(result, Err(Denied { .. })));
    }

    #[test]
    fn max_amount_accepts_an_amount_at_or_under_cap() {
        let profile = profile_with(|p| p.max_amount = Some(10.0));
        let dir = tempfile::tempdir().unwrap();

        let result = check_in(dir.path(), &profile, "addr-a", 10.0, true);

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn missing_confirmation_rejects_when_profile_is_manual() {
        let profile = profile_with(|p| p.confirm = Confirm::Manual);
        let dir = tempfile::tempdir().unwrap();

        let result = check_in(dir.path(), &profile, "addr-a", 1.0, false);

        assert!(matches!(result, Err(Denied { .. })));
    }

    #[test]
    fn yes_flag_satisfies_confirmation_on_a_manual_profile() {
        let profile = profile_with(|p| p.confirm = Confirm::Manual);
        let dir = tempfile::tempdir().unwrap();

        let result = check_in(dir.path(), &profile, "addr-a", 1.0, true);

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn auto_confirm_profile_passes_without_the_yes_flag() {
        let profile = profile_with(|p| p.confirm = Confirm::Auto);
        let dir = tempfile::tempdir().unwrap();

        let result = check_in(dir.path(), &profile, "addr-a", 1.0, false);

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn daily_cap_rejects_once_todays_spend_would_be_exceeded() {
        let profile = profile_with(|p| p.daily_cap = Some(15.0));
        let dir = tempfile::tempdir().unwrap();

        let first = check_in(dir.path(), &profile, "addr-a", 10.0, true);
        assert_eq!(first, Ok(()));

        let second = check_in(dir.path(), &profile, "addr-a", 10.0, true);
        assert!(matches!(second, Err(Denied { .. })));
    }

    #[test]
    fn daily_cap_persists_the_tally_across_calls_in_the_same_dir() {
        let profile = profile_with(|p| p.daily_cap = Some(100.0));
        let dir = tempfile::tempdir().unwrap();

        assert_eq!(check_in(dir.path(), &profile, "addr-a", 40.0, true), Ok(()));
        assert_eq!(check_in(dir.path(), &profile, "addr-a", 40.0, true), Ok(()));
        // 40 + 40 + 40 = 120 > 100
        assert!(matches!(
            check_in(dir.path(), &profile, "addr-a", 40.0, true),
            Err(Denied { .. })
        ));

        assert!(tally_path(dir.path()).exists());
    }

    #[test]
    fn denied_maps_to_the_denied_exit_code() {
        let denied = denied("nope");
        assert_eq!(Code::from(denied), Code::Denied);
    }
}
