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

/// RFC-0003 workflow: propose → confirm/reject → the next run reuses the decisions.
#[test]
fn correspondence_workflow_confirm_and_reject() {
    let dir = std::env::temp_dir().join(format!("pmef-diff-rfc3-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let proposals = dir.join("proposals.ndjson");
    let base_args = ["diff", DESIGN, SCAN, "--ignore", "revision", "--ignore-recognition", "--match", "--format", "json"];

    // 1. propose
    let (code, first, _) = pmef(&[&base_args[..], &["--emit-matches", proposals.to_str().unwrap()]].concat());
    assert_eq!(code, 1);
    let props: Vec<serde_json::Value> = std::fs::read_to_string(&proposals).unwrap().lines()
        .map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(props.len(), 5);
    assert!(props.iter().all(|p| p["changeType"] == "AS_BUILT" && p["review"]["status"] == "detected"));
    let (vcode, vout, _) = pmef(&["validate", proposals.to_str().unwrap(), "--schemas", "schemas"]);
    assert_eq!(vcode, 0, "proposals must be schema-valid:\n{vout}");

    // 2. a reviewer approves everything except the beam, which is rejected
    let decided: Vec<String> = props.iter().map(|p| {
        let mut p = p.clone();
        let reject = p["targetId"] == "urn:pmef:obj:demo-asb:B-101";
        p["review"] = serde_json::json!({"status": if reject { "rejected" } else { "approved" },
                                          "reviewedBy": "reviewer-01", "reviewedAt": "2026-10-31T12:00:00Z"});
        serde_json::to_string(&p).unwrap()
    }).collect();
    let scan_with = dir.join("asbuilt-scan-reviewed.ndjson");
    let scan_text = std::fs::read_to_string(root().join(SCAN)).unwrap();
    std::fs::write(&scan_with, scan_text + &decided.join("\n") + "\n").unwrap();

    // 3. rerun: approved pairs are declared, the rejected beam pair is no longer matched
    let args = ["diff", DESIGN, scan_with.to_str().unwrap(), "--ignore", "revision", "--ignore-recognition", "--match", "--format", "json"];
    let (code, second, _) = pmef(&args);
    assert_eq!(code, 1);
    let v: serde_json::Value = serde_json::from_str(&second).unwrap();
    let methods: Vec<(&str, &str)> = v["matches"].as_array().unwrap().iter()
        .map(|m| (m["baseId"].as_str().unwrap(), m["method"].as_str().unwrap())).collect();
    assert!(methods.iter().all(|(_, m)| *m == "declared"), "{methods:?}");
    assert_eq!(methods.len(), 4);
    let status = |id: &str| v["objects"].as_array().unwrap().iter()
        .find(|o| o["@id"] == id).map(|o| o["status"].as_str().unwrap().to_owned());
    assert_eq!(status("urn:pmef:obj:demo-asb:B-101").as_deref(), Some("removed"));
    assert_eq!(status("urn:pmef:obj:scan-2026-10:R-0002").as_deref(), Some("added"));
    // correspondence relationships are evidence, never reported as added objects
    assert!(v["objects"].as_array().unwrap().iter().all(|o| o["@type"] != "pmef:IsRevisionOf"));
    // the pipe deviation found in the first run is still found
    let first: serde_json::Value = serde_json::from_str(&first).unwrap();
    let pipe = |v: &serde_json::Value| v["objects"].as_array().unwrap().iter()
        .find(|o| o["@id"] == "urn:pmef:obj:demo-asb:CW-301-PIPE-002").unwrap()["changes"].clone();
    assert_eq!(pipe(&first), pipe(&v));
    std::fs::remove_dir_all(dir).ok();
}

// ── pmef merge (spec 03 §10) ─────────────────────────────────────────────────
// Regenerate after an intended change:
//   pmef merge tests/diff/design.ndjson tests/merge/ours-design-change.ndjson \
//     tests/merge/theirs-asbuilt-update.ndjson -o tests/merge/expected-merged.ndjson > tests/merge/expected-report.txt

const OURS: &str = "tests/merge/ours-design-change.ndjson";
const THEIRS: &str = "tests/merge/theirs-asbuilt-update.ndjson";

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pmef-merge-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("out.ndjson")
}

#[test]
fn merge_golden_with_conflicts() {
    let out = tmp("golden");
    let (code, report, _) = pmef(&["merge", DESIGN, OURS, THEIRS, "-o", out.to_str().unwrap()]);
    assert_eq!(code, 1, "conflicts must exit with 1");
    let expected_report = std::fs::read_to_string(root().join("tests/merge/expected-report.txt")).unwrap();
    // the report names the output path; compare everything else
    let strip = |s: &str| s.lines().filter(|l| !l.starts_with("Merged ")).collect::<Vec<_>>().join("\n");
    assert_eq!(strip(&report), strip(&expected_report));
    assert_eq!(std::fs::read_to_string(&out).unwrap(),
               std::fs::read_to_string(root().join("tests/merge/expected-merged.ndjson")).unwrap());
    let (vcode, vout, _) = pmef(&["validate", out.to_str().unwrap(), "--schemas", "schemas"]);
    assert_eq!(vcode, 0, "merged output must be schema-valid:\n{vout}");
}

#[test]
fn merge_prefer_theirs_and_symmetry() {
    let out = tmp("prefer");
    pmef(&["merge", DESIGN, OURS, THEIRS, "-o", out.to_str().unwrap(), "--prefer", "theirs"]);
    let merged: Vec<serde_json::Value> = std::fs::read_to_string(&out).unwrap().lines()
        .map(|l| serde_json::from_str(l).unwrap()).collect();
    let pipe = merged.iter().find(|o| o["@id"] == "urn:pmef:obj:demo-asb:CW-301-PIPE-002").unwrap();
    assert_eq!(pipe["pipeLength"], 2271.4);
    // swapping sides with --prefer theirs gives the same bytes as the default
    let swapped = tmp("swapped");
    pmef(&["merge", DESIGN, THEIRS, OURS, "-o", swapped.to_str().unwrap(), "--prefer", "theirs"]);
    let default = tmp("default");
    pmef(&["merge", DESIGN, OURS, THEIRS, "-o", default.to_str().unwrap()]);
    assert_eq!(std::fs::read_to_string(swapped).unwrap(), std::fs::read_to_string(default).unwrap());
}

#[test]
fn merge_clean_exits_zero_and_errors_exit_two() {
    let out = tmp("clean");
    // theirs == base: result equals ours, no conflicts
    let (code, _, _) = pmef(&["merge", DESIGN, OURS, DESIGN, "-o", out.to_str().unwrap()]);
    assert_eq!(code, 0);
    let (code, _, _) = pmef(&["diff", OURS, out.to_str().unwrap()]);
    assert_eq!(code, 0, "merge(base, ours, base) must equal ours");
    let (code, _, err) = pmef(&["merge", DESIGN, OURS, "tests/merge/missing.ndjson", "-o", out.to_str().unwrap()]);
    assert_eq!(code, 2);
    assert!(err.contains("Cannot open"), "{err}");
}

// ── pmef convert: AutoCAD Plant 3D via PCF ───────────────────────────────────

#[test]
fn pcf_round_trip_of_benchmark_is_lossless_and_detects_tool_edits() {
    let dir = std::env::temp_dir().join(format!("pmef-pcf-{}", std::process::id()));
    let pcf = dir.join("pcf");
    let back = dir.join("back.ndjson");
    let ds01 = "examples/pump-skid-complete.ndjson";
    let (code, _, err) = pmef(&["convert", ds01, "--to", "pcf", "-o", pcf.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("wrote 2 PCF file(s)"), "{err}");
    let (code, _, err) = pmef(&["convert", pcf.to_str().unwrap(), "--to", "pmef", "--base", ds01, "-o", back.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("updated 14, created 0, deleted 0"), "{err}");
    assert_eq!(pmef(&["diff", ds01, back.to_str().unwrap(), "--exact"]).0, 0, "unchanged round trip must be lossless");

    // simulate an edit in Plant 3D: move the start of pipe CW-202-PIPE-001 by 25 mm
    let file = pcf.join("6_-CW-202-A1A2.pcf");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("END-POINT 10200.0 5400.0 1250.0 150.0"));
    std::fs::write(&file, text.replacen("END-POINT 10200.0 5400.0 1250.0 150.0", "END-POINT 10200.0 5400.0 1225.0 150.0", 1)).unwrap();
    pmef(&["convert", pcf.to_str().unwrap(), "--to", "pmef", "--base", ds01, "-o", back.to_str().unwrap()]);
    let (code, out, _) = pmef(&["diff", ds01, back.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(out.contains("~ ports[portId=P1].coordinate[2]: 1250.0 → 1225.0 (Δ -25)"), "{out}");
    assert!(out.contains("~ pipeLength: 1200.0 → 1225.0 (Δ +25)"), "{out}");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn convert_to_pmef_requires_base() {
    let (code, _, err) = pmef(&["convert", "tests/diff", "--to", "pmef", "-o", "/dev/null"]);
    assert_ne!(code, 0);
    assert!(err.contains("--base"), "{err}");
}
