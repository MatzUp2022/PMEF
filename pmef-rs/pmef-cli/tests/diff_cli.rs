//! `pmef diff` end-to-end: golden output, determinism, exit codes (spec 03 §9).
//!
//! Regenerate the golden files after an intended output change:
//!   pmef diff tests/diff/design.ndjson tests/diff/asbuilt.ndjson --ignore revision --ignore-recognition > tests/diff/expected-asbuilt.txt
//!   pmef diff tests/diff/design.ndjson tests/diff/asbuilt.ndjson --ignore revision --format json > tests/diff/expected-full.json
//!   pmef diff tests/diff/design.ndjson tests/diff/asbuilt-scan.ndjson --ignore revision --ignore-recognition --match > tests/diff/expected-match.txt
//!   (same with --format json > tests/diff/expected-match.json)

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pmef(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_pmef")).args(args).current_dir(root()).output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap())
}

const DESIGN: &str = "tests/diff/design.ndjson";
const ASBUILT: &str = "tests/diff/asbuilt.ndjson";

#[test]
fn golden_as_built_vs_design_text() {
    let (code, out, _) = pmef(&["diff", DESIGN, ASBUILT, "--ignore", "revision", "--ignore-recognition"]);
    assert_eq!(code, 1, "differences must exit with 1");
    assert_eq!(out, std::fs::read_to_string(root().join("tests/diff/expected-asbuilt.txt")).unwrap());
}

#[test]
fn golden_full_json() {
    let (code, out, _) = pmef(&["diff", DESIGN, ASBUILT, "--ignore", "revision", "--format", "json"]);
    assert_eq!(code, 1);
    assert_eq!(out, std::fs::read_to_string(root().join("tests/diff/expected-full.json")).unwrap());
}

#[test]
fn output_is_independent_of_object_order() {
    // Reverse the object order of the head file: output must be byte-identical.
    let text = std::fs::read_to_string(root().join(ASBUILT)).unwrap();
    let mut lines: Vec<&str> = text.lines().filter(|l| l.starts_with('{')).collect();
    lines.reverse();
    let dir = std::env::temp_dir().join(format!("pmef-diff-order-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let reversed = dir.join("asbuilt-reversed.ndjson");
    std::fs::write(&reversed, lines.join("\n") + "\n").unwrap();
    let a = pmef(&["diff", DESIGN, ASBUILT, "--format", "json"]);
    let b = pmef(&["diff", DESIGN, reversed.to_str().unwrap(), "--format", "json"]);
    assert_eq!(a, b);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn exit_codes() {
    assert_eq!(pmef(&["diff", DESIGN, DESIGN]).0, 0, "identical files exit 0");
    let (code, _, err) = pmef(&["diff", DESIGN, "tests/diff/does-not-exist.ndjson"]);
    assert_eq!(code, 2, "errors exit 2");
    assert!(err.contains("Cannot open"), "{err}");
}

#[test]
fn duplicate_ids_are_an_error() {
    let dir = std::env::temp_dir().join(format!("pmef-diff-dup-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dup = dir.join("dup.ndjson");
    let line = r#"{"@type":"pmef:Plant","@id":"urn:pmef:plant:x:P","pmefVersion":"0.9.0","name":"P"}"#;
    std::fs::write(&dup, format!("{line}\n{line}\n")).unwrap();
    let (code, _, err) = pmef(&["diff", dup.to_str().unwrap(), DESIGN]);
    assert_eq!(code, 2);
    assert!(err.contains("duplicate @id"), "{err}");
    std::fs::remove_dir_all(dir).ok();
}

const SCAN: &str = "tests/diff/asbuilt-scan.ndjson";

#[test]
fn golden_matching_with_recogniser_ids() {
    let args = ["diff", DESIGN, SCAN, "--ignore", "revision", "--ignore-recognition", "--match"];
    let (code, out, _) = pmef(&args);
    assert_eq!(code, 1);
    assert_eq!(out, std::fs::read_to_string(root().join("tests/diff/expected-match.txt")).unwrap());
    let (_, json, _) = pmef(&[&args[..], &["--format", "json"]].concat());
    assert_eq!(json, std::fs::read_to_string(root().join("tests/diff/expected-match.json")).unwrap());
}

#[test]
fn matching_reports_the_same_plant_deviations_as_identical_ids() {
    // Recogniser ids + --match must find the same field changes as the identical-id case,
    // apart from memberMark, which the recogniser does not know.
    let changes = |file: &str, extra: &[&str]| -> Vec<(String, String)> {
        let args = [&["diff", DESIGN, file, "--ignore", "revision", "--ignore-recognition", "--format", "json"][..], extra].concat();
        let v: serde_json::Value = serde_json::from_str(&pmef(&args).1).unwrap();
        v["objects"].as_array().unwrap().iter()
            .flat_map(|o| o["changes"].as_array().cloned().unwrap_or_default().into_iter()
                .map(move |c| (o["@id"].as_str().unwrap().to_owned(), c["path"].as_str().unwrap().to_owned())))
            .filter(|(_, p)| p != "memberMark")
            .collect()
    };
    assert_eq!(changes(ASBUILT, &[]), changes(SCAN, &["--match"]));
}

#[test]
fn without_match_differing_ids_are_added_and_removed() {
    let v: serde_json::Value =
        serde_json::from_str(&pmef(&["diff", DESIGN, SCAN, "--format", "json"]).1).unwrap();
    assert_eq!(v["summary"]["added"], 6);
    assert_eq!(v["summary"]["removed"], 6);
}
