//! `pmef validate` / `pmef check`: unreadable lines, unknown @type (AUDIT P3.8) and the package
//! check that replaced `pmef conformance` (AUDIT P2.1).
//! Lines without a string @type/@id or with invalid JSON used to be dropped silently.

use std::path::PathBuf;
use std::process::Command;

fn pmef(args: &[&str]) -> (i32, String, String) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_pmef")).args(args).current_dir(root).output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap())
}

const PLANT: &str = r#"{"@type":"pmef:Plant","@id":"urn:pmef:plant:t:P","pmefVersion":"0.9.0","name":"P"}"#;

fn file(name: &str, lines: &[&str]) -> String {
    let dir = std::env::temp_dir().join(format!("pmef-validate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, lines.join("\n") + "\n").unwrap();
    p.to_str().unwrap().to_owned()
}

fn report(args: &[&str]) -> (i32, serde_json::Value) {
    let (code, out, _) = pmef(args);
    (code, serde_json::from_str(&out).unwrap())
}

#[test]
fn unreadable_lines_are_errors() {
    let f = file("unreadable.ndjson", &[
        PLANT,
        r#"{"@type":"pmef:Plant","pmefVersion":"0.9.0"}"#,
        r#"{"@type":"pmef:Plant","@id":123,"pmefVersion":"0.9.0"}"#,
        "not json",
    ]);
    let (code, r) = report(&["validate", &f, "--schemas", "schemas", "--format", "json"]);
    assert_eq!(code, 1);
    assert_eq!((r["total"].as_u64(), r["ok"].as_u64(), r["fail"].as_u64()), (Some(4), Some(1), Some(3)), "{r}");
    let errs = r["errors"].to_string();
    assert!(errs.contains("line 2: object has no string '@id'") && errs.contains("line 4: JSON parse error"), "{errs}");
}

#[test]
fn unknown_type_is_skipped_by_default_and_an_error_with_strict() {
    let f = file("unknown.ndjson", &[PLANT, r#"{"@type":"pmef:FutureThing","@id":"urn:pmef:obj:t:F"}"#]);
    let (code, r) = report(&["validate", &f, "--schemas", "schemas", "--format", "json"]);
    assert_eq!((code, r["skip"].as_u64(), r["fail"].as_u64()), (0, Some(1), Some(0)), "spec 06 CL1-03: {r}");
    let (code, r) = report(&["validate", &f, "--schemas", "schemas", "--format", "json", "--strict"]);
    assert_eq!((code, r["fail"].as_u64()), (1, Some(1)), "{r}");
}

#[test]
fn format_is_an_annotation() {
    // date-time is not asserted (Draft 2020-12 default, same as the Python reference)
    let obj = r#"{"@type":"pmef:Plant","@id":"urn:pmef:plant:t:P","pmefVersion":"0.9.0","name":"P","revision":{"revisionId":"r2026-11-01-001","changeState":"WIP","changedAt":"yesterday"}}"#;
    let f = file("format.ndjson", &[obj]);
    let (code, r) = report(&["validate", &f, "--schemas", "schemas", "--format", "json"]);
    assert_eq!(code, 0, "{r}");
}

const HEADER: &str = r#"{"@type":"pmef:FileHeader","@id":"urn:pmef:pkg:t:p","pmefVersion":"0.9.0","plantId":"urn:pmef:plant:t:P","projectCode":"T","coordinateSystem":"Z-up","units":"mm","changeState":"WIP"}"#;

fn check(args: &[&str]) -> (i32, serde_json::Value) {
    let (code, out, _) = pmef(args);
    (code, serde_json::from_str(&out).unwrap())
}

fn status(r: &serde_json::Value, name: &str) -> String {
    r["checks"].as_array().unwrap().iter().find(|c| c["check"] == name).unwrap()["status"].as_str().unwrap().to_owned()
}

#[test]
fn check_passes_reference_package() {
    let (code, r) = check(&["check", "examples/pump-skid-complete.ndjson", "--json"]);
    assert_eq!(code, 0, "{r}");
    assert_eq!(r["pmefVersion"], "0.9.0");
}

#[test]
fn check_finds_missing_id_duplicates_and_dangling_connections() {
    let pipe = |id: &str, to: &str| format!(
        r#"{{"@type":"pmef:Pipe","@id":"{id}","ports":[{{"portId":"P1","connectedTo":"{to}"}}]}}"#);
    let a = pipe("urn:pmef:obj:t:A", "urn:pmef:obj:t:MISSING");
    let b = pipe("urn:pmef:obj:t:A", "urn:pmef:external:t:BOUNDARY");
    let f = file("check.ndjson", &[HEADER, PLANT, r#"{"@type":"pmef:Plant","pmefVersion":"0.9.0"}"#, &a, &b]);
    let (code, r) = check(&["check", &f, "--json"]);
    assert_eq!(code, 1);
    assert_eq!(status(&r, "allObjectsHaveId"), "FAIL", "{r}");
    assert_eq!(status(&r, "idsUnique"), "FAIL", "{r}");
    assert_eq!(status(&r, "connectionsResolve"), "FAIL", "{r}");
    assert!(r.to_string().contains("t:MISSING") && !r.to_string().contains("BOUNDARY"), "external prefix is allowed: {r}");
}

#[test]
fn conformance_is_a_deprecated_alias_of_check() {
    let (code, out, err) = pmef(&["conformance", "examples/pump-skid-complete.ndjson", "--dataset", "ds01"]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("deprecated"), "{err}");
    assert!(serde_json::from_str::<serde_json::Value>(&out).is_ok(), "alias keeps JSON output");
}

#[test]
fn stats_fails_on_unreadable_line() {
    let f = file("stats.ndjson", &[PLANT, "not json"]);
    assert_ne!(pmef(&["stats", &f]).0, 0);
}
