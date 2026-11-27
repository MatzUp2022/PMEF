//! # pmef-plant3d-wasm — PCF isometrics to PMEF in the browser
//!
//! One call, [`pcf_to_pmef`], runs the Plant 3D adapter on one PCF file (`PIPING-SPEC` becomes
//! the line `pipeClass`, `ITEM-CODE` becomes `catalogRef.catalogId`, a PMEF id in
//! `UNIQUE-COMPONENT-IDENTIFIER` stays the `@id`). Small on purpose: web apps that only need
//! this conversion (e.g. a material takeoff from isometrics) do not load a full converter.
//!
//! ```js
//! import init, { pcf_to_pmef } from "./pmef_plant3d_wasm.js";
//! await init();
//! const ndjson = pcf_to_pmef(pcfText); // throws on a file without components
//! ```
//!
//! Build: `cargo build -p pmef-plant3d-wasm --target wasm32-unknown-unknown --release`, then
//! `wasm-bindgen --target web` on the `.wasm`.

use wasm_bindgen::prelude::*;

/// PCF text → PMEF NDJSON. Errors if the file has no component (no `END-POINT`/`CO-ORDS`).
#[wasm_bindgen]
pub fn pcf_to_pmef(pcf: &str) -> Result<String, JsValue> {
    convert(pcf).map_err(|e| JsValue::from_str(&e))
}

fn convert(pcf: &str) -> Result<String, String> {
    if !pcf.contains("END-POINT") && !pcf.contains("CO-ORDS") {
        return Err("PCF without components (no END-POINT or CO-ORDS)".into());
    }
    let mut adapter = pmef_adapter_plant3d::Plant3DAdapter::new(Default::default());
    adapter.export_pcf_to_pmef(pcf).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::convert;

    #[test]
    fn converts_a_line_and_rejects_an_empty_file() {
        let pcf =
            "PIPELINE-REFERENCE L1\n    PIPING-SPEC A1A2\nPIPE\n    END-POINT 0 0 0 100\n    \
                   END-POINT 1000 0 0 100\n    ITEM-CODE A1A2-PIPE-100\n";
        let out = convert(pcf).unwrap();
        assert!(out.contains("\"pipeClass\":\"A1A2\""), "{out}");
        assert!(out.contains("A1A2-PIPE-100"), "{out}");
        assert!(convert("PIPELINE-REFERENCE L1\n").is_err());
    }
}
