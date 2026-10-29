//! End-to-end CLI contracts, including paths received from JSON on Windows.
use serde_json::{json, Value};
use std::process::Command;
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
