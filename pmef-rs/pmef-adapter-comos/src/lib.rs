//! # pmef-adapter-comos
//!
//! PMEF adapter for **Siemens COMOS** Engineering Data Management.
//!
//! ## COMOS and PMEF
//!
//! COMOS is an engineering database that stores the complete plant information
//! model — from the P&ID through to commissioning documentation. It covers:
//!
//! - **Equipment** (`@E` classes): pumps, vessels, HX, reactors, tanks
//! - **Piping** (`@L` classes): lines, specifications, insulation
//! - **Instruments** (`@I` classes): transmitters, controllers, valves, SIS
//! - **Electrical** (`@K` classes): cables, cable trays, MCC
//! - **Control systems** (`@S` classes): PLC/DCS, I/O modules, HMI
//! - **Documents** (`@D` classes): P&IDs, datasheets, loop diagrams
//!
//! PMEF covers the 3D physical model. COMOS covers the engineering
//! attributes and P&ID. Together they provide full P&ID-to-3D integration
//! via `HasEquivalentIn` relationships keyed on COMOS CUIDs.
//!
//! ## Architecture
//!
//! Like the Tekla adapter, COMOS uses a two-component design:
//! 1. **C# exporter** (`ComosExporter.cs`) — reads COMOS via .NET API
//!    and writes a structured JSON export.
//! 2. **Rust processor** (this crate) — maps the JSON to PMEF NDJSON.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use pmef_adapter_comos::{ComosAdapter, ComosConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = ComosConfig {
//!         project_code: "eaf-2026".to_owned(),
//!         export_path: "comos-export.json".into(),
//!         ..Default::default()
//!     };
//!     let mut adapter = ComosAdapter::new(config);
//!     let stats = adapter.export_to_pmef("output.ndjson").await?;
//!     Ok(())
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod export_schema;
pub mod mapping;

pub use export_schema::*;
pub use mapping::{
    barg_to_pa_abs, comos_class_to_equipment, comos_class_to_instrument,
    comos_class_to_plc, comos_material_to_pmef, degc_to_k, kw_to_w,
    loop_type_from_number,
};

use pmef_core::traits::{AdapterError, AdapterStats, PmefAdapter};
use std::collections::HashMap;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the COMOS adapter.
#[derive(Debug, Clone)]
pub struct ComosConfig {
    /// PMEF project code for @id generation.
    pub project_code: String,
    /// Path to the COMOS JSON export file.
    pub export_path: PathBuf,
    /// Export equipment objects. Default: true.
    pub include_equipment: bool,
    /// Export piping lines. Default: true.
    pub include_piping: bool,
    /// Export instruments + loops. Default: true.
    pub include_instruments: bool,
    /// Export cables. Default: true.
    pub include_cables: bool,
    /// Export PLC/control system objects. Default: true.
    pub include_plc: bool,
    /// Export document references. Default: false.
    pub include_documents: bool,
}

