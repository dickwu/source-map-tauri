use std::fs;

use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;
use serde_json::Value;

fn binary() -> Command {
    Command::cargo_bin("source-map-tauri").expect("binary exists")
}

#[test]
fn help_lists_main_commands() {
    let mut cmd = binary();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Build a searchable source map for Tauri apps",
        ))
        .stdout(predicate::str::contains("scan"))
        .stdout(predicate::str::contains("reindex"))
        .stdout(predicate::str::contains("validate"));
}

#[test]
fn init_creates_default_config_and_ignore_file() {
    let temp = assert_fs::TempDir::new().expect("temp dir");

    let mut cmd = binary();
    cmd.env("HOME", temp.path());
    cmd.args(["init", "--root"])
        .arg(temp.path())
        .assert()
        .success();

    temp.child(".repo-search/tauri/source-map-tauri.toml")
        .assert(predicate::path::exists());
    temp.child(".repo-search/tauri/.gitignore")
        .assert(predicate::path::exists());
    temp.child(".config/meilisearch/connect.json")
        .assert(predicate::path::exists());
}

#[test]
fn scan_fixture_then_validate() {
    let temp = assert_fs::TempDir::new().expect("temp dir");
    let out = temp.child("out");

    let mut scan = binary();
    scan.args([
        "scan",
        "--root",
        "tests/fixtures/tauri-custom-plugin",
        "--repo",
        "fixture",
        "--out",
    ])
    .arg(out.path())
    .assert()
    .success();

    out.child("artifacts.ndjson")
        .assert(predicate::path::exists());
    out.child("edges.ndjson").assert(predicate::path::exists());
    out.child("warnings.ndjson")
        .assert(predicate::path::exists());
    out.child("summary.json").assert(predicate::path::exists());
    out.child("project-info.json")
        .assert(predicate::path::exists());

    let artifacts =
        fs::read_to_string(out.child("artifacts.ndjson").path()).expect("read artifacts");
    let artifact_kinds: Vec<String> = artifacts
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("valid json"))
        .filter_map(|value| value.get("kind").and_then(Value::as_str).map(str::to_owned))
        .collect();

    for kind in [
        "frontend_component",
        "frontend_hook_def",
        "frontend_hook_use",
        "tauri_invoke",
        "tauri_command",
        "tauri_plugin",
        "tauri_plugin_command",
        "tauri_plugin_lifecycle_hook",
        "tauri_permission",
        "tauri_capability",
        "tauri_capability_effective",
        "frontend_test",
        "rust_test",
    ] {
        assert!(
            artifact_kinds.iter().any(|candidate| candidate == kind),
            "missing kind {kind}"
        );
    }

    let mut validate = binary();
    validate.args(["validate", "--input"]).arg(out.path());
    validate.assert().success();
}

#[test]
fn scan_realish_inline_command_fixture() {
    let temp = assert_fs::TempDir::new().expect("temp dir");
    let out = temp.child("out");

    let mut scan = binary();
    scan.args([
        "scan",
        "--root",
        "tests/fixtures/tauri-inline-commands",
        "--repo",
        "realish",
        "--out",
    ])
    .arg(out.path())
    .assert()
    .success();

    let artifacts =
        fs::read_to_string(out.child("artifacts.ndjson").path()).expect("read artifacts");
    let docs: Vec<Value> = artifacts
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("valid json"))
        .collect();

    assert!(docs
        .iter()
        .any(|doc| doc.get("kind").and_then(Value::as_str) == Some("tauri_invoke")));
    assert!(docs
        .iter()
        .any(|doc| doc.get("kind").and_then(Value::as_str) == Some("tauri_command")));
    assert!(docs.iter().any(|doc| {
        doc.get("kind").and_then(Value::as_str) == Some("tauri_capability_effective")
    }));
    assert!(!docs.iter().any(|doc| {
        doc.get("kind").and_then(Value::as_str) == Some("frontend_hook_use")
            && matches!(
                doc.get("name").and_then(Value::as_str),
                Some("useState" | "useEffect" | "useMemo" | "useCallback")
            )
    }));

    let mut validate = binary();
    validate.args(["validate", "--input"]).arg(out.path());
    validate.assert().success();
}

