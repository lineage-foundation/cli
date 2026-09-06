//! The `pay` command: send LNGX to an address using the profile's signer
//! backend (`Local` keystore or delegated to a `Node`), after guardrail
//! checks. `--dry-run` prints the intended action and makes no network
//! write.

use std::process::ExitCode;

use serde_json::json;

use lineage_sdk::{LocalSigner, NodeSigner, Receipt, Signer, Tokens, Wallet};

use super::{build_client, ok_code, report_sdk_error};
use crate::config::{Profile, SignerKind};
use crate::exit::Code;
use crate::guard;
use crate::output::Reporter;
use crate::secrets;

pub async fn run(profile: &Profile, reporter: &Reporter, to: &str, amount_lngx: f64, confirm_flag: bool, dry_run: bool) -> ExitCode {
    if let Err(denied) = guard::check(profile, to, amount_lngx, confirm_flag) {
        let code = Code::from(denied.clone());
        reporter.fail(code, &denied.to_string(), None);
        return code.into();
    }

    let amount_raw = Tokens::from_lngx(amount_lngx).0;

    if dry_run {
        let backend = match profile.signer {
            SignerKind::Local => "local",
            SignerKind::Node => "node",
        };
        let data = json!({
            "dry_run": true,
            "backend": backend,
            "to": to,
            "amount_lngx": amount_lngx,
            "amount_raw": amount_raw,
            "guardrail": "passed",
        });
        let human = format!(
            "dry run: would pay {amount_lngx} LNGX ({amount_raw} raw) to {to} via the {backend} backend"
        );
        reporter.ok(&data, &human);
        return ok_code();
    }

    let client = match build_client(profile) {
        Ok(client) => client,
        Err(err) => return report_sdk_error(reporter, &err),
    };

    let receipt = match profile.signer {
        SignerKind::Local => {
            let wallet_path = match profile.wallet_path.as_deref() {
                Some(path) => path,
                None => {
                    reporter.fail(Code::Usage, "profile has no wallet_path configured", None);
                    return Code::Usage.into();
                }
            };
            let passphrase = match secrets::passphrase(profile) {
                Ok(passphrase) => passphrase,
                Err(err) => {
                    reporter.fail(Code::Runtime, &err.to_string(), None);
                    return Code::Runtime.into();
                }
            };
            let mut wallet = match Wallet::open(wallet_path, &passphrase) {
                Ok(wallet) => wallet,
                Err(err) => return report_sdk_error(reporter, &err),
            };
            let change_address = match wallet.addresses().into_iter().next() {
                Some(address) => address,
                None => match wallet.new_address() {
                    Ok(address) => address,
                    Err(err) => return report_sdk_error(reporter, &err),
                },
            };
            let signer = LocalSigner::new(&client, &wallet, change_address);
            signer.pay(to, amount_raw).await
        }
        SignerKind::Node => {
            let passphrase = match secrets::passphrase(profile) {
                Ok(passphrase) => passphrase,
                Err(err) => {
                    reporter.fail(Code::Runtime, &err.to_string(), None);
                    return Code::Runtime.into();
                }
            };
            let signer = NodeSigner::new(&client, passphrase);
            signer.pay(to, amount_raw).await
        }
    };

    match receipt {
        Ok(receipt) => {
            emit_receipt(reporter, &receipt);
            ok_code()
        }
        Err(err) => report_sdk_error(reporter, &err),
    }
}

fn emit_receipt(reporter: &Reporter, receipt: &Receipt) {
    let data = json!({
        "tx_hash": receipt.tx_hash,
        "to_address": receipt.to_address,
        "amount_raw": receipt.amount,
        "amount_lngx": Tokens(receipt.amount).as_lngx(),
    });
    let human = format!(
        "sent {} LNGX to {} (tx {})\nview on the explorer: https://explorer.lineage.to/tx/{}",
        Tokens(receipt.amount).as_lngx(),
        receipt.to_address,
        receipt.tx_hash,
        receipt.tx_hash,
    );
    reporter.ok(&data, &human);
}
