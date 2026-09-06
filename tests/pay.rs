//! `pay` command: dry-run (no network write), the node backend happy path,
//! and the `max_amount` guardrail.
//!
//! `Config::load` resolves `~/.config/lineage/config.toml` via
//! `dirs::config_dir`, which in turn follows `$HOME`. Pointing `HOME` at a
//! tempdir for the child process gives each test an isolated config file
//! without touching the real user config.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Write a config file under `home/.config/lineage/config.toml` (Linux
/// layout) and `home/Library/Application Support/lineage/config.toml`
/// (macOS layout) so the test works regardless of which one `dirs`
/// resolves to on the host platform running the suite.
fn write_config(home: &Path, extra: &str) {
    let contents = format!(
        r#"
        default_profile = "paytest"

        [profiles.paytest]
        mempool = "http://127.0.0.1:1"
        storage = "http://127.0.0.1:1"
        miner = "http://127.0.0.1:1"
        signer = "node"
        confirm = "manual"
        {extra}
        "#
    );

    for dir in [
        home.join(".config").join("lineage"),
        home.join("Library").join("Application Support").join("lineage"),
    ] {
        fs::create_dir_all(&dir).expect("create config dir");
        fs::write(dir.join("config.toml"), &contents).expect("write config file");
    }
}

#[tokio::test]
async fn dry_run_prints_intent_and_makes_no_network_call() {
    let server = MockServer::start().await;
    // Deliberately mount no /v1/payments responder: any request to it would
    // be an unhandled-request panic from wiremock, proving a dry run makes
    // no network write.

    let home = tempfile::tempdir().expect("tempdir");
    write_config(home.path(), "");

    Command::cargo_bin("lineage")
        .unwrap()
        .env("HOME", home.path())
        .env("LINEAGE_PASSPHRASE", "test-passphrase")
        .args([
            "--json",
            "--network",
            &server.uri(),
            "--dry-run",
            "pay",
            "some-address",
            "10",
            "--yes",
        ])
        .assert()
        .success();

    assert_eq!(
        server.received_requests().await.unwrap().len(),
        0,
        "dry run must not hit the network"
    );
}

#[tokio::test]
async fn node_backend_happy_path_returns_a_receipt() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/payments"))
        .respond_with(ResponseTemplate::new(202).set_body_json(serde_json::json!({
            "to_address": "some-address",
            "amount": {"kind": "token", "amount": 720720000u64},
            "tx_hash": "gabc"
        })))
        .mount(&server)
        .await;

    let home = tempfile::tempdir().expect("tempdir");
    write_config(home.path(), "");

    let output = Command::cargo_bin("lineage")
        .unwrap()
        .env("HOME", home.path())
        .env("LINEAGE_PASSPHRASE", "test-passphrase")
        .args([
            "--json",
            "--network",
            &server.uri(),
            "pay",
            "some-address",
            "10",
            "--yes",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["ok"], true);
    assert_eq!(stdout["data"]["tx_hash"], "gabc");
    assert_eq!(stdout["data"]["amount_raw"], 720720000u64);
}

#[test]
fn max_amount_guardrail_denies_an_over_cap_payment() {
    let home = tempfile::tempdir().expect("tempdir");
    write_config(home.path(), "max_amount = 5.0");

    Command::cargo_bin("lineage")
        .unwrap()
        .env("HOME", home.path())
        .env("LINEAGE_PASSPHRASE", "test-passphrase")
        .args(["--json", "pay", "some-address", "10", "--yes"])
        .assert()
        .code(3);
}
