//! `wallet new` + `wallet address` against a local keystore.
//!
//! `Config::load` resolves `~/.config/lineage/config.toml` via
//! `dirs::config_dir`, which in turn follows `$HOME`. Pointing `HOME` at a
//! tempdir for the child process gives each test an isolated config file
//! (and therefore an isolated `wallet_path`) without touching the real
//! user config.

use std::fs;
use std::path::PathBuf;

use assert_cmd::Command;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Write a config file under `home/.config/lineage/config.toml` (Linux
/// layout) and `home/Library/Application Support/lineage/config.toml`
/// (macOS layout) so the test works regardless of which one `dirs`
/// resolves to on the host platform running the suite.
fn write_config(home: &std::path::Path, wallet_path: &std::path::Path) {
    let contents = format!(
        r#"
        default_profile = "wallettest"

        [profiles.wallettest]
        mempool = "http://127.0.0.1:1"
        storage = "http://127.0.0.1:1"
        miner = "http://127.0.0.1:1"
        signer = "local"
        confirm = "manual"
        wallet_path = "{}"
        "#,
        wallet_path.display()
    );

    for dir in [
        home.join(".config").join("lineage"),
        home.join("Library").join("Application Support").join("lineage"),
    ] {
        fs::create_dir_all(&dir).expect("create config dir");
        fs::write(dir.join("config.toml"), &contents).expect("write config file");
    }
}

#[test]
fn wallet_new_then_address_produces_a_keystore_and_a_hex_address() {
    let home = tempfile::tempdir().expect("tempdir");
    let wallet_path: PathBuf = home.path().join("wallet.json");
    write_config(home.path(), &wallet_path);

    Command::cargo_bin("lineage")
        .unwrap()
        .env("HOME", home.path())
        .env("LINEAGE_PASSPHRASE", "test-passphrase")
        .args(["--json", "wallet", "new"])
        .assert()
        .success();

    assert!(wallet_path.exists(), "keystore file should have been created");

    let output = Command::cargo_bin("lineage")
        .unwrap()
        .env("HOME", home.path())
        .env("LINEAGE_PASSPHRASE", "test-passphrase")
        .args(["--json", "wallet", "address"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["ok"], true);
    assert!(stdout["error"].is_null());

    let address = stdout["data"]["address"].as_str().expect("address string");
    assert_eq!(address.len(), 64, "address should be a 64-char hex string");
    assert!(address.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn wallet_new_creates_a_missing_parent_directory() {
    let home = tempfile::tempdir().expect("tempdir");
    // wallet_path lives in a subdirectory that does not exist yet.
    let wallet_path: PathBuf = home.path().join("nested").join("dir").join("wallet.json");
    write_config(home.path(), &wallet_path);

    Command::cargo_bin("lineage")
        .unwrap()
        .env("HOME", home.path())
        .env("LINEAGE_PASSPHRASE", "test-passphrase")
        .args(["--json", "wallet", "new"])
        .assert()
        .success();

    assert!(wallet_path.exists(), "keystore should be created with its parent dir");
}

#[test]
fn wallet_new_refuses_to_overwrite_without_force() {
    let home = tempfile::tempdir().expect("tempdir");
    let wallet_path: PathBuf = home.path().join("wallet.json");
    write_config(home.path(), &wallet_path);

    let run_new = |extra: &[&str]| {
        let mut args = vec!["--json", "wallet", "new"];
        args.extend_from_slice(extra);
        Command::cargo_bin("lineage")
            .unwrap()
            .env("HOME", home.path())
            .env("LINEAGE_PASSPHRASE", "test-passphrase")
            .args(&args)
            .assert()
    };

    // First create succeeds.
    run_new(&[]).success();
    // Second create without --force is denied (exit code 3), keystore preserved.
    run_new(&[]).failure().code(3);
    // With --force it replaces the keystore.
    run_new(&["--force"]).success();
}

#[tokio::test]
async fn wallet_refresh_posts_addresses_to_the_running_total_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/wallet/running-total:refresh"))
        .and(body_json(serde_json::json!({"addresses": ["addr-a"]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"running_total": 42})))
        .mount(&server)
        .await;

    let home = tempfile::tempdir().expect("tempdir");
    let wallet_path: PathBuf = home.path().join("wallet.json");
    write_config(home.path(), &wallet_path);

    let output = Command::cargo_bin("lineage")
        .unwrap()
        .env("HOME", home.path())
        .args([
            "--json",
            "--network",
            &server.uri(),
            "wallet",
            "refresh",
            "miner",
            "addr-a",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["ok"], true);
    assert_eq!(stdout["data"]["running_total"], 42);
}
