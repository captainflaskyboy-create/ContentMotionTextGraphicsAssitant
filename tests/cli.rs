//! End-to-end CLI contracts, including paths received from JSON on Windows.
use serde_json::{json, Value};
use std::process::Command;
#[test]
fn standalone_licenses_are_embedded() {
    let output = Command::new(env!("CARGO_BIN_EXE_cmtga"))
        .arg("licenses")
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    let notices = result["licenses"].as_str().unwrap();
    assert!(notices.contains("Copyright (c) 2026 Captain Flasky Boy"));
    assert!(notices.contains("AIContentEditorAssistant contributors"));
    assert!(notices.contains("third-party"));
    assert!(notices.contains("Apache License"));
    assert!(notices.contains("Press Start 2P"));
    assert!(!notices.contains(env!("CARGO_MANIFEST_DIR")));
}

#[test]
fn kinetic_cli_schema_frame_and_mixed_batch() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = tempfile::tempdir_in(root.join("target")).unwrap();
    let request = dir.path().join("kinetic.json");
    std::fs::write(&request,serde_json::to_vec(&json!({"kind":"kinetic","version":1,"canvas":{"width":320,"height":180},"words":[{"word":"hello","start":0.,"end":0.2},{"word":"world","start":0.25,"end":0.5,"emphasis":true}]})).unwrap()).unwrap();
    let executable = env!("CARGO_BIN_EXE_cmtga");
    let schema = Command::new(executable)
        .args(["schema", "--kind", "kinetic"])
        .output()
        .unwrap();
    assert!(schema.status.success());
    let schema: Value = serde_json::from_slice(&schema.stdout).unwrap();
    assert!(schema["properties"]["words"].is_object());
    for command in ["validate", "kinetic", "render", "frame"] {
        let mut process = Command::new(executable);
        process.args([command, "--request"]).arg(&request);
        if command != "validate" {
            process.arg("--output").arg(dir.path().join(format!(
                "{command}.{}",
                if command == "frame" { "png" } else { "mp4" }
            )));
        }
        if matches!(command, "kinetic" | "render") {
            process.args(["--acceleration", "software"]);
        }
        if command == "frame" {
            process.args(["--time", "0.3"]);
        }
        let result = process.output().unwrap();
        let value: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert!(result.status.success(), "{command}: {value}");
        if matches!(command, "kinetic" | "render") {
            assert_eq!(value["verification"]["frames"], 30);
            assert_eq!(value["kinetic"]["font_source"], "embedded");
        }
    }
    let scene = dir.path().join("scene.json");
    std::fs::write(&scene,serde_json::to_vec(&json!({"version":1,"duration":0.5,"canvas":{"width":320,"height":180},"layers":[{"id":"box","start":0,"duration":0.5,"x":10,"y":10,"width":100,"height":100,"content":{"type":"rectangle","color":"yellow"}}]})).unwrap()).unwrap();
    let batch = dir.path().join("batch.json");
    std::fs::write(&batch,serde_json::to_vec(&json!({"version":1,"jobs":[{"request":"scene.json","output":"scene.mp4"},{"request":"kinetic.json","output":"words.mp4"}]})).unwrap()).unwrap();
    let result = Command::new(executable)
        .args(["batch", "--request"])
        .arg(batch)
        .args(["--acceleration", "software"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
}

#[test]
fn batch_relative_paths_unicode_stdout_and_overwrite() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = tempfile::tempdir_in(root.join("target")).unwrap();
    let request = directory.path().join("scene ü.json");
    let scene = json!({"version":1,"duration":0.6,"canvas":{"width":320,"height":180},"layers":[{"id":"title","start":0,"duration":0.6,"x":100,"y":200,"width":1720,"height":600,"content":{"type":"text","text":"UNICODE ✓","font_size":100}}]});
    std::fs::write(&request, serde_json::to_vec(&scene).unwrap()).unwrap();
    let batch = directory.path().join("batch.json");
    std::fs::write(
        &batch,
        serde_json::to_vec(
            &json!({"version":1,"jobs":[{"request":"scene ü.json","output":"finished title.mp4"}]}),
        )
        .unwrap(),
    )
    .unwrap();
    let executable = env!("CARGO_BIN_EXE_cmtga");
    let output = Command::new(executable)
        .args(["batch", "--request"])
        .arg(&batch)
        .args(["--acceleration", "software"])
        .output()
        .unwrap();
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(output.status.success(), "{result}");
    assert_eq!(result["jobs"][0]["verification"]["frames"], 18);
    let conflicting = Command::new(executable)
        .args(["render", "--request"])
        .arg(&request)
        .arg("--output")
        .arg(directory.path().join("finished title.mp4"))
        .output()
        .unwrap();
    assert_eq!(conflicting.status.code(), Some(3));
    let error: Value = serde_json::from_slice(&conflicting.stdout).unwrap();
    assert_eq!(error["error"]["code"], "output_exists");
}
