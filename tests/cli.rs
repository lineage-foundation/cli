use assert_cmd::Command;

#[test]
fn prints_help_and_version() {
    Command::cargo_bin("lineage").unwrap().arg("--help").assert().success();
    Command::cargo_bin("lineage").unwrap().arg("--version").assert().success();
}
