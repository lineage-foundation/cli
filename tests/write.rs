//! `items` and `donate` commands.
//!
//! `Config::load` resolves `~/.config/lineage/config.toml` via
//! `dirs::config_dir`, which in turn follows `$HOME`. Pointing `HOME` at a
//! tempdir for the child process gives each test an isolated config file
//! without touching the real user config.

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Write a config file under `home/.config/lineage/config.toml` (Linux
/// layout) and `home/Library/Application Support/lineage/config.toml`
/// (macOS layout) so the test works regardless of which one `dirs`
/// resolves to on the host platform running the suite.
fn write_config(home: &Path, extra: &str) {
    let contents = format!(
        r#"
        default_profile = "writetest"

        [profiles.writetest]
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

fn write_json_file(dir: &Path, name: &str, value: &serde_json::Value) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, serde_json::to_vec(value).unwrap()).expect("write json file");
    path
}

#[tokio::test]
async fn items_posts_the_file_body_to_v1_items() {
    let server = MockServer::start().await;
    let body = serde_json::json!({"kind": "block", "hash": "abc"});
    Mock::given(method("POST"))
        .and(path("/v1/items"))
        .and(body_json(&body))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"accepted": true})))
        .mount(&server)
        .await;

    let home = tempfile::tempdir().expect("tempdir");
    write_config(home.path(), "");
    let file = write_json_file(home.path(), "items.json", &body);

    let output = Command::cargo_bin("lineage")
        .unwrap()
        .env("HOME", home.path())
        .args([
            "--json",
            "--network",
            &server.uri(),
            "items",
            file.to_str().unwrap(),
            "--yes",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["ok"], true);
    assert_eq!(stdout["data"]["accepted"], true);
}

#[test]
fn items_without_confirmation_is_denied() {
    let home = tempfile::tempdir().expect("tempdir");
    write_config(home.path(), "");
    let body = serde_json::json!({"kind": "block"});
    let file = write_json_file(home.path(), "items.json", &body);

    Command::cargo_bin("lineage")
        .unwrap()
        .env("HOME", home.path())
        .args(["--json", "items", file.to_str().unwrap()])
        .assert()
        .code(3);
}

#[tokio::test]
async fn donate_requests_a_donation() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/donation-requests"))
        .and(body_json(serde_json::json!({"address": "some-target"})))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let home = tempfile::tempdir().expect("tempdir");
    write_config(home.path(), "");

    Command::cargo_bin("lineage")
        .unwrap()
        .env("HOME", home.path())
        .args([
            "--json",
            "--network",
            &server.uri(),
            "donate",
            "some-target",
            "--yes",
        ])
        .assert()
        .success();
}

#[test]
fn donate_without_confirmation_is_denied() {
    let home = tempfile::tempdir().expect("tempdir");
    write_config(home.path(), "");

    Command::cargo_bin("lineage")
        .unwrap()
        .env("HOME", home.path())
        .args(["--json", "donate", "some-target"])
        .assert()
        .code(3);
}
