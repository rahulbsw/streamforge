use serde_json::Value;
use std::process::Command;

#[test]
fn validates_pipeline_crd_with_structured_diagnostics() {
    let output = Command::new(env!("CARGO_BIN_EXE_streamforge-validate"))
        .args([
            "--input-format",
            "pipeline-crd",
            "--output",
            "json",
            "tests/fixtures/pipeline-valid.yaml",
        ])
        .output()
        .expect("validator should execute");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("valid JSON report");
    assert_eq!(report["valid"], true);
    assert_eq!(report["inputFormat"], "pipeline-crd");
    assert_eq!(report["expressionsValidated"], 2);
    assert_eq!(report["diagnostics"], serde_json::json!([]));
}
