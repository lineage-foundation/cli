//! Remaining write commands: `items` (POST raw items to the mempool),
//! `tx submit`/`tx serialize`/`tx deserialize`, and `donate` (request a
//! testnet donation from the miner). `items`, `tx submit`, and `donate`
//! carry no amount or address to run the spend guardrails against, so they
//! only require confirmation (`--yes` or `confirm = "auto"`) via
//! `guard::confirm`. `tx serialize`/`tx deserialize` are reads-through-POST
//! and need no confirmation.

use std::path::Path;
use std::process::ExitCode;

use serde_json::Value;

use super::{build_client, ok_code, report_sdk_error};
use crate::config::Profile;
use crate::exit::Code;
use crate::guard;
use crate::output::Reporter;

/// Read and parse a JSON file, reporting a usage error through `reporter`
/// on failure.
fn read_json_file(reporter: &Reporter, file: &Path) -> Result<Value, ExitCode> {
    let contents = std::fs::read_to_string(file).map_err(|err| {
        reporter.fail(
            Code::Usage,
            &format!("could not read {}: {err}", file.display()),
            None,
        );
        ExitCode::from(Code::Usage)
    })?;
    serde_json::from_str(&contents).map_err(|err| {
        reporter.fail(
            Code::Usage,
            &format!("invalid JSON in {}: {err}", file.display()),
            None,
        );
        ExitCode::from(Code::Usage)
    })
}

/// Require confirmation for a no-amount write, reporting a denial through
/// `reporter` on failure.
fn require_confirm(profile: &Profile, reporter: &Reporter, confirmed: bool) -> Result<(), ExitCode> {
    guard::confirm(profile, confirmed).map_err(|denied| {
        let code = Code::from(denied.clone());
        reporter.fail(code, &denied.to_string(), None);
        ExitCode::from(code)
    })
}

pub async fn items(profile: &Profile, reporter: &Reporter, file: &Path, confirmed: bool) -> ExitCode {
    if let Err(code) = require_confirm(profile, reporter, confirmed) {
        return code;
    }
    let payload = match read_json_file(reporter, file) {
        Ok(payload) => payload,
        Err(code) => return code,
    };
    let client = match build_client(profile) {
        Ok(client) => client,
        Err(err) => return report_sdk_error(reporter, &err),
    };
    match client.post_items(payload).await {
        Ok(value) => {
            reporter.ok(&value, &value.to_string());
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}

pub async fn donate(profile: &Profile, reporter: &Reporter, target: &str, confirmed: bool) -> ExitCode {
    if let Err(code) = require_confirm(profile, reporter, confirmed) {
        return code;
    }
    let client = match build_client(profile) {
        Ok(client) => client,
        Err(err) => return report_sdk_error(reporter, &err),
    };
    match client.request_donation(target).await {
        Ok(()) => {
            let human = format!("requested a testnet donation to {target}");
            reporter.ok(&serde_json::json!({ "target": target }), &human);
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}

pub async fn tx_submit(profile: &Profile, reporter: &Reporter, file: &Path, confirmed: bool) -> ExitCode {
    if let Err(code) = require_confirm(profile, reporter, confirmed) {
        return code;
    }
    let payload = match read_json_file(reporter, file) {
        Ok(payload) => payload,
        Err(code) => return code,
    };
    // Accept either a single transaction object or an array of them.
    let txs: Vec<Value> = match payload {
        Value::Array(txs) => txs,
        other => vec![other],
    };
    let client = match build_client(profile) {
        Ok(client) => client,
        Err(err) => return report_sdk_error(reporter, &err),
    };
    match client.submit_transactions(&txs).await {
        Ok(value) => {
            reporter.ok(&value, &value.to_string());
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}

pub async fn tx_serialize(profile: &Profile, reporter: &Reporter, file: &Path) -> ExitCode {
    let payload = match read_json_file(reporter, file) {
        Ok(payload) => payload,
        Err(code) => return code,
    };
    let client = match build_client(profile) {
        Ok(client) => client,
        Err(err) => return report_sdk_error(reporter, &err),
    };
    match client.serialize_transactions(payload).await {
        Ok(value) => {
            reporter.ok(&value, &value.to_string());
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}

pub async fn tx_deserialize(profile: &Profile, reporter: &Reporter, file: &Path) -> ExitCode {
    let payload = match read_json_file(reporter, file) {
        Ok(payload) => payload,
        Err(code) => return code,
    };
    let client = match build_client(profile) {
        Ok(client) => client,
        Err(err) => return report_sdk_error(reporter, &err),
    };
    match client.deserialize_transactions(payload).await {
        Ok(value) => {
            reporter.ok(&value, &value.to_string());
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}
