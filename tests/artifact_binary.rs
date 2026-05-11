use std::process::Command;

#[test]
fn artifact_from_derives_name_from_positional_source() {
    let output = Command::new(env!("CARGO_BIN_EXE_artifact"))
        .args([
            "from",
            "https://github.com/HackerNews/API",
            "--goal",
            "give agents focused access to top stories",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["workerName"], "hackernews-api-worker");
    assert_eq!(
        value["functions"][0]["functionId"],
        "hackernews_api::top_stories"
    );
}

#[test]
fn artifact_catalog_prints_reusable_worker_catalog() {
    let output = Command::new(env!("CARGO_BIN_EXE_artifact"))
        .arg("catalog")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value["engineBuiltins"]
        .as_array()
        .unwrap()
        .iter()
        .any(|worker| worker["name"] == "iii-state"));
    assert!(value["installableWorkers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|worker| worker["name"] == "iii-database"));
}
