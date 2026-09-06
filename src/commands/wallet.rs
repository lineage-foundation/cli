//! Wallet commands: local keystore management (`new`, `address`, `list`)
//! backed by `lineage_sdk::Wallet` at the profile's `wallet_path`, and
//! node-wallet operations (`import`, `passphrase`) via the SDK client.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{json, Value};

use lineage_sdk::{NodeClass, Wallet};

use super::{build_client, ok_code, report_sdk_error};
use crate::cli::NodeArg;
use crate::config::Profile;
use crate::exit::Code;
use crate::output::Reporter;
use crate::secrets;

/// Resolve the wallet passphrase: `LINEAGE_PASSPHRASE`, else the OS
/// keyring entry for this profile.
pub fn read_passphrase(profile: &Profile) -> Result<String, String> {
    secrets::passphrase(profile).map_err(|err| err.to_string())
}

/// The profile's local keystore path, or a usage error if it isn't
/// configured.
fn require_wallet_path(profile: &Profile) -> Result<&Path, String> {
    profile
        .wallet_path
        .as_deref()
        .ok_or_else(|| "profile has no wallet_path configured".to_string())
}

/// Look up the local wallet's passphrase, reporting a usage or runtime
/// failure through `reporter` and returning `None` on error.
fn resolve_local(profile: &Profile, reporter: &Reporter) -> Result<(PathBuf, String), ExitCode> {
    let path = require_wallet_path(profile).map_err(|msg| {
        reporter.fail(Code::Usage, &msg, None);
        ExitCode::from(Code::Usage)
    })?;
    let passphrase = read_passphrase(profile).map_err(|msg| {
        reporter.fail(Code::Runtime, &msg, None);
        ExitCode::from(Code::Runtime)
    })?;
    Ok((path.to_path_buf(), passphrase))
}

pub async fn new(profile: &Profile, reporter: &Reporter) -> ExitCode {
    let (path, passphrase) = match resolve_local(profile, reporter) {
        Ok(pair) => pair,
        Err(code) => return code,
    };
    match Wallet::create(&path, &passphrase) {
        Ok(_) => {
            let data = json!({ "wallet_path": path });
            reporter.ok(&data, &format!("created wallet at {}", path.display()));
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}

pub async fn address(profile: &Profile, reporter: &Reporter) -> ExitCode {
    let (path, passphrase) = match resolve_local(profile, reporter) {
        Ok(pair) => pair,
        Err(code) => return code,
    };
    let mut wallet = match Wallet::open(&path, &passphrase) {
        Ok(wallet) => wallet,
        Err(err) => return report_sdk_error(reporter, &err),
    };
    match wallet.new_address() {
        Ok(address) => {
            reporter.ok(&json!({ "address": address }), &address);
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}

pub async fn list(profile: &Profile, reporter: &Reporter) -> ExitCode {
    let (path, passphrase) = match resolve_local(profile, reporter) {
        Ok(pair) => pair,
        Err(code) => return code,
    };
    let wallet = match Wallet::open(&path, &passphrase) {
        Ok(wallet) => wallet,
        Err(err) => return report_sdk_error(reporter, &err),
    };
    let addresses = wallet.addresses();
    let human = if addresses.is_empty() {
        "no addresses".to_string()
    } else {
        addresses.join("\n")
    };
    reporter.ok(&json!({ "addresses": addresses }), &human);
    ok_code()
}

pub async fn import(profile: &Profile, reporter: &Reporter, node: NodeArg, file: &Path) -> ExitCode {
    let client = match build_client(profile) {
        Ok(client) => client,
        Err(err) => return report_sdk_error(reporter, &err),
    };
    let contents = match std::fs::read_to_string(file) {
        Ok(contents) => contents,
        Err(err) => {
            reporter.fail(
                Code::Usage,
                &format!("could not read {}: {err}", file.display()),
                None,
            );
            return Code::Usage.into();
        }
    };
    let payload: Value = match serde_json::from_str(&contents) {
        Ok(payload) => payload,
        Err(err) => {
            reporter.fail(
                Code::Usage,
                &format!("invalid JSON in {}: {err}", file.display()),
                None,
            );
            return Code::Usage.into();
        }
    };
    let class: NodeClass = node.into();
    match client.import_keypairs(class, payload).await {
        Ok(value) => {
            reporter.ok(&value, &value.to_string());
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}

pub async fn passphrase(profile: &Profile, reporter: &Reporter, node: NodeArg, new: &str) -> ExitCode {
    let client = match build_client(profile) {
        Ok(client) => client,
        Err(err) => return report_sdk_error(reporter, &err),
    };
    let old = match read_passphrase(profile) {
        Ok(old) => old,
        Err(msg) => {
            reporter.fail(Code::Runtime, &msg, None);
            return Code::Runtime.into();
        }
    };
    let class: NodeClass = node.into();
    match client.change_passphrase(class, &old, new).await {
        Ok(()) => {
            reporter.ok(&json!({}), "passphrase changed");
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}