#[test]
fn scan_frontend_http_flow_fixture() {
    let temp = assert_fs::TempDir::new().expect("temp dir");
    let out = temp.child("out");

    let mut scan = binary();
    scan.args([
        "scan",
        "--root",
        "tests/fixtures/frontend-http-flow",
        "--repo",
        "httpflow",
        "--out",
    ])
    .arg(out.path())
    .assert()
    .success();

    let artifacts =
        fs::read_to_string(out.child("artifacts.ndjson").path()).expect("read artifacts");
    let docs: Vec<Value> = artifacts
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("valid json"))
        .collect();

    assert!(docs.iter().any(|doc| {
        doc.get("kind").and_then(Value::as_str) == Some("frontend_api_wrapper")
            && doc.get("name").and_then(Value::as_str) == Some("useLogin")
            && doc.get("normalized_path").and_then(Value::as_str) == Some("/auth/login")
    }));
    assert!(docs.iter().any(|doc| {
        doc.get("kind").and_then(Value::as_str) == Some("frontend_transport")
            && doc.get("name").and_then(Value::as_str) == Some("usePostApi")
    }));
    assert!(docs.iter().any(|doc| {
        doc.get("kind").and_then(Value::as_str) == Some("frontend_http_endpoint")
            && doc.get("name").and_then(Value::as_str) == Some("/auth/login")
    }));

    let flow_docs: Vec<&Value> = docs
        .iter()
        .filter(|doc| {
            doc.get("kind").and_then(Value::as_str) == Some("frontend_http_flow")
                && doc.get("normalized_path").and_then(Value::as_str) == Some("/auth/login")
        })
        .collect();

    assert_eq!(
        flow_docs.len(),
        1,
        "expected exactly one flow for /auth/login"
    );
    let flow = flow_docs[0];
    assert_eq!(
        flow.get("primary_component").and_then(Value::as_str),
        Some("LoginModal")
    );
    assert_eq!(
        flow.get("primary_wrapper").and_then(Value::as_str),
        Some("useLogin")
    );
    assert_eq!(
        flow.get("primary_transport").and_then(Value::as_str),
        Some("usePostApi")
    );

    let suffix_flow_docs: Vec<&Value> = docs
        .iter()
        .filter(|doc| {
            doc.get("kind").and_then(Value::as_str) == Some("frontend_http_flow")
                && doc.get("normalized_path").and_then(Value::as_str)
                    == Some("/appointment/home/search")
        })
        .collect();

    assert_eq!(
        suffix_flow_docs.len(),
        1,
        "expected exactly one flow for /appointment/home/search"
    );
    let suffix_flow = suffix_flow_docs[0];
    assert_eq!(
        suffix_flow.get("primary_wrapper").and_then(Value::as_str),
        Some("useSearchPatientMutation")
    );
    assert_eq!(
        suffix_flow.get("primary_transport").and_then(Value::as_str),
        Some("usePostMutation")
    );
    assert_eq!(
        suffix_flow.get("caller_count").and_then(Value::as_u64),
        Some(4)
    );
    let path_aliases = suffix_flow
        .get("path_aliases")
        .and_then(Value::as_array)
        .expect("path aliases");
    assert!(path_aliases
        .iter()
        .any(|value| value.as_str() == Some("/home/search")));
    let alternate_components = suffix_flow
        .get("alternate_components")
        .and_then(Value::as_array)
        .expect("alternate components");
    assert!(alternate_components
        .iter()
        .any(|value| value.as_str() == Some("PatientSearch")));
    assert!(alternate_components
        .iter()
        .any(|value| value.as_str() == Some("SearchDefaultPanel")));
    assert!(
        suffix_flow.get("primary_component").and_then(Value::as_str) == Some("SearchWithHelper")
            || alternate_components
                .iter()
                .any(|value| value.as_str() == Some("SearchWithHelper"))
    );
    assert!(!alternate_components
        .iter()
        .any(|value| value.as_str() == Some("useSearchHelper")));

    let mut validate = binary();
    validate.args(["validate", "--input"]).arg(out.path());
    validate.assert().success();
}
