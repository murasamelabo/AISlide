use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn request_returns_only_json_on_stdout() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aislide"))
        .arg("request").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(br#"{"op":"sample"}"#).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["sections"].as_array().unwrap().len(), 12);
}

#[test]
fn generate_does_not_overwrite_an_existing_destination() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.json");
    let output = directory.path().join("report.pptx");
    std::fs::write(&input, serde_json::to_vec(&aislide_core::report::sample_report()).unwrap()).unwrap();
    let first = Command::new(env!("CARGO_BIN_EXE_aislide")).arg("generate").arg(&input).arg(&output).output().unwrap();
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));
    let original = std::fs::read(&output).unwrap();
    let second = Command::new(env!("CARGO_BIN_EXE_aislide")).arg("generate").arg(&input).arg(&output).output().unwrap();
    assert!(!second.status.success());
    assert_eq!(std::fs::read(output).unwrap(), original);
}