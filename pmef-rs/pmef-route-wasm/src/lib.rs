//! # pmef-route-wasm — the routing kernel in the browser
//!
//! One call, [`route`], runs a whole `pmef route` problem (pipes, branches, supports, cables,
//! P&ID line lists) without a file system: the files the problem names (`pmefScene`,
//! `obstacles[].obj`, `lineLists[].file`) are passed in as a JSON object, file name as written
//! in the problem → content. Viewers (PlantView `mvcache-web`, ProDocAimAi) re-route
//! interactively by changing the problem (terminals, `pmefIgnore` for the line being moved)
//! and calling [`route`] again.
//!
//! ```js
//! import init, { route } from "./pmef_route_wasm.js";
//! await init();
//! const out = JSON.parse(route(JSON.stringify(problem), JSON.stringify({"skid.ndjson": text}),
//!                              "urn:pmef:plant:demo:P1"));
//! // out.ok, out.ndjson (PMEF), out.report (as written by `pmef route --report`)
//! ```
//!
//! Build: `cargo build -p pmef-route-wasm --target wasm32-unknown-unknown --release`, then
//! `wasm-bindgen --target web` on the `.wasm` (or `wasm-pack build pmef-route-wasm --target web`).

use std::collections::HashMap;
use wasm_bindgen::prelude::*;

/// Route `problem` (problem JSON as for `pmef route`) with the files in `files`
/// (`{"<name>": "<content>"}`); `plant` adds a `pmef:FileHeader`. Returns
/// `{"ok": bool, "ndjson": "<PMEF NDJSON>", "report": …}`.
#[wasm_bindgen]
pub fn route(problem: &str, files: &str, plant: Option<String>) -> Result<String, JsError> {
    route_json(problem, files, plant.as_deref()).map_err(|e| JsError::new(&e))
}

/// The binding without `wasm-bindgen` types (native tests).
pub fn route_json(problem: &str, files: &str, plant: Option<&str>) -> Result<String, String> {
    let problem: pmef_route::RouteProblem =
        serde_json::from_str(problem).map_err(|e| format!("problem: {e}"))?;
    let files: HashMap<String, String> = if files.trim().is_empty() {
        HashMap::new()
    } else {
        serde_json::from_str(files).map_err(|e| format!("files: {e}"))?
    };
    let read = |f: &str| {
        files
            .get(f)
            .cloned()
            .ok_or_else(|| format!("{f}: file not given"))
    };
    let out = pmef_route::run(problem, plant, &read).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "ok": out.ok(),
        "ndjson": out.ndjson(),
        "report": out.report(),
    })
    .to_string())
}

/// Version of the routing kernel (`pmef-route x.y.z`).
#[wasm_bindgen]
pub fn version() -> String {
    pmef_route::GENERATOR.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_a_problem_with_in_memory_files() {
        let problem = r#"{"project": "t", "revisionId": "r2026-11-25-001",
            "parent": "urn:pmef:unit:t:U1", "grid": {"step": 100, "margin": 1500},
            "obstacles": [{"id": "B1", "obj": "box.obj"}],
            "routes": [{"id": "L1", "pipeClass": "A1A2", "dn": 100,
                "from": {"point": [0, 0, 0], "direction": [1, 0, 0]},
                "to": {"point": [5000, 3000, 0], "direction": [0, -1, 0]}}]}"#;
        // Unit cube far from the route as an OBJ "file".
        let obj = "v 0 0 9000\nv 1 0 9000\nv 0 1 9000\nf 1 2 3\n";
        let files = serde_json::json!({ "box.obj": obj }).to_string();
        let out: serde_json::Value = serde_json::from_str(
            &route_json(problem, &files, Some("urn:pmef:plant:t:P1")).unwrap(),
        )
        .unwrap();
        assert_eq!(out["ok"], true);
        let ndjson = out["ndjson"].as_str().unwrap();
        assert!(ndjson.starts_with(r#"{"@type":"pmef:FileHeader""#));
        assert!(ndjson.contains("pmef:Elbow"));
        assert_eq!(out["report"][0]["id"], "L1");

        let err = route_json(problem, "{}", None).unwrap_err();
        assert!(err.contains("box.obj"), "{err}");
    }
}