impl Default for ComosConfig {
    fn default() -> Self {
        Self {
            project_code: "proj".to_owned(),
            export_path: PathBuf::from("comos-export.json"),
            include_equipment: true,
            include_piping: true,
            include_instruments: true,
            include_cables: true,
            include_plc: true,
            include_documents: false,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Adapter
// ─────────────────────────────────────────────────────────────────────────────

/// COMOS → PMEF adapter.
pub struct ComosAdapter {
    config: ComosConfig,
    /// COMOS CUID → PMEF @id (for cross-reference resolution).
    cuid_to_id: HashMap<String, String>,
}

impl ComosAdapter {
    pub fn new(config: ComosConfig) -> Self {
        Self { config, cuid_to_id: HashMap::new() }
    }

    /// Export the COMOS plant model to PMEF NDJSON.
    pub async fn export_to_pmef(
        &mut self,
        output_path: &str,
    ) -> Result<AdapterStats, AdapterError> {
        use pmef_io::{NdjsonWriter, WriterConfig};
        use std::fs::File;
        use std::io::BufWriter;

        let t0 = std::time::Instant::now();
        let mut stats = AdapterStats::default();

        let json_text = std::fs::read_to_string(&self.config.export_path)
            .map_err(AdapterError::Io)?;
        let export: ComosExport = serde_json::from_str(&json_text)
            .map_err(|e| AdapterError::Json(e))?;

        tracing::info!(
            "Loaded COMOS export: {} equipment, {} instruments, {} lines from '{}'",
            export.equipment.len(), export.instruments.len(),
            export.piping_lines.len(), export.project_name
        );

        // Pre-register all CUIDs
        self.register_cuids(&export);

        let file = File::create(output_path).map_err(AdapterError::Io)?;
        let mut writer = NdjsonWriter::new(BufWriter::new(file), WriterConfig::default());

        // Header objects
        for obj in self.make_header_objects(&export) {
            writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
            stats.objects_ok += 1;
        }

        // Plant units
        for unit in &export.plant_units {
            let obj = self.map_unit(unit);
            writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
            stats.objects_ok += 1;
        }

        // Equipment
        if self.config.include_equipment {
            for equip in &export.equipment {
                for obj in self.map_equipment(equip) {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
        }

        // Piping lines
        if self.config.include_piping {
            for line in &export.piping_lines {
                for obj in self.map_piping_line(line) {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
        }

        // Instruments + loops
        if self.config.include_instruments {
            for inst in &export.instruments {
                for obj in self.map_instrument(inst) {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
            for lp in &export.instrument_loops {
                for obj in self.map_loop(lp) {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
        }

        // Cables
        if self.config.include_cables {
            for cable in &export.cables {
                for obj in self.map_cable(cable) {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
        }

        // PLC objects
        if self.config.include_plc {
            for plc in &export.plc_objects {
                for obj in self.map_plc(plc) {
                    writer.write_value(&obj).map_err(|e| AdapterError::Json(e.into()))?;
                    stats.objects_ok += 1;
                }
            }
        }

        writer.flush().map_err(AdapterError::Io)?;
        stats.duration_ms = t0.elapsed().as_millis() as u64;
        tracing::info!(
            "COMOS export complete: {} ok, {} failed in {}ms",
            stats.objects_ok, stats.objects_failed, stats.duration_ms
        );
        Ok(stats)
    }

    // ── CUID registration ─────────────────────────────────────────────────────

    fn register_cuids(&mut self, export: &ComosExport) {
        for e in &export.equipment {
            let id = self.equipment_id(&e.tag_number, &e.cuid);
            self.cuid_to_id.insert(e.cuid.clone(), id);
        }
        for i in &export.instruments {
            let id = self.instrument_id(&i.tag_number, &i.cuid);
            self.cuid_to_id.insert(i.cuid.clone(), id);
        }
        for l in &export.piping_lines {
            let id = self.line_id(&l.line_number);
            self.cuid_to_id.insert(l.cuid.clone(), id);
        }
        for lp in &export.instrument_loops {
            let id = self.loop_id(&lp.loop_number);
            self.cuid_to_id.insert(lp.cuid.clone(), id);
        }
        for plc in &export.plc_objects {
            let id = self.plc_id(&plc.tag_number, &plc.cuid);
            self.cuid_to_id.insert(plc.cuid.clone(), id);
        }
        for cable in &export.cables {
            let id = self.cable_id(&cable.cable_number, &cable.cuid);
            self.cuid_to_id.insert(cable.cuid.clone(), id);
        }
    }

    fn resolve_cuid(&self, cuid: &str) -> Option<&str> {
        self.cuid_to_id.get(cuid).map(|s| s.as_str())
    }

    // ── @id helpers ───────────────────────────────────────────────────────────

    fn clean(s: &str) -> String {
        s.chars().filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_')).collect()
    }

    fn equipment_id(&self, tag: &str, _cuid: &str) -> String {
        format!("urn:pmef:obj:{}:{}", self.config.project_code, Self::clean(tag))
    }
    fn instrument_id(&self, tag: &str, _cuid: &str) -> String {
        format!("urn:pmef:obj:{}:{}", self.config.project_code, Self::clean(tag))
    }
    fn line_id(&self, number: &str) -> String {
        format!("urn:pmef:line:{}:{}", self.config.project_code, Self::clean(number))
    }
    fn loop_id(&self, number: &str) -> String {
        format!("urn:pmef:loop:{}:{}", self.config.project_code, Self::clean(number))
    }
    fn plc_id(&self, tag: &str, _cuid: &str) -> String {
        format!("urn:pmef:obj:{}:{}", self.config.project_code, Self::clean(tag))
    }
    fn cable_id(&self, number: &str, _cuid: &str) -> String {
        format!("urn:pmef:obj:{}:CABLE-{}", self.config.project_code, Self::clean(number))
    }
    fn unit_id(&self, name: &str) -> String {
        format!("urn:pmef:unit:{}:{}", self.config.project_code, Self::clean(name))
    }

    fn make_has_equivalent_in(&self, pmef_id: &str, comos_cuid: &str) -> serde_json::Value {
        let local = pmef_id.split(':').last().unwrap_or("obj");
        serde_json::json!({
            "@type": "pmef:HasEquivalentIn",
            "@id": format!("urn:pmef:rel:{}:{local}-comos", self.config.project_code),
            "relationType": "HAS_EQUIVALENT_IN",
            "sourceId": pmef_id,
            "targetId": pmef_id,
            "targetSystem": "COMOS",
            "targetSystemId": comos_cuid,
            "mappingType": "EXACT",
            "derivedBy": "ADAPTER_IMPORT",
            "confidence": 1.0,
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED",
                          "authoringTool":"pmef-adapter-comos 0.9.0" }
        })
    }

    // ── Header ────────────────────────────────────────────────────────────────

    fn make_header_objects(&self, export: &ComosExport) -> Vec<serde_json::Value> {
        let pc = &self.config.project_code;
        let proj_clean = Self::clean(&export.project_name);
        let plant_id = format!("urn:pmef:plant:{pc}:{proj_clean}");
        vec![
            serde_json::json!({
                "@type": "pmef:FileHeader",
                "@id": format!("urn:pmef:pkg:{pc}:{proj_clean}"),
                "pmefVersion": "0.9.0",
                "plantId": plant_id,
                "projectCode": pc,
                "coordinateSystem": "Z-up",
                "units": "mm",
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringTool": format!("pmef-adapter-comos 0.9.0 / COMOS {}", export.comos_version)
            }),
            serde_json::json!({
                "@type": "pmef:Plant",
                "@id": plant_id,
                "pmefVersion": "0.9.0",
                "name": export.project_name,
                "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED" }
            }),
        ]
    }

    fn map_unit(&self, unit: &ComosUnit) -> serde_json::Value {
        let uid = self.unit_id(&unit.name);
        serde_json::json!({
            "@type": "pmef:Unit",
            "@id": uid,
            "pmefVersion": "0.9.0",
            "name": unit.name,
            "isPartOf": format!("urn:pmef:plant:{}:plant", self.config.project_code),
            "unitNumber": unit.cuid,
            "revision": { "revisionId":"r2026-01-01-001","changeState":"SHARED" }
        })
    }

    // ── Equipment ─────────────────────────────────────────────────────────────

    fn map_equipment(&self, equip: &ComosEquipment) -> Vec<serde_json::Value> {
        let obj_id = self.equipment_id(&equip.tag_number, &equip.cuid);
        let (pmef_type, equip_class) = comos_class_to_equipment(&equip.comos_class);

        let unit_id = self.unit_id("U-100"); // simplified; full impl uses unit_cuid lookup
        let d = &equip.design_attrs;

        // Map nozzles
        let nozzles: Vec<serde_json::Value> = equip.nozzles.iter().map(|noz| {
            let conn_line = noz.connected_line_cuid.as_deref()
                .and_then(|c| self.resolve_cuid(c));
            serde_json::json!({
                "nozzleId": noz.nozzle_mark,
                "nozzleMark": noz.nozzle_mark,
                "service": noz.service,
                "nominalDiameter": noz.nominal_diameter_mm.unwrap_or(100.0),
                "flangeRating": noz.flange_rating.as_deref().unwrap_or("ANSI-150"),
                "facingType": noz.facing_type.as_deref().unwrap_or("RF"),
                "coordinate": [0.0, 0.0, 0.0],  // position not in COMOS — from 3D tool
                "direction": [0.0, 0.0, 1.0],
                "connectedLineId": conn_line
            })
        }).collect();

        // Build document links
        let docs: Vec<serde_json::Value> = equip.documents.iter().map(|doc| {
            serde_json::json!({
                "documentId": doc.document_cuid,
                "documentType": doc.document_type,
                "revision": doc.revision
            })
        }).collect();

        let obj = serde_json::json!({
            "@type": pmef_type,
            "@id": obj_id,
            "pmefVersion": "0.9.0",
            "isPartOf": unit_id,
            "equipmentBasic": {
                "tagNumber": equip.tag_number,
                "equipmentClass": equip_class,
                "serviceDescription": equip.description,
                "designCode": d.design_code,
                "manufacturer": d.manufacturer,
                "model": d.model
            },
            "nozzles": nozzles,
            "iec81346": {
                "functionalAspect": equip.iec81346_functional,
                "productAspect": equip.iec81346_product
            },
            "pidSheetRef": equip.pid_reference,
            "documents": docs,
            "customAttributes": {
                "comosCuid": equip.cuid,
                "comosClass": equip.comos_class,
                "status": equip.status,
                "designPressure_Pa": d.design_pressure_barg.map(barg_to_pa_abs),
                "designTemperature_K": d.design_temperature_degc.map(degc_to_k),
                "designTemperatureMin_K": d.design_temperature_min_degc.map(degc_to_k),
                "operatingPressure_Pa": d.operating_pressure_barg.map(barg_to_pa_abs),
                "operatingTemperature_K": d.operating_temperature_degc.map(degc_to_k),
                "volume_m3": d.volume_m3,
                "material": d.material.as_deref().map(comos_material_to_pmef),
                "weightEmpty_kg": d.weight_empty_kg,
                "weightOperating_kg": d.weight_operating_kg,
                "motorPower_W": d.motor_power_kw.map(kw_to_w),
                "designFlow_m3h": d.design_flow_m3h,
                "designHead_m": d.design_head_m,
                "heatDuty_W": d.heat_duty_kw.map(kw_to_w),
                "heatTransferArea_m2": d.heat_transfer_area_m2,
                "temaType": d.tema_type,
                "insideDiameter_mm": d.inside_diameter_mm,
                "tangentLength_mm": d.tangent_length_mm,
                "shellSidePressure_Pa": d.shell_side_pressure_barg.map(barg_to_pa_abs),
                "tubeSidePressure_Pa": d.tube_side_pressure_barg.map(barg_to_pa_abs)
            },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringToolObjectId": equip.cuid,
                "authoringTool": "pmef-adapter-comos 0.9.0"
            }
        });

        vec![obj, self.make_has_equivalent_in(&obj_id, &equip.cuid)]
    }

    // ── Piping line ───────────────────────────────────────────────────────────

    fn map_piping_line(&self, line: &ComotLine) -> Vec<serde_json::Value> {
        let line_id = self.line_id(&line.line_number);
        let unit_id = self.unit_id("U-100");

        let design_conds = serde_json::json!({
            "designPressure":       line.design_pressure_barg.map(barg_to_pa_abs),
            "designTemperature":    line.design_temperature_degc.map(degc_to_k),
            "operatingPressure":    line.operating_pressure_barg.map(barg_to_pa_abs),
            "operatingTemperature": line.operating_temperature_degc.map(degc_to_k),
            "testPressure":         line.test_pressure_barg.map(barg_to_pa_abs),
            "testMedium": "WATER",
            "vacuumService": false
        });

        let spec = serde_json::json!({
            "nominalDiameter":  line.nominal_diameter_mm.unwrap_or(100.0),
            "pipeClass":        line.pipe_class,
            "material":         line.material.as_deref().map(comos_material_to_pmef),
            "pressureRating":   "ANSI-150",
            "corrosionAllowance": 3.0,
            "insulationType":   line.insulation_type.as_deref().unwrap_or("NONE"),
            "heatTracingType":  line.heat_tracing
        });

        let obj = serde_json::json!({
            "@type": "pmef:PipingNetworkSystem",
            "@id": line_id,
            "pmefVersion": "0.9.0",
            "lineNumber": line.line_number,
            "nominalDiameter": line.nominal_diameter_mm,
            "pipeClass": line.pipe_class,
            "mediumCode": line.medium_code,
            "mediumDescription": line.medium_description,
            "fluidPhase": "LIQUID",
            "isPartOf": unit_id,
            "designConditions": design_conds,
            "specification": spec,
            "segments": [],
            "iec81346": { "functionalAspect": line.iec81346_functional },
            "pidSheetRef": line.pid_reference,
            "customAttributes": {
                "comosCuid": line.cuid,
                "status": line.status
            },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringToolObjectId": line.cuid,
                "authoringTool": "pmef-adapter-comos 0.9.0"
            }
        });

        vec![obj, self.make_has_equivalent_in(&line_id, &line.cuid)]
    }

    // ── Instrument ────────────────────────────────────────────────────────────

    fn map_instrument(&self, inst: &ComosInstrument) -> Vec<serde_json::Value> {
        let obj_id = self.instrument_id(&inst.tag_number, &inst.cuid);
        let m = comos_class_to_instrument(&inst.comos_class);
        let d = &inst.design_attrs;
        let unit_id = self.unit_id("U-100");

        let measured_range = match (d.range_min, d.range_max, d.range_unit.as_deref()) {
            (Some(min), Some(max), Some(unit)) =>
                serde_json::json!({"min": min, "max": max, "unit": unit}),
            _ => serde_json::Value::Null,
        };

        let safety_spec = match d.sil_level {
            Some(sil) if sil > 0 => serde_json::json!({
                "safetyIntegrityLevel": sil,
                "safetyFunction": "ESD",
                "architectureType": d.architecture.as_deref().unwrap_or("1oo1"),
                "pfd": d.pfd,
                "pfh": d.pfh,
                "proofTestInterval": d.proof_test_interval_months,
                "safeState": d.safe_state
            }),
            _ => serde_json::Value::Null,
        };

        let conn_spec = serde_json::json!({
            "signalType": d.signal_type,
            "failSafe": d.fail_safe,
            "loopPowered": false,
            "intrinsicSafe": d.intrinsic_safe,
            "hazardousArea": d.hazardous_area,
            "ipRating": d.ip_rating
        });

        let obj = serde_json::json!({
            "@type": m.pmef_type,
            "@id": obj_id,
            "pmefVersion": "0.9.0",
            "isPartOf": unit_id,
            "tagNumber": inst.tag_number,
            "instrumentClass": m.instrument_class,
            "serviceDescription": inst.class_description,
            "processVariable": m.process_variable.or(d.process_variable.as_deref()),
            "loopNumber": inst.loop_cuid.as_deref()
                .and_then(|c| self.resolve_cuid(c))
                .map(|id| id.split(':').last().unwrap_or("").to_owned()),
            "measuredRange": if measured_range.is_null() { None } else { Some(measured_range) },
            "safetySpec": if safety_spec.is_null() { None } else { Some(safety_spec) },
            "connectionSpec": conn_spec,
            "iec81346": {
                "functionalAspect": inst.iec81346_functional,
                "productAspect": inst.iec81346_product
            },
            "pidSheetRef": inst.pid_reference,
            "comosCuid": inst.cuid,
            "tiaPLCAddress": d.tia_plc_address,
            "eplanBkz": d.eplan_function_text,
            "customAttributes": {
                "comosClass": inst.comos_class,
                "status": inst.status,
                "kvValue": d.kv_value,
                "shutoffClass": d.shutoff_class,
                "actuatorType": d.actuator_type,
                "manufacturer": d.manufacturer,
                "model": d.model
            },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringToolObjectId": inst.cuid,
                "authoringTool": "pmef-adapter-comos 0.9.0"
            }
        });

        vec![obj, self.make_has_equivalent_in(&obj_id, &inst.cuid)]
    }

    // ── Instrument loop ───────────────────────────────────────────────────────

    fn map_loop(&self, lp: &ComosLoop) -> Vec<serde_json::Value> {
        let loop_id = self.loop_id(&lp.loop_number);
        let unit_id = self.unit_id("U-100");

        let member_ids: Vec<String> = lp.member_cuids.iter()
            .filter_map(|c| self.resolve_cuid(c))
            .map(|s| s.to_owned())
            .collect();

        let controller_id = lp.controller_cuid.as_deref()
            .and_then(|c| self.resolve_cuid(c));
        let final_elem_id = lp.final_element_cuid.as_deref()
            .and_then(|c| self.resolve_cuid(c));

        let loop_type = if lp.loop_type.is_empty() || lp.loop_type == "UNKNOWN" {
            loop_type_from_number(&lp.loop_number)
        } else {
            &lp.loop_type
        };

        let obj = serde_json::json!({
            "@type": "pmef:InstrumentLoop",
            "@id": loop_id,
            "loopNumber": lp.loop_number,
            "loopType": loop_type,
            "isPartOf": unit_id,
            "memberIds": member_ids,
            "controllerTagId": controller_id,
            "finalElementTagId": final_elem_id,
            "silLevel": lp.sil_level,
            "iec81346": { "functionalAspect": lp.iec81346_functional },
            "pidSheetRef": lp.pid_reference,
            "customAttributes": {
                "comosCuid": lp.cuid,
                "status": lp.status
            },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringToolObjectId": lp.cuid,
                "authoringTool": "pmef-adapter-comos 0.9.0"
            }
        });

        vec![obj, self.make_has_equivalent_in(&loop_id, &lp.cuid)]
    }

    // ── Cable ─────────────────────────────────────────────────────────────────

    fn map_cable(&self, cable: &ComosCable) -> Vec<serde_json::Value> {
        let cable_id = self.cable_id(&cable.cable_number, &cable.cuid);
        let unit_id  = self.unit_id("U-100");

        let from_id = cable.from_cuid.as_deref().and_then(|c| self.resolve_cuid(c));
        let to_id   = cable.to_cuid.as_deref().and_then(|c| self.resolve_cuid(c));

        let obj = serde_json::json!({
            "@type": "pmef:CableObject",
            "@id": cable_id,
            "isPartOf": unit_id,
            "cableNumber": cable.cable_number,
            "cableType": cable.cable_type.as_deref().unwrap_or("POWER"),
            "crossSection": cable.cross_section_mm2.unwrap_or(1.5),
            "numberOfCores": cable.number_of_cores.unwrap_or(3),
            "fromId": from_id.unwrap_or("urn:pmef:obj:unknown:from"),
            "toId":   to_id.unwrap_or("urn:pmef:obj:unknown:to"),
            "routeLength": cable.route_length_m.map(|l| l * 1000.0), // m → mm
            "iec81346": { "productAspect": cable.iec81346_product },
            "customAttributes": {
                "comosCuid": cable.cuid,
                "voltageRating_V": cable.voltage_rating_v,
                "cableTrayId": cable.cable_tray_cuid
            },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringToolObjectId": cable.cuid,
                "authoringTool": "pmef-adapter-comos 0.9.0"
            }
        });

        vec![obj, self.make_has_equivalent_in(&cable_id, &cable.cuid)]
    }

    // ── PLC ───────────────────────────────────────────────────────────────────

    fn map_plc(&self, plc: &ComosPlc) -> Vec<serde_json::Value> {
        let plc_id  = self.plc_id(&plc.tag_number, &plc.cuid);
        let unit_id = self.unit_id("U-100");
        let plc_class = comos_class_to_plc(&plc.comos_class);

        let obj = serde_json::json!({
            "@type": "pmef:PLCObject",
            "@id": plc_id,
            "isPartOf": unit_id,
            "plcClass": plc_class,
            "vendor": plc.vendor.as_deref().unwrap_or("Siemens"),
            "family": plc.family.as_deref().unwrap_or("S7-1500"),
            "articleNumber": plc.article_number,
            "rack": plc.rack,
            "slot": plc.slot,
            "ipAddress": plc.ip_address,
            "safetyCpu": plc.safety_cpu.unwrap_or(false),
            "amlRef": plc.aml_ref,
            "iec81346": { "productAspect": plc.iec81346_product },
            "customAttributes": {
                "comosCuid": plc.cuid,
                "comosClass": plc.comos_class,
                "tiaportalRef": plc.tia_portal_ref
            },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringToolObjectId": plc.cuid,
                "authoringTool": "pmef-adapter-comos 0.9.0"
            }
        });

        vec![obj, self.make_has_equivalent_in(&plc_id, &plc.cuid)]
    }
}

impl PmefAdapter for ComosAdapter {
    fn name(&self) -> &str { "pmef-adapter-comos" }
    fn version(&self) -> &str { "0.9.0" }
    fn target_system(&self) -> &str { "COMOS" }
    fn supported_domains(&self) -> &[&str] {
        &["equipment", "piping", "ei", "cables", "plc"]
    }
    fn conformance_level(&self) -> u8 { 2 }
    fn description(&self) -> &str {
        "Siemens COMOS → PMEF adapter. Maps engineering attributes from COMOS \
         (equipment datasheets, piping line data, instrument loop data, cables, PLCs) \
         to PMEF. The 3D position data comes from the 3D tool adapter (E3D/Plant3D). \
         Level 2 conformance."
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ComosConfig {
        ComosConfig {
            project_code: "test".to_owned(),
            export_path: PathBuf::from("nonexistent.json"),
            ..Default::default()
        }
    }

    fn mock_pump() -> ComosEquipment {
        ComosEquipment {
            cuid: "CUID-P-201A".to_owned(),
            tag_number: "P-201A".to_owned(),
            comos_class: "@E03".to_owned(),
            class_description: "Centrifugal pump".to_owned(),
            description: Some("Cooling water pump".to_owned()),
            unit_cuid: "CUID-UNIT-001".to_owned(),
            pid_reference: Some("P&ID-U100-001".to_owned()),
            status: Some("ACTIVE".to_owned()),
            iec81346_functional: Some("=U100.M01.A".to_owned()),
            iec81346_product: Some("-P201A".to_owned()),
            nozzles: vec![
                ComosNozzle {
                    cuid: "CUID-NOZZ-001".to_owned(),
                    nozzle_mark: "N1".to_owned(),
                    service: Some("Suction".to_owned()),
                    nominal_diameter_mm: Some(200.0),
                    flange_rating: Some("ANSI-150".to_owned()),
                    facing_type: Some("RF".to_owned()),
                    connected_line_cuid: None,
                    iec81346: None,
                }
            ],
            design_attrs: ComosEquipmentDesign {
                design_pressure_barg: Some(15.0),
                design_temperature_degc: Some(60.0),
                operating_pressure_barg: Some(5.0),
                operating_temperature_degc: Some(30.0),
                material: Some("CS".to_owned()),
                design_code: Some("API 610".to_owned()),
                motor_power_kw: Some(55.0),
                design_flow_m3h: Some(250.0),
                design_head_m: Some(45.0),
                weight_empty_kg: Some(1650.0),
                weight_operating_kg: Some(1850.0),
                manufacturer: Some("Flowserve".to_owned()),
                model: Some("LCSA 200-315".to_owned()),
                ..Default::default()
            },
            raw_attrs: Default::default(),
            documents: vec![],
        }
    }

    fn mock_line() -> ComotLine {
        ComotLine {
            cuid: "CUID-LINE-201".to_owned(),
            line_number: "8\"-CW-201-A1A2".to_owned(),
            unit_cuid: "CUID-UNIT-001".to_owned(),
            description: Some("Cooling water supply".to_owned()),
            nominal_diameter_mm: Some(200.0),
            pipe_class: Some("A1A2".to_owned()),
            medium_code: Some("CW".to_owned()),
            medium_description: Some("Cooling water".to_owned()),
            design_pressure_barg: Some(15.0),
            design_temperature_degc: Some(60.0),
            operating_pressure_barg: Some(5.0),
            operating_temperature_degc: Some(30.0),
            test_pressure_barg: Some(22.5),
            material: Some("A106B".to_owned()),
            insulation_type: Some("NONE".to_owned()),
            heat_tracing: None,
            pid_reference: Some("P&ID-U100-001".to_owned()),
            iec81346_functional: None,
            status: None,
            raw_attrs: Default::default(),
        }
    }

    fn mock_instrument() -> ComosInstrument {
        ComosInstrument {
            cuid: "CUID-FIC-10101".to_owned(),
            tag_number: "FIC-10101".to_owned(),
            comos_class: "@I10.F".to_owned(),
            class_description: "Flow transmitter".to_owned(),
            unit_cuid: "CUID-UNIT-001".to_owned(),
            loop_cuid: Some("CUID-LOOP-101".to_owned()),
            pid_reference: Some("P&ID-U100-001".to_owned()),
            iec81346_functional: Some("=U100.FIC101".to_owned()),
            iec81346_product: Some("-FIC10101".to_owned()),
            status: Some("ACTIVE".to_owned()),
            design_attrs: ComosInstrumentDesign {
                process_variable: Some("FLOW".to_owned()),
                range_min: Some(0.0),
                range_max: Some(300.0),
                range_unit: Some("m3/h".to_owned()),
                signal_type: Some("HART".to_owned()),
                fail_safe: Some("FO".to_owned()),
                sil_level: Some(1),
                proof_test_interval_months: Some(12),
                pfd: Some(1.2e-3),
                architecture: Some("1oo1".to_owned()),
                safe_state: Some("CLOSED".to_owned()),
                intrinsic_safe: Some(false),
                hazardous_area: Some("Zone 1".to_owned()),
                ip_rating: Some("IP66".to_owned()),
                manufacturer: Some("Endress+Hauser".to_owned()),
                model: Some("Promag 53".to_owned()),
                ..Default::default()
            },
            raw_attrs: Default::default(),
            documents: vec![],
        }
    }

    #[test]
    fn test_map_equipment_pump() {
        let mut adapter = ComosAdapter::new(test_config());
        let pump = mock_pump();
        adapter.cuid_to_id.insert(pump.cuid.clone(),
            adapter.equipment_id(&pump.tag_number, &pump.cuid));
        let objs = adapter.map_equipment(&pump);
        assert_eq!(objs.len(), 2);
        let eq = &objs[0];
        assert_eq!(eq["@type"], "pmef:Pump");
        assert_eq!(eq["equipmentBasic"]["tagNumber"], "P-201A");
        assert_eq!(eq["equipmentBasic"]["equipmentClass"], "CENTRIFUGAL_PUMP");
        assert_eq!(eq["equipmentBasic"]["designCode"], "API 610");
        // Design pressure
        let dp = eq["customAttributes"]["designPressure_Pa"].as_f64().unwrap();
        assert!((dp - barg_to_pa_abs(15.0)).abs() < 1.0, "Got {dp}");
        // Motor power
        let pw = eq["customAttributes"]["motorPower_W"].as_f64().unwrap();
        assert!((pw - 55_000.0).abs() < 0.1);
        // Material
        assert_eq!(eq["customAttributes"]["material"], "ASTM A106 Gr. B");
        // Nozzle
        let nozzles = eq["nozzles"].as_array().unwrap();
        assert_eq!(nozzles.len(), 1);
        assert_eq!(nozzles[0]["nozzleId"], "N1");
        assert_eq!(nozzles[0]["nominalDiameter"], 200.0);
    }

    #[test]
    fn test_map_piping_line() {
        let mut adapter = ComosAdapter::new(test_config());
        let line = mock_line();
        adapter.cuid_to_id.insert(line.cuid.clone(), adapter.line_id(&line.line_number));
        let objs = adapter.map_piping_line(&line);
        assert_eq!(objs.len(), 2);
        let ln = &objs[0];
        assert_eq!(ln["@type"], "pmef:PipingNetworkSystem");
        assert_eq!(ln["lineNumber"], "8\"-CW-201-A1A2");
        assert_eq!(ln["pipeClass"], "A1A2");
        let dp = ln["designConditions"]["designPressure"].as_f64().unwrap();
        assert!((dp - barg_to_pa_abs(15.0)).abs() < 1.0);
        let dt = ln["designConditions"]["designTemperature"].as_f64().unwrap();
        assert!((dt - degc_to_k(60.0)).abs() < 0.01);
        assert_eq!(ln["specification"]["material"], "ASTM A106 Gr. B");
    }

    #[test]
    fn test_map_instrument_flow_transmitter() {
        let mut adapter = ComosAdapter::new(test_config());
        let inst = mock_instrument();
        adapter.cuid_to_id.insert(inst.cuid.clone(),
            adapter.instrument_id(&inst.tag_number, &inst.cuid));
        let objs = adapter.map_instrument(&inst);
        assert_eq!(objs.len(), 2);
        let obj = &objs[0];
        assert_eq!(obj["@type"], "pmef:InstrumentObject");
        assert_eq!(obj["tagNumber"], "FIC-10101");
        assert_eq!(obj["instrumentClass"], "TRANSMITTER");
        assert_eq!(obj["processVariable"], "FLOW");
        // Measured range
        let mr = &obj["measuredRange"];
        assert_eq!(mr["min"], 0.0);
        assert_eq!(mr["max"], 300.0);
        assert_eq!(mr["unit"], "m3/h");
        // Safety spec (SIL1)
        let ss = &obj["safetySpec"];
        assert_eq!(ss["safetyIntegrityLevel"], 1);
        assert_eq!(ss["architectureType"], "1oo1");
        // Connection
        assert_eq!(obj["connectionSpec"]["signalType"], "HART");
        assert_eq!(obj["connectionSpec"]["ipRating"], "IP66");
        // IEC 81346
        assert_eq!(obj["iec81346"]["functionalAspect"], "=U100.FIC101");
    }

    #[test]
    fn test_map_loop() {
        let mut adapter = ComosAdapter::new(test_config());
        let lp = ComosLoop {
            cuid: "CUID-LOOP-101".to_owned(),
            loop_number: "FIC-10101".to_owned(),
            loop_type: "".to_owned(),
            unit_cuid: "CUID-UNIT-001".to_owned(),
            sil_level: Some(1),
            status: None,
            member_cuids: vec!["CUID-FIC-10101".to_owned(), "CUID-XV-10101".to_owned()],
            controller_cuid: None,
            final_element_cuid: None,
            pid_reference: None,
            iec81346_functional: None,
        };
        adapter.cuid_to_id.insert(lp.cuid.clone(), adapter.loop_id(&lp.loop_number));
        let objs = adapter.map_loop(&lp);
        assert_eq!(objs.len(), 2);
        let obj = &objs[0];
        assert_eq!(obj["@type"], "pmef:InstrumentLoop");
        assert_eq!(obj["loopNumber"], "FIC-10101");
        // Loop type derived from number: FIC → FLOW_CONTROL
        assert_eq!(obj["loopType"], "FLOW_CONTROL");
        assert_eq!(obj["silLevel"], 1);
    }

    #[test]
    fn test_map_plc() {
        let mut adapter = ComosAdapter::new(test_config());
        let plc = ComosPlc {
            cuid: "CUID-PLC-001".to_owned(),
            tag_number: "S7-1500-01".to_owned(),
            comos_class: "@S10".to_owned(),
            class_description: "PLC CPU".to_owned(),
            unit_cuid: "CUID-UNIT-001".to_owned(),
            vendor: Some("Siemens".to_owned()),
            family: Some("S7-1500".to_owned()),
            article_number: Some("6ES7 515-2AM01-0AB0".to_owned()),
            rack: Some(0), slot: Some(1),
            ip_address: Some("192.168.1.10".to_owned()),
            safety_cpu: Some(false),
            tia_portal_ref: Some("TIA-S7-1500-01".to_owned()),
            aml_ref: None,
            iec81346_product: Some("-PLC01".to_owned()),
        };
        adapter.cuid_to_id.insert(plc.cuid.clone(), adapter.plc_id(&plc.tag_number, &plc.cuid));
        let objs = adapter.map_plc(&plc);
        assert_eq!(objs.len(), 2);
        let obj = &objs[0];
        assert_eq!(obj["@type"], "pmef:PLCObject");
        assert_eq!(obj["plcClass"], "CPU");
        assert_eq!(obj["vendor"], "Siemens");
        assert_eq!(obj["slot"], 1);
    }

    #[test]
    fn test_has_equivalent_in() {
        let adapter = ComosAdapter::new(test_config());
        let rel = adapter.make_has_equivalent_in("urn:pmef:obj:test:P-201A", "CUID-P-201A");
        assert_eq!(rel["targetSystem"], "COMOS");
        assert_eq!(rel["targetSystemId"], "CUID-P-201A");
        assert_eq!(rel["confidence"], 1.0);
    }

    #[test]
    fn test_adapter_trait() {
        let adapter = ComosAdapter::new(test_config());
        assert_eq!(adapter.name(), "pmef-adapter-comos");
        assert_eq!(adapter.target_system(), "COMOS");
        assert_eq!(adapter.conformance_level(), 2);
        assert!(adapter.supported_domains().contains(&"ei"));
        assert!(adapter.supported_domains().contains(&"equipment"));
    }
}
