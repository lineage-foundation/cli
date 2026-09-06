//! Read-only commands: `supply`, `balance`, `blocks`, `entries`, `tx
//! status`, `mining`, `debug`. Each resolves an SDK client from the
//! profile, calls a read method, and emits through the `Reporter` — a
//! human line (LNGX where the payload is a token amount) or the JSON
//! envelope (raw units preserved in `data`).

use std::process::ExitCode;

use serde_json::{json, Value};

use lineage_sdk::{NodeClass, Tokens};

use super::{build_client, ok_code, report_sdk_error};
use crate::cli::NodeArg;
use crate::config::Profile;
use crate::output::Reporter;

pub async fn supply(profile: &Profile, reporter: &Reporter) -> ExitCode {
    let client = match build_client(profile) {
        Ok(c) => c,
        Err(err) => return report_sdk_error(reporter, &err),
    };
    match client.supply().await {
        Ok(supply) => {
            let data = json!({ "total": supply.total, "issued": supply.issued });
            let human = format!(
                "total: {} LNGX, issued: {} LNGX",
                Tokens(supply.total).as_lngx(),
                Tokens(supply.issued).as_lngx()
            );
            reporter.ok(&data, &human);
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}

pub async fn balance(profile: &Profile, reporter: &Reporter, addresses: &[String]) -> ExitCode {
    let client = match build_client(profile) {
        Ok(c) => c,
        Err(err) => return report_sdk_error(reporter, &err),
    };
    let refs: Vec<&str> = addresses.iter().map(String::as_str).collect();
    match client.balances(&refs).await {
        Ok(resp) => {
            let address_list: Value = resp
                .balance
                .address_list
                .iter()
                .map(|(addr, utxos)| {
                    let utxos: Vec<Value> = utxos
                        .iter()
                        .map(|u| {
                            json!({
                                "out_point": { "n": u.out_point.n, "t_hash": u.out_point.t_hash },
                                "value": u.value,
                            })
                        })
                        .collect();
                    (addr.clone(), Value::Array(utxos))
                })
                .collect::<serde_json::Map<String, Value>>()
                .into();
            let data = json!({
                "balance": {
                    "address_list": address_list,
                    "total": { "tokens": resp.balance.total.tokens, "items": resp.balance.total.items },
                }
            });
            let human = format!(
                "total: {} LNGX across {} address(es)",
                Tokens(resp.balance.total.tokens).as_lngx(),
                addresses.len()
            );
            reporter.ok(&data, &human);
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}

pub async fn blocks(profile: &Profile, reporter: &Reporter, target: Option<&str>, nums: &[u64]) -> ExitCode {
    let client = match build_client(profile) {
        Ok(c) => c,
        Err(err) => return report_sdk_error(reporter, &err),
    };

    let result = if !nums.is_empty() {
        client.blocks(nums).await
    } else {
        match target {
            None | Some("latest") => client.latest_block().await,
            Some(num) => match num.parse::<u64>() {
                Ok(num) => client.block_by_num(num).await,
                Err(_) => {
                    reporter.fail(
                        crate::exit::Code::Usage,
                        &format!("invalid block target: {num}"),
                        None,
                    );
                    return crate::exit::Code::Usage.into();
                }
            },
        }
    };

    match result {
        Ok(value) => {
            reporter.ok(&value, &value.to_string());
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}

pub async fn entries(profile: &Profile, reporter: &Reporter, key: &str) -> ExitCode {
    let client = match build_client(profile) {
        Ok(c) => c,
        Err(err) => return report_sdk_error(reporter, &err),
    };
    match client.blockchain_entry(key).await {
        Ok(value) => {
            reporter.ok(&value, &value.to_string());
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}

pub async fn tx_status(profile: &Profile, reporter: &Reporter, hash: &str) -> ExitCode {
    let client = match build_client(profile) {
        Ok(c) => c,
        Err(err) => return report_sdk_error(reporter, &err),
    };
    match client.transaction_status(hash).await {
        Ok(statuses) => {
            let data: Value = statuses
                .iter()
                .map(|(hash, status)| {
                    (
                        hash.clone(),
                        json!({
                            "status": status.status,
                            "timestamp": status.timestamp,
                            "additional_info": status.additional_info,
                        }),
                    )
                })
                .collect::<serde_json::Map<String, Value>>()
                .into();
            let human = statuses
                .get(hash)
                .map(|s| format!("{hash}: {}", s.status))
                .unwrap_or_else(|| format!("{hash}: unknown"));
            reporter.ok(&data, &human);
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}

pub async fn mining(profile: &Profile, reporter: &Reporter) -> ExitCode {
    let client = match build_client(profile) {
        Ok(c) => c,
        Err(err) => return report_sdk_error(reporter, &err),
    };
    match client.current_mining_block().await {
        Ok(value) => {
            reporter.ok(&value, &value.to_string());
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}

pub async fn debug(profile: &Profile, reporter: &Reporter, node: NodeArg) -> ExitCode {
    let client = match build_client(profile) {
        Ok(c) => c,
        Err(err) => return report_sdk_error(reporter, &err),
    };
    let class: NodeClass = node.into();
    match client.debug(class).await {
        Ok(data) => {
            let value = json!({
                "node_type": data.node_type,
                "node_api": data.node_api,
                "node_peers": data.node_peers,
            });
            let human = format!(
                "{} — {} api route(s), {} peer(s)",
                data.node_type,
                data.node_api.len(),
                data.node_peers.len()
            );
            reporter.ok(&value, &human);
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}
