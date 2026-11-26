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
//! Large models: a [`RouteSession`] holds the files and parses each one once (OBJ meshes with
//! their search tree, PMEF packages). Every [`RouteSession::route`] after the first reuses
//! them, so moving one line in a building model no longer re-reads the building:
//!
//! ```js
//! const s = new RouteSession();
//! s.addFile("hard.obj", hardObj);           // once
//! s.addFile("soft.obj", softObj);
//! let out = JSON.parse(s.route(JSON.stringify(problem), undefined));  // parses the meshes
//! problem.routes[0].to.point = [x, y, z];
//! out = JSON.parse(s.route(JSON.stringify(problem), undefined));      // reuses them
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
    let files: HashMap<String, String> = if files.trim().is_empty() {
        HashMap::new()
    } else {
        serde_json::from_str(files).map_err(|e| format!("files: {e}"))?
    };
    let mut session = RouteSession {
        files,
        ..RouteSession::default()
    };
    session.route_json(problem, plant)
}

/// Files plus their parsed content, kept across [`RouteSession::route`] calls.
///
/// [`RouteSession::add_file`] stores a file under the name the problem uses for it; the first
/// run that needs it parses it and drops the text (the parsed form is what stays in memory).
/// Adding a file again under the same name replaces it and its parsed form.
#[wasm_bindgen]
#[derive(Default)]
pub struct RouteSession {
    files: HashMap<String, String>,
    cache: pmef_route::SceneCache,
}

#[wasm_bindgen]
impl RouteSession {
    /// An empty session.
    #[wasm_bindgen(constructor)]
    pub fn new() -> RouteSession {
        RouteSession::default()
    }

    /// Add or replace the file `name` (as written in the problem: `pmefScene`, `obstacles[].obj`,
    /// `lineLists[].file`).
    #[wasm_bindgen(js_name = addFile)]
    pub fn add_file(&mut self, name: &str, content: String) {
        self.cache.invalidate(name);
        self.files.insert(name.to_string(), content);
    }

    /// Drop the file `name` and its parsed form.
    #[wasm_bindgen(js_name = removeFile)]
    pub fn remove_file(&mut self, name: &str) {
        self.cache.invalidate(name);
        self.files.remove(name);
    }

    /// Whether `name` was added (as text or already parsed).
    #[wasm_bindgen(js_name = hasFile)]
    pub fn has_file(&self, name: &str) -> bool {
        self.files.contains_key(name) || self.cache.contains(name)
    }

    /// As the free function [`route`], with the session's files.
    pub fn route(&mut self, problem: &str, plant: Option<String>) -> Result<String, JsError> {
        self.route_json(problem, plant.as_deref())
            .map_err(|e| JsError::new(&e))
    }
}

impl RouteSession {
    /// [`RouteSession::route`] without `wasm-bindgen` types (native tests).
    pub fn route_json(&mut self, problem: &str, plant: Option<&str>) -> Result<String, String> {
        let problem: pmef_route::RouteProblem =
            serde_json::from_str(problem).map_err(|e| format!("problem: {e}"))?;
        let files = &self.files;
        let read = |f: &str| {
            files
                .get(f)
                .cloned()
                .ok_or_else(|| format!("{f}: file not given"))
        };
        let out = pmef_route::run_cached(problem, plant, &read, &mut self.cache)
            .map_err(|e| e.to_string());
        // Parsed files are held by the cache; their text is no longer needed.
        let cache = &self.cache;
        self.files.retain(|name, _| !cache.contains(name));
        let out = out?;
        Ok(serde_json::json!({
            "ok": out.ok(),
            "ndjson": out.ndjson(),
            "report": out.report(),
        })
        .to_string())
    }
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

    #[test]
    fn session_parses_each_file_once() {
        let problem = |to_y: f64| {
            format!(
                r#"{{"project": "t", "revisionId": "r2026-11-26-001",
                "parent": "urn:pmef:unit:t:U1", "grid": {{"step": 100, "margin": 1500}},
                "obstacles": [{{"id": "B1", "obj": "box.obj"}}],
                "routes": [{{"id": "L1", "pipeClass": "A1A2", "dn": 100,
                    "from": {{"point": [0, 0, 0], "direction": [1, 0, 0]}},
                    "to": {{"point": [5000, {to_y}, 0], "direction": [0, -1, 0]}}}}]}}"#
            )
        };
        let far = "v 0 0 9000\nv 1 0 9000\nv 0 1 9000\nf 1 2 3\n";
        let mut s = RouteSession::new();
        s.add_file("box.obj", far.into());
        assert!(s.has_file("box.obj"));
        let a: serde_json::Value =
            serde_json::from_str(&s.route_json(&problem(3000.0), None).unwrap()).unwrap();
        assert_eq!(a["ok"], true);
        // Parsed: the text is gone, the mesh stays and serves the next run (terminal moved).
        assert!(s.files.is_empty());
        assert!(s.has_file("box.obj"));
        let b: serde_json::Value =
            serde_json::from_str(&s.route_json(&problem(4000.0), None).unwrap()).unwrap();
        assert_eq!(b["ok"], true);
        assert_ne!(a["ndjson"], b["ndjson"]);
        // Same result as the one-shot binding.
        let files = serde_json::json!({ "box.obj": far }).to_string();
        let one: serde_json::Value =
            serde_json::from_str(&route_json(&problem(4000.0), &files, None).unwrap()).unwrap();
        assert_eq!(one["ndjson"], b["ndjson"]);

        // Replacing the file replaces its parsed form: a box across the straight path.
        let wall = "v 2000 -2000 -2000\nv 2000 6000 -2000\nv 2000 6000 2000\nv 2000 -2000 2000\n\
                    f 1 2 3\nf 1 3 4\n";
        s.add_file("box.obj", wall.into());
        let c: serde_json::Value =
            serde_json::from_str(&s.route_json(&problem(4000.0), None).unwrap()).unwrap();
        assert_ne!(c["ndjson"], b["ndjson"]);

        s.remove_file("box.obj");
        assert!(!s.has_file("box.obj"));
        let err = s.route_json(&problem(4000.0), None).unwrap_err();
        assert!(err.contains("box.obj"), "{err}");
    }
}
