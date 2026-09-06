use assert_cmd::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn supply_over_json_prints_the_stable_envelope() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/supply"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"total": 360360000000000000u64, "issued": 720720000u64})),
        )
        .mount(&server)
        .await;

    let output = Command::cargo_bin("lineage")
        .unwrap()
        .args(["--json", "--network", &server.uri(), "supply"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        stdout,
        serde_json::json!({
            "ok": true,
            "data": {"total": 360360000000000000u64, "issued": 720720000u64},
            "error": null
        })
    );
}

#[test]
fn supply_against_an_unreachable_host_exits_network_error() {
    Command::cargo_bin("lineage")
        .unwrap()
        .args(["--json", "--network", "http://127.0.0.1:1", "supply"])
        .assert()
        .code(4);
}
