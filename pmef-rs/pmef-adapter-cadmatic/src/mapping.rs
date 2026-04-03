//! CADMATIC → PMEF field mapping tables and mapper implementation.
//!
//! This module provides:
//! - [`component_class_map`] — CADMATIC component type → PMEF componentClass
//! - [`equipment_class_map`] — CADMATIC equipment type → PMEF @type + equipmentClass
//! - [`material_map`] — CADMATIC material code → PMEF material string
//! - [`CadmaticFieldMapper`] — stateful mapper producing PMEF `serde_json::Value` objects

use crate::api::*;
use std::collections::HashMap;
use thiserror::Error;

/// Errors during field mapping.
#[derive(Debug, Error)]
pub enum MappingError {
    #[error("No PMEF mapping found for CADMATIC type '{0}'")]
    UnknownType(String),
    #[error("Required field '{field}' missing on CADMATIC object '{object}'")]
    MissingField { object: String, field: &'static str },
    #[error("Coordinate conversion error: {0}")]
    Coordinate(String),
}

/// Statistics accumulated during a mapping run.
#[derive(Debug, Default, Clone)]
pub struct MappingStats {
    pub components_mapped: usize,
    pub components_unmapped: usize,
    pub equipment_mapped: usize,
    pub equipment_unmapped: usize,
    pub fields_unmapped: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Component Class Map
// ─────────────────────────────────────────────────────────────────────────────

/// Map a CADMATIC component type string to a PMEF `@type` and `componentClass`.
///
/// Returns `(pmef_type, component_class, skey_default)`.
///
/// # Source
/// CADMATIC component type strings from the CADMATIC Data Model Reference
/// (CADMATIC 2024.1 API documentation, endpoint
/// `GET /pipelines/{lineId}/components`, field `componentType`).
pub fn component_class_map(cadmatic_type: &str) -> Option<(&'static str, &'static str, &'static str)> {
    match cadmatic_type {
        // ── Pipe ──────────────────────────────────────────────────────────
        "StraightPipe" | "Pipe"              => Some(("pmef:Pipe",       "PIPE",                "PIPW    ")),
        "BentPipe"                           => Some(("pmef:Pipe",       "PIPE",                "PIPB    ")),

        // ── Elbows ────────────────────────────────────────────────────────
        "Elbow90LR" | "Elbow90LongRadius"   => Some(("pmef:Elbow",      "ELBOW",               "ELBWLR90")),
        "Elbow90SR" | "Elbow90ShortRadius"  => Some(("pmef:Elbow",      "ELBOW",               "ELBWSR90")),
        "Elbow45LR" | "Elbow45LongRadius"   => Some(("pmef:Elbow",      "ELBOW",               "ELBWLR45")),
        "Elbow45SR" | "Elbow45ShortRadius"  => Some(("pmef:Elbow",      "ELBOW",               "ELBWSR45")),
        "ElbowCustomAngle" | "ElbowMitre"   => Some(("pmef:Elbow",      "ELBOW",               "ELBWCUST")),

        // ── Tees ──────────────────────────────────────────────────────────
        "EqualTee" | "Tee"                  => Some(("pmef:Tee",        "TEE",                 "TEBWEQUL")),
        "ReducingTee"                        => Some(("pmef:Tee",        "TEE",                 "TEBWREDC")),
        "LatTee" | "Lateral"                => Some(("pmef:Tee",        "TEE",                 "TEBWLATL")),

        // ── Reducers ──────────────────────────────────────────────────────
        "ConcentricReducer" | "Reducer"     => Some(("pmef:Reducer",    "REDUCER_CONCENTRIC",  "RDCWCNCN")),
        "EccentricReducer"                  => Some(("pmef:Reducer",    "REDUCER_ECCENTRIC",   "RDCWECCT")),

        // ── Flanges ───────────────────────────────────────────────────────
        "WeldNeckFlange" | "Flange"         => Some(("pmef:Flange",     "FLANGE",              "FLWNRF  ")),
        "BlindFlange"                        => Some(("pmef:Flange",     "BLIND_FLANGE",        "FLBLRF  ")),
        "SlipOnFlange"                       => Some(("pmef:Flange",     "FLANGE",              "FLSORF  ")),
        "SocketWeldFlange"                   => Some(("pmef:Flange",     "FLANGE",              "FLSWRF  ")),
        "LapJointFlange"                     => Some(("pmef:Flange",     "FLANGE",              "FLLJ    ")),
        "OrificeFlange"                      => Some(("pmef:Flange",     "FLANGE",              "FLORFRF ")),

        // ── Gaskets ───────────────────────────────────────────────────────
        "SpiralWoundGasket" | "Gasket"      => Some(("pmef:Gasket",     "GASKET",              "GKSWRG  ")),
        "RingJointGasket"                    => Some(("pmef:Gasket",     "GASKET",              "GKRJRT  ")),
        "SheetGasket"                        => Some(("pmef:Gasket",     "GASKET",              "GKSHFR  ")),

        // ── Valves ────────────────────────────────────────────────────────
        "GateValve"                          => Some(("pmef:Valve",      "VALVE_GATE",          "GTBWFLFL")),
        "GlobeValve"                         => Some(("pmef:Valve",      "VALVE_GLOBE",         "GLBWFLFL")),
        "BallValve"                          => Some(("pmef:Valve",      "VALVE_BALL",          "BLBWFLFL")),
        "ButterflyValve"                     => Some(("pmef:Valve",      "VALVE_BUTTERFLY",     "BFBWFLFL")),
        "CheckValve"                         => Some(("pmef:Valve",      "VALVE_CHECK",         "CKBWFLFL")),
        "ControlValve"                       => Some(("pmef:Valve",      "VALVE_CONTROL",       "GLBWFLFL")),
        "SafetyValve" | "PSV" | "PRV"       => Some(("pmef:Valve",      "VALVE_RELIEF",        "SVBWFLFL")),
        "NeedleValve"                        => Some(("pmef:Valve",      "VALVE_NEEDLE",        "NLBWFLFL")),
        "DiaphragmValve"                     => Some(("pmef:Valve",      "VALVE_DIAPHRAGM",     "DGBWFLFL")),
        "PinchValve"                         => Some(("pmef:Valve",      "VALVE_GATE",          "PCBWFLFL")), // fallback

        // ── Olets ────────────────────────────────────────────────────────
        "Weldolet"                           => Some(("pmef:Olet",       "OLET_WELDOLET",       "WOLW    ")),
        "Sockolet"                           => Some(("pmef:Olet",       "OLET_SOCKOLET",       "SOLW    ")),
        "Thredolet"                          => Some(("pmef:Olet",       "OLET_THREDOLET",      "TOLW    ")),
        "Elbolet"                            => Some(("pmef:Olet",       "OLET_ELBOLET",        "EOLW    ")),
        "Nipolet"                            => Some(("pmef:Olet",       "OLET_NIPOLET",        "NOLW    ")),

        // ── Welds ────────────────────────────────────────────────────────
        "ButtWeld" | "Weld"                 => Some(("pmef:Weld",       "WELD_BUTT",           "WLDW    ")),
        "SocketWeld"                         => Some(("pmef:Weld",       "WELD_SOCKET",         "WLDSW   ")),
        "FilletWeld"                         => Some(("pmef:Weld",       "WELD_FILLET",         "WLDFW   ")),

        // ── Supports ──────────────────────────────────────────────────────
        "PipeSupport" | "Support"           => Some(("pmef:PipeSupport","PIPE_SUPPORT",        "SUPRW   ")),
        "PipeSupportAnchor"                  => Some(("pmef:PipeSupport","PIPE_SUPPORT",        "ANCHRW  ")),
        "PipeSupportGuide"                   => Some(("pmef:PipeSupport","PIPE_SUPPORT",        "GUIDERW ")),

        // ── Fallback ─────────────────────────────────────────────────────
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Equipment Class Map
// ─────────────────────────────────────────────────────────────────────────────

/// Map a CADMATIC equipment type to (PMEF @type, equipmentClass).
///
/// # Source
/// CADMATIC equipment type strings from the CADMATIC Equipment Model
/// (CADMATIC 2024.1 API documentation, `GET /equipment`, field `equipmentType`).
pub fn equipment_class_map(cadmatic_type: &str) -> (&'static str, &'static str) {
    match cadmatic_type {
        // ── Pumps ─────────────────────────────────────────────────────────
        "CentrifugalPump" | "Pump"          => ("pmef:Pump",          "CENTRIFUGAL_PUMP"),
        "ReciprocatingPump"                  => ("pmef:Pump",          "RECIPROCATING_PUMP"),
        "GearPump"                           => ("pmef:Pump",          "GEAR_PUMP"),
        "SubmersiblePump"                    => ("pmef:Pump",          "SUBMERSIBLE_PUMP"),
        "ScrewPump"                          => ("pmef:Pump",          "SCREW_PUMP"),
        "DiaphragmPump"                      => ("pmef:Pump",          "DIAPHRAGM_PUMP"),

        // ── Compressors ──────────────────────────────────────────────────
        "CentrifugalCompressor"             => ("pmef:Compressor",     "CENTRIFUGAL_COMPRESSOR"),
        "ReciprocatingCompressor"            => ("pmef:Compressor",     "RECIPROCATING_COMPRESSOR"),
        "ScrewCompressor"                    => ("pmef:Compressor",     "SCREW_COMPRESSOR"),
        "LobedCompressor"                    => ("pmef:Compressor",     "LOBE_COMPRESSOR"),

        // ── Vessels ───────────────────────────────────────────────────────
        "PressureVessel" | "Vessel"         => ("pmef:Vessel",         "PRESSURE_VESSEL"),
        "Drum" | "KnockOutDrum"             => ("pmef:Vessel",         "KNOCK_OUT_DRUM"),
        "Accumulator"                        => ("pmef:Vessel",         "ACCUMULATOR"),
        "Separator"                          => ("pmef:Vessel",         "SEPARATOR"),
        "Absorber"                           => ("pmef:Vessel",         "ABSORBER"),
        "Scrubber"                           => ("pmef:Vessel",         "SCRUBBER"),

        // ── Tanks ─────────────────────────────────────────────────────────
        "FixedRoofTank" | "Tank"            => ("pmef:Tank",           "FIXED_ROOF_TANK"),
        "FloatingRoofTank"                   => ("pmef:Tank",           "FLOATING_ROOF_TANK"),
        "SphericalTank"                      => ("pmef:Tank",           "SPHERICAL_TANK"),
        "HorizontalTank"                     => ("pmef:Tank",           "HORIZONTAL_TANK"),
        "DayTank"                            => ("pmef:Tank",           "DAY_TANK"),
        "OpenTopTank"                        => ("pmef:Tank",           "OPEN_TOP"),

        // ── Heat exchangers ───────────────────────────────────────────────
        "ShellAndTubeHX" | "HeatExchanger" => ("pmef:HeatExchanger",  "SHELL_AND_TUBE_HEAT_EXCHANGER"),
        "PlateHX"                            => ("pmef:HeatExchanger",  "PLATE_HEAT_EXCHANGER"),
        "FinTubeHX" | "AirCooler"           => ("pmef:HeatExchanger",  "AIR_COOLED_HEAT_EXCHANGER"),
        "DoubleWallHX"                       => ("pmef:HeatExchanger",  "DOUBLE_WALL_HEAT_EXCHANGER"),
        "SpiralHX"                           => ("pmef:HeatExchanger",  "SPIRAL_HEAT_EXCHANGER"),

        // ── Columns / Towers ─────────────────────────────────────────────
        "DistillationColumn" | "Column"     => ("pmef:Column",         "DISTILLATION_COLUMN"),
        "AbsorptionColumn"                   => ("pmef:Column",         "ABSORPTION_COLUMN"),
        "PackedColumn"                       => ("pmef:Column",         "PACKED_COLUMN"),
        "StripperColumn"                     => ("pmef:Column",         "STRIPPER_COLUMN"),

        // ── Reactors ──────────────────────────────────────────────────────
        "Reactor"                            => ("pmef:Reactor",        "FIXED_BED_REACTOR"),
        "FluidisedBedReactor"                => ("pmef:Reactor",        "FLUIDISED_BED_REACTOR"),
        "ElectricArcFurnace" | "EAF"        => ("pmef:Reactor",        "ELECTRIC_ARC_FURNACE"),
        "Converter"                          => ("pmef:Reactor",        "CONVERTER"),
        "Ladle"                              => ("pmef:Reactor",        "LADLE"),

        // ── Filters / Strainers ───────────────────────────────────────────
        "Filter" | "Strainer"               => ("pmef:Filter",         "STRAINER"),
        "BasketStrainer"                     => ("pmef:Filter",         "BASKET_STRAINER"),
        "YStrainer"                          => ("pmef:Filter",         "Y_STRAINER"),
        "CartridgeFilter"                    => ("pmef:Filter",         "CARTRIDGE_FILTER"),
        "BagFilter"                          => ("pmef:Filter",         "BAG_FILTER"),

        // ── Turbines ─────────────────────────────────────────────────────
        "SteamTurbine"                       => ("pmef:Turbine",        "STEAM_TURBINE"),
        "GasTurbine"                         => ("pmef:Turbine",        "GAS_TURBINE"),
        "HydroTurbine"                       => ("pmef:Turbine",        "HYDRO_TURBINE"),
        "SteamExpander"                      => ("pmef:Turbine",        "STEAM_EXPANDER"),

        // ── Fallback ──────────────────────────────────────────────────────
        _ => ("pmef:GenericEquipment", "GENERIC"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Material Map
// ─────────────────────────────────────────────────────────────────────────────

/// Map a CADMATIC material code to a PMEF material string.
pub fn material_map(cadmatic_material: &str) -> &str {
    match cadmatic_material {
        // CADMATIC native codes → PMEF (ASTM/EN designations)
        "A106B" | "A106GRB" | "CS"         => "ASTM A106 Gr. B",
        "A106A"                              => "ASTM A106 Gr. A",
        "A53B"  | "A53GRB"                  => "ASTM A53 Gr. B",
        "A312TP316L" | "SS316L" | "316L"   => "ASTM A312 TP316L",
        "A312TP304L" | "SS304L" | "304L"   => "ASTM A312 TP304L",
        "A312TP316" | "SS316"               => "ASTM A312 TP316",
        "A312TP304" | "SS304"               => "ASTM A312 TP304",
        "A335P11"                            => "ASTM A335 Gr. P11",
        "A335P22"                            => "ASTM A335 Gr. P22",
        "A335P91"                            => "ASTM A335 Gr. P91",
        "A333GR6"                            => "ASTM A333 Gr. 6",
        "A234WPB" | "WPBCS"                 => "ASTM A234 WPB",
        "A105"                               => "ASTM A105",
        "A216WCB" | "WCB"                   => "ASTM A216 WCB",
        "P265GH"                             => "EN 10216-2 P265GH",
        "P235GH"                             => "EN 10216-2 P235GH",
        "P355GH"                             => "EN 10216-2 P355GH",
        "X5CrNi18-10" | "1.4301"           => "EN 10216-5 X5CrNi18-10",
        "X2CrNiMo17-12-2" | "1.4404"       => "EN 10216-5 X2CrNiMo17-12-2",
        "SA516GR70" | "SA516-70"            => "ASTM A516 Gr. 70",
        "SA106B"                             => "ASTM A106 Gr. B",
        _ => cadmatic_material, // passthrough — preserve CADMATIC code
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Flange Type Map
// ─────────────────────────────────────────────────────────────────────────────

/// Map CADMATIC SKEY prefix to PMEF flangeType string.
fn skey_to_flange_type(skey: Option<&str>) -> &'static str {
    let sk = skey.unwrap_or("").trim();
    if sk.starts_with("FLWN") { "WELD_NECK" }
    else if sk.starts_with("FLBL") { "BLIND" }
    else if sk.starts_with("FLSO") { "SLIP_ON" }
    else if sk.starts_with("FLSW") { "SOCKET_WELD" }
    else if sk.starts_with("FLLJ") { "LAP_JOINT" }
    else if sk.starts_with("FLOR") { "ORIFICE" }
    else { "WELD_NECK" }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit conversions
// ─────────────────────────────────────────────────────────────────────────────

/// Convert bar gauge → Pa absolute.
fn barg_to_pa_abs(barg: f64) -> f64 { barg * 100_000.0 + 101_325.0 }

/// Convert °C → K.
fn degc_to_k(degc: f64) -> f64 { degc + 273.15 }

// ─────────────────────────────────────────────────────────────────────────────
// CadmaticFieldMapper
// ─────────────────────────────────────────────────────────────────────────────

/// Stateful field mapper. Maintains counters and a GUID → @id lookup
/// for topology resolution across components.
pub struct CadmaticFieldMapper {
    project_code: String,
    /// CADMATIC objectGuid → PMEF @id (populated during export)
    guid_to_id: HashMap<String, String>,
    /// Counters per component type (for sequential @id generation)
    counters: HashMap<String, usize>,
    pub stats: MappingStats,
}

impl CadmaticFieldMapper {
    pub fn new(project_code: String) -> Self {
        Self {
            project_code,
            guid_to_id: HashMap::new(),
            counters: HashMap::new(),
            stats: MappingStats::default(),
        }
    }

    // ── @id generation ───────────────────────────────────────────────────────

    fn next_id(&mut self, domain: &str, local: &str) -> String {
        format!("urn:pmef:{domain}:{}:{local}", self.project_code)
    }

    fn component_id(&mut self, line_clean: &str, keyword: &str, guid: &str) -> String {
        let count = self.counters.entry(format!("{line_clean}-{keyword}")).or_insert(0);
        *count += 1;
        let id = format!(
            "urn:pmef:obj:{}:{line_clean}-{keyword}-{:03}",
            self.project_code, count
        );
        self.guid_to_id.insert(guid.to_owned(), id.clone());
        id
    }

    fn equipment_id(&mut self, tag: &str, guid: &str) -> String {
        let tag_clean: String = tag.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        let id = format!("urn:pmef:obj:{}:{tag_clean}", self.project_code);
        self.guid_to_id.insert(guid.to_owned(), id.clone());
        id
    }

    fn resolve_guid(&self, guid: &str) -> Option<&str> {
        self.guid_to_id.get(guid).map(|s| s.as_str())
    }

    // ── Fixed objects ────────────────────────────────────────────────────────

    pub fn make_file_header(&self, project_id: &str) -> serde_json::Value {
        serde_json::json!({
            "@type": "pmef:FileHeader",
            "@id": format!("urn:pmef:pkg:{}:{project_id}", self.project_code),
            "pmefVersion": "0.9.0",
            "plantId": format!("urn:pmef:plant:{}:{project_id}", self.project_code),
            "projectCode": self.project_code,
            "coordinateSystem": "Z-up",
            "units": "mm",
            "revisionId": "r2026-01-01-001",
            "changeState": "SHARED",
            "authoringTool": "pmef-adapter-cadmatic 0.9.0"
        })
    }

    pub fn make_plant(&self, project_id: &str) -> serde_json::Value {
        serde_json::json!({
            "@type": "pmef:Plant",
            "@id": format!("urn:pmef:plant:{}:{project_id}", self.project_code),
            "pmefVersion": "0.9.0",
            "name": project_id,
            "revision": { "revisionId": "r2026-01-01-001", "changeState": "SHARED",
                          "authoringTool": "pmef-adapter-cadmatic 0.9.0" }
        })
    }

    pub fn make_unit(&self, project_id: &str) -> serde_json::Value {
        serde_json::json!({
            "@type": "pmef:Unit",
            "@id": format!("urn:pmef:unit:{}:{project_id}-U01", self.project_code),
            "pmefVersion": "0.9.0",
            "name": format!("{project_id} — Main Unit"),
            "isPartOf": format!("urn:pmef:plant:{}:{project_id}", self.project_code),
            "revision": { "revisionId": "r2026-01-01-001", "changeState": "SHARED" }
        })
    }

    pub fn make_has_equivalent_in(&self, pmef_id: &str, cadmatic_guid: &str) -> serde_json::Value {
        let local = pmef_id.split(':').last().unwrap_or("obj");
        let rel_id = format!("urn:pmef:rel:{}:{local}-cadmatic", self.project_code);
        serde_json::json!({
            "@type": "pmef:HasEquivalentIn",
            "@id": rel_id,
            "relationType": "HAS_EQUIVALENT_IN",
            "sourceId": pmef_id,
            "targetId": pmef_id,
            "targetSystem": "CADMATIC",
            "targetSystemId": cadmatic_guid,
            "mappingType": "EXACT",
            "derivedBy": "ADAPTER_IMPORT",
            "confidence": 1.0,
            "revision": { "revisionId": "r2026-01-01-001", "changeState": "SHARED",
                          "authoringTool": "pmef-adapter-cadmatic 0.9.0" }
        })
    }

    // ── Equipment mapping ────────────────────────────────────────────────────

    pub fn map_equipment(
        &mut self,
        equip: &CadmaticEquipment,
    ) -> Result<serde_json::Value, MappingError> {
        let (pmef_type, equip_class) = equipment_class_map(&equip.equipment_type);
        let obj_id = self.equipment_id(&equip.tag_number, &equip.object_guid);

        // Map nozzles
        let nozzles: Vec<serde_json::Value> = equip.nozzles.iter().map(|noz| {
            let dn = noz.nominal_diameter_mm.unwrap_or(100.0);
            serde_json::json!({
                "nozzleId": noz.nozzle_id,
                "nozzleMark": noz.nozzle_mark.as_deref().unwrap_or(&noz.nozzle_id),
                "service": noz.service,
                "nominalDiameter": dn,
                "flangeRating": noz.flange_rating.as_deref().unwrap_or("ANSI-150"),
                "facingType": noz.facing_type.as_deref().unwrap_or("RF"),
                "coordinate": [noz.position.x, noz.position.y, noz.position.z],
                "direction": noz.direction.as_ref()
                    .map(|d| serde_json::json!([d.x, d.y, d.z]))
                    .unwrap_or(serde_json::json!([0, 0, 1])),
                "connectedLineId": noz.connected_line_id
            })
        }).collect();

        let obj = serde_json::json!({
            "@type": pmef_type,
            "@id": obj_id,
            "pmefVersion": "0.9.0",
            "isPartOf": format!("urn:pmef:unit:{}:{}-U01", self.project_code, &self.project_code),
            "equipmentBasic": {
                "tagNumber": equip.tag_number,
                "equipmentClass": equip_class,
                "serviceDescription": equip.description,
                "designCode": equip.design_code,
                "trainId": equip.train_id,
                "manufacturer": equip.manufacturer,
                "model": equip.model
            },
            "nozzles": nozzles,
            "geometry": {
                "type": "none",
                "boundingBox": equip.bbox_min.as_ref().zip(equip.bbox_max.as_ref()).map(|(mn, mx)| {
                    serde_json::json!({
                        "xMin": mn.x, "xMax": mx.x,
                        "yMin": mn.y, "yMax": mx.y,
                        "zMin": mn.z, "zMax": mx.z
                    })
                })
            },
            "customAttributes": {
                "cadmaticGuid": equip.object_guid,
                "areaCode": equip.area_code,
                "weightKg": equip.weight_kg,
                "emptyWeightKg": equip.empty_weight_kg,
                "operatingWeightKg": equip.operating_weight_kg
            },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringToolObjectId": equip.object_guid,
                "authoringTool": "pmef-adapter-cadmatic 0.9.0"
            }
        });

        self.stats.equipment_mapped += 1;
        Ok(obj)
    }

    // ── Pipeline mapping ─────────────────────────────────────────────────────

    pub fn map_pipeline(
        &mut self,
        line: &CadmaticLine,
    ) -> Result<serde_json::Value, MappingError> {
        let line_clean: String = line.line_number.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        let line_id = format!("urn:pmef:line:{}:{line_clean}", self.project_code);

        let design_conds = serde_json::json!({
            "designPressure": line.design_pressure_barg.map(barg_to_pa_abs),
            "designTemperature": line.design_temperature_degc.map(degc_to_k),
            "operatingPressure": line.operating_pressure_barg.map(barg_to_pa_abs),
            "operatingTemperature": line.operating_temperature_degc.map(degc_to_k),
            "testPressure": line.test_pressure_barg.map(barg_to_pa_abs),
            "testMedium": "WATER",
            "vacuumService": false
        });

        let spec = serde_json::json!({
            "nominalDiameter": line.nominal_diameter.unwrap_or(100.0),
            "outsideDiameter": line.outside_diameter_mm,
            "wallThickness": line.wall_thickness_mm,
            "schedule": line.schedule,
            "pipeClass": line.pipe_class,
            "material": line.material.as_deref().map(material_map),
            "pressureRating": "ANSI-150",  // derive from pipe class in full impl
            "corrosionAllowance": 3.0,
            "insulationType": line.insulation_type.as_deref().unwrap_or("NONE")
        });

        let obj = serde_json::json!({
            "@type": "pmef:PipingNetworkSystem",
            "@id": line_id,
            "pmefVersion": "0.9.0",
            "lineNumber": line.line_number,
            "nominalDiameter": line.nominal_diameter,
            "pipeClass": line.pipe_class,
            "mediumCode": line.fluid_code,
            "mediumDescription": line.fluid_description,
            "fluidPhase": "LIQUID",
            "isPartOf": format!("urn:pmef:unit:{}:{}-U01", self.project_code, self.project_code),
            "designConditions": design_conds,
            "specification": spec,
            "segments": [format!("urn:pmef:seg:{}:{line_clean}-S1", self.project_code)],
            "pidSheetRef": line.pid_reference,
            "customAttributes": {
                "cadmaticLineId": line.line_id,
                "dexpiRef": line.dexpi_ref,
                "lastModified": line.modified_date
            },
            "revision": {
                "revisionId": "r2026-01-01-001",
                "changeState": "SHARED",
                "authoringToolObjectId": line.line_id,
                "authoringTool": "pmef-adapter-cadmatic 0.9.0"
            }
        });
        Ok(obj)
    }

    // ── Segment + component mapping ──────────────────────────────────────────

    /// Map a CADMATIC line + its components to a PMEF segment + component objects + relationships.
    pub fn map_segment_and_components(
        &mut self,
        line: &CadmaticLine,
        components: &[CadmaticComponent],
    ) -> (serde_json::Value, Vec<serde_json::Value>, Vec<serde_json::Value>) {
        let line_clean: String = line.line_number.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        let line_id = format!("urn:pmef:line:{}:{line_clean}", self.project_code);
        let seg_id = format!("urn:pmef:seg:{}:{line_clean}-S1", self.project_code);

        // Pre-register all component IDs (needed for connectedTo resolution)
        let mut comp_ids: Vec<String> = Vec::new();
        for (i, comp) in components.iter().enumerate() {
            let kwshort = component_class_map(&comp.component_type)
                .map(|(_, cls, _)| &cls[..4])
                .unwrap_or("COMP");
            let id = format!(
                "urn:pmef:obj:{}:{line_clean}-{kwshort}-{:03}",
                self.project_code, i + 1
            );
            self.guid_to_id.insert(comp.object_guid.clone(), id.clone());
            comp_ids.push(id);
        }

        // Map each component
        let mut pmef_components: Vec<serde_json::Value> = Vec::new();
        let mut relationships: Vec<serde_json::Value> = Vec::new();

        for (i, comp) in components.iter().enumerate() {
            let obj_id = comp_ids[i].clone();

            let mapped = match component_class_map(&comp.component_type) {
                None => {
                    tracing::warn!(
                        "No PMEF mapping for CADMATIC type '{}' (guid={})",
                        comp.component_type, comp.object_guid
                    );
                    self.stats.components_unmapped += 1;
                    continue;
                }
                Some((pmef_type, comp_class, skey_default)) => {
                    let skey = comp.spec_key.as_deref().unwrap_or(skey_default);
                    let material_str = comp.material.as_deref()
                        .map(material_map).unwrap_or("ASTM A106 Gr. B");

                    // Build ports
                    let ports: Vec<serde_json::Value> = comp.end_points.iter().map(|ep| {
                        let conn = ep.connected_to_guid.as_deref()
                            .and_then(|g| self.guid_to_id.get(g))
                            .cloned();
                        serde_json::json!({
                            "portId": format!("P{}", ep.index + 1),
                            "coordinate": [ep.position.x, ep.position.y, ep.position.z],
                            "direction": ep.direction.as_ref()
                                .map(|d| serde_json::json!([d.x, d.y, d.z])),
                            "nominalDiameter": ep.bore_mm,
                            "endType": ep.end_type.as_deref().unwrap_or("BW"),
                            "connectedTo": conn
                        })
                    }).collect();

                    // Common base
                    let mut obj = serde_json::json!({
                        "@type": pmef_type,
                        "@id": obj_id,
                        "pmefVersion": "0.9.0",
                        "isPartOf": seg_id,
                        "itemNumber": comp.item_number.as_deref().unwrap_or(&(i+1).to_string()),
                        "componentSpec": {
                            "componentClass": comp_class,
                            "skey": skey,
                            "weight": comp.weight_kg
                        },
                        "ports": ports,
                        "catalogRef": {
                            "catalogId": comp.catalogue_ref.as_deref().unwrap_or(""),
                            "vendorMappings": [{
                                "vendorSystem": "CADMATIC",
                                "vendorId": comp.object_guid
                            }]
                        },
                        "revision": {
                            "revisionId": "r2026-01-01-001",
                            "changeState": "SHARED",
                            "authoringToolObjectId": comp.object_guid,
                            "authoringTool": "pmef-adapter-cadmatic 0.9.0"
                        }
                    });

                    // Type-specific fields
                    match pmef_type {
                        "pmef:Pipe" => {
                            if comp.end_points.len() >= 2 {
                                let p1 = &comp.end_points[0].position;
                                let p2 = &comp.end_points[1].position;
                                let len = ((p2.x-p1.x).powi(2)+(p2.y-p1.y).powi(2)+(p2.z-p1.z).powi(2)).sqrt();
                                obj["pipeLength"] = serde_json::Value::from(len);
                            }
                        }
                        "pmef:Elbow" => {
                            obj["angle"] = serde_json::Value::from(comp.angle_deg.unwrap_or(90.0));
                            // Classify radius
                            let dn = comp.nominal_diameter_mm.unwrap_or(100.0);
                            let r = comp.bend_radius_mm.unwrap_or(1.5 * dn);
                            let radius_enum = if (r - 1.5*dn).abs() < 5.0 { "LONG_RADIUS" }
                                             else if (r - 1.0*dn).abs() < 5.0 { "SHORT_RADIUS" }
                                             else { "CUSTOM" };
                            obj["radius"] = serde_json::Value::from(radius_enum);
                            if radius_enum == "CUSTOM" {
                                obj["radiusMm"] = serde_json::Value::from(r);
                            }
                        }
                        "pmef:Reducer" => {
                            obj["reducerType"] = serde_json::Value::from(
                                if comp.component_type.to_lowercase().contains("eccentric") {
                                    "ECCENTRIC"
                                } else { "CONCENTRIC" }
                            );
                            obj["largeDiameter"] = serde_json::Value::from(
                                comp.large_bore_mm.unwrap_or(
                                    comp.end_points.first().and_then(|e| e.bore_mm).unwrap_or(100.0)
                                )
                            );
                            obj["smallDiameter"] = serde_json::Value::from(
                                comp.small_bore_mm.unwrap_or(
                                    comp.end_points.last().and_then(|e| e.bore_mm).unwrap_or(80.0)
                                )
                            );
                        }
                        "pmef:Flange" => {
                            obj["flangeType"] = serde_json::Value::from(
                                skey_to_flange_type(comp.spec_key.as_deref())
                            );
                            obj["rating"] = serde_json::Value::from("ANSI-150");
                            obj["facing"] = serde_json::Value::from("RF");
                        }
                        "pmef:Gasket" => {
                            obj["gasketType"] = serde_json::Value::from("SPIRAL_WOUND");
                            obj["gasketMaterial"] = serde_json::Value::from("SS316-FLEXITE");
                        }
                        "pmef:Valve" => {
                            if let Some(tag) = &comp.tag_number {
                                obj["tagNumber"] = serde_json::Value::from(tag.as_str());
                            }
                            let mut valve_spec = serde_json::json!({});
                            if let Some(act) = &comp.actuator_type {
                                valve_spec["actuatorType"] = serde_json::Value::from(act.as_str());
                            }
                            if let Some(fp) = &comp.fail_position {
                                valve_spec["failPosition"] = serde_json::Value::from(fp.as_str());
                            }
                            if !valve_spec.as_object().unwrap().is_empty() {
                                obj["valveSpec"] = valve_spec;
                            }
                        }
                        "pmef:Weld" => {
                            if let Some(wn) = &comp.weld_number {
                                let connects: Vec<_> = comp.end_points.iter()
                                    .take(2)
                                    .filter_map(|ep| ep.connected_to_guid.as_deref())
                                    .filter_map(|g| self.guid_to_id.get(g))
                                    .cloned()
                                    .collect();
                                obj["weldSpec"] = serde_json::json!({
                                    "weldNumber": wn,
                                    "weldType": "BW",
                                    "weldingProcess": "GTAW",
                                    "pwht": false,
                                    "ndeMethod": comp.nde_method.as_deref().unwrap_or("VT"),
                                    "ndePercentage": 10,
                                    "inspectionLevel": "B",
                                    "inspectionStatus": "PENDING"
                                });
                                if connects.len() == 2 {
                                    obj["connects"] = serde_json::json!(connects);
                                }
                            }
                        }
                        "pmef:PipeSupport" => {
                            obj["supportsMark"] = serde_json::Value::from(
                                comp.item_number.as_deref().unwrap_or("S1")
                            );
                            let sup_type = if comp.component_type.contains("Anchor") { "ANCHOR" }
                                else if comp.component_type.contains("Guide") { "GUIDE" }
                                else { "RESTING" };
                            obj["supportSpec"] = serde_json::json!({
                                "supportType": sup_type,
                                "attachmentType": "WELDED"
                            });
                        }
                        _ => {}
                    }

                    obj
                }
            };

            // HasEquivalentIn relationship
            let equiv = self.make_has_equivalent_in(
                mapped["@id"].as_str().unwrap_or(""),
                &comp.object_guid,
            );
            pmef_components.push(mapped);
            relationships.push(equiv);
            self.stats.components_mapped += 1;
        }

        // Build segment
        let segment = serde_json::json!({
            "@type": "pmef:PipingSegment",
            "@id": seg_id,
            "isPartOf": line_id,
            "segmentNumber": 1,
            "components": comp_ids,
            "revision": { "revisionId": "r2026-01-01-001", "changeState": "SHARED",
                          "authoringTool": "pmef-adapter-cadmatic 0.9.0" }
        });

        (segment, pmef_components, relationships)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::mock;

    #[test]
    fn test_component_class_map_coverage() {
        // All common CADMATIC types must map to a known PMEF type
        let known = [
            "StraightPipe", "Elbow90LR", "Elbow45LR", "EqualTee", "ReducingTee",
            "ConcentricReducer", "EccentricReducer", "WeldNeckFlange", "BlindFlange",
            "SpiralWoundGasket", "GateValve", "GlobeValve", "BallValve",
            "ButterflyValve", "CheckValve", "ControlValve", "SafetyValve",
            "Weldolet", "ButtWeld", "PipeSupport",
        ];
        for t in known {
            let result = component_class_map(t);
            assert!(result.is_some(), "Missing mapping for CADMATIC type: {t}");
            let (pmef_type, _, _) = result.unwrap();
            assert!(pmef_type.starts_with("pmef:"), "Invalid pmef_type for {t}: {pmef_type}");
        }
    }

    #[test]
    fn test_component_class_map_unknown_returns_none() {
        assert!(component_class_map("NonExistentType").is_none());
    }

    #[test]
    fn test_equipment_class_map_coverage() {
        let known = [
            "CentrifugalPump", "ReciprocatingPump", "CentrifugalCompressor",
            "PressureVessel", "Drum", "FixedRoofTank", "ShellAndTubeHX",
            "PlateHX", "DistillationColumn", "Reactor", "Filter",
            "SteamTurbine", "YStrainer", "ElectricArcFurnace",
        ];
        for t in known {
            let (pmef_type, cls) = equipment_class_map(t);
            assert!(pmef_type.starts_with("pmef:"), "Bad pmef_type for {t}: {pmef_type}");
            assert!(!cls.is_empty(), "Empty equipmentClass for {t}");
        }
    }

    #[test]
    fn test_equipment_class_map_fallback() {
        let (pmef_type, cls) = equipment_class_map("SomeWeirdThing");
        assert_eq!(pmef_type, "pmef:GenericEquipment");
        assert_eq!(cls, "GENERIC");
    }

    #[test]
    fn test_material_map() {
        assert_eq!(material_map("A106B"), "ASTM A106 Gr. B");
        assert_eq!(material_map("SS316L"), "ASTM A312 TP316L");
        assert_eq!(material_map("A312TP316L"), "ASTM A312 TP316L");
        assert_eq!(material_map("P265GH"), "EN 10216-2 P265GH");
        assert_eq!(material_map("A234WPB"), "ASTM A234 WPB");
        assert_eq!(material_map("CUSTOM-MAT"), "CUSTOM-MAT"); // passthrough
    }

    #[test]
    fn test_unit_conversion_barg_to_pa() {
        // 15 barg → ~1.61 MPa abs
        let pa = barg_to_pa_abs(15.0);
        assert!((pa - 1_613_250.0).abs() < 100.0, "Got {pa}");
        // 0 barg → 101325 Pa abs
        let atm = barg_to_pa_abs(0.0);
        assert!((atm - 101_325.0).abs() < 1.0);
    }

    #[test]
    fn test_unit_conversion_degc_to_k() {
        assert!((degc_to_k(0.0) - 273.15).abs() < 0.01);
        assert!((degc_to_k(100.0) - 373.15).abs() < 0.01);
        assert!((degc_to_k(-273.15) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_map_equipment_pump() {
        let mut mapper = CadmaticFieldMapper::new("test".to_owned());
        let pump = mock::mock_pump();
        let result = mapper.map_equipment(&pump).unwrap();
        assert_eq!(result["@type"], "pmef:Pump");
        assert!(result["@id"].as_str().unwrap().contains("P-201A"));
        assert_eq!(result["equipmentBasic"]["tagNumber"], "P-201A");
        assert_eq!(result["equipmentBasic"]["equipmentClass"], "CENTRIFUGAL_PUMP");
        let nozzles = result["nozzles"].as_array().unwrap();
        assert_eq!(nozzles.len(), 2);
        assert_eq!(nozzles[0]["nozzleId"], "SUCTION");
        assert_eq!(nozzles[0]["nominalDiameter"], 200.0);
    }

    #[test]
    fn test_map_pipeline() {
        let mut mapper = CadmaticFieldMapper::new("test".to_owned());
        let line = mock::mock_line();
        let result = mapper.map_pipeline(&line).unwrap();
        assert_eq!(result["@type"], "pmef:PipingNetworkSystem");
        assert_eq!(result["lineNumber"], "8\"-CW-201-A1A2");
        assert_eq!(result["pipeClass"], "A1A2");
        // Design pressure: 15 barg → ~1.613 MPa abs
        let dp = result["designConditions"]["designPressure"].as_f64().unwrap();
        assert!((dp - 1_601_325.0).abs() < 100.0, "designPressure={dp}");
        // Design temperature: 60°C → 333.15 K
        let dt = result["designConditions"]["designTemperature"].as_f64().unwrap();
        assert!((dt - 333.15).abs() < 0.1, "designTemperature={dt}");
    }

    #[test]
    fn test_map_segment_and_components() {
        let mut mapper = CadmaticFieldMapper::new("test".to_owned());
        let line = mock::mock_line();
        let components = vec![mock::mock_pipe_component(), mock::mock_elbow_component()];
        let (seg, comps, rels) = mapper.map_segment_and_components(&line, &components);

        assert_eq!(seg["@type"], "pmef:PipingSegment");
        assert_eq!(comps.len(), 2);
        assert_eq!(comps[0]["@type"], "pmef:Pipe");
        assert_eq!(comps[1]["@type"], "pmef:Elbow");
        assert_eq!(comps[1]["angle"], 90.0);
        assert_eq!(comps[1]["radius"], "LONG_RADIUS");
        // Pipe length: distance from (9000,5400,850) to (11500,5400,850) = 2500 mm
        let len = comps[0]["pipeLength"].as_f64().unwrap();
        assert!((len - 2500.0).abs() < 1.0, "pipeLength={len}");
        // HasEquivalentIn rels
        assert_eq!(rels.len(), 2);
        assert_eq!(rels[0]["targetSystem"], "CADMATIC");
    }

    #[test]
    fn test_has_equivalent_in() {
        let mapper = CadmaticFieldMapper::new("test".to_owned());
        let rel = mapper.make_has_equivalent_in(
            "urn:pmef:obj:test:P-201A",
            "GUID-P-201A-0001",
        );
        assert_eq!(rel["@type"], "pmef:HasEquivalentIn");
        assert_eq!(rel["targetSystem"], "CADMATIC");
        assert_eq!(rel["targetSystemId"], "GUID-P-201A-0001");
        assert_eq!(rel["mappingType"], "EXACT");
        assert_eq!(rel["confidence"], 1.0);
    }

    #[test]
    fn test_skey_to_flange_type() {
        assert_eq!(skey_to_flange_type(Some("FLWN")), "WELD_NECK");
        assert_eq!(skey_to_flange_type(Some("FLBLRF  ")), "BLIND");
        assert_eq!(skey_to_flange_type(Some("FLSO")), "SLIP_ON");
        assert_eq!(skey_to_flange_type(None), "WELD_NECK");
    }

    #[test]
    fn test_make_file_header() {
        let mapper = CadmaticFieldMapper::new("eaf-2026".to_owned());
        let hdr = mapper.make_file_header("EAF_2026");
        assert_eq!(hdr["@type"], "pmef:FileHeader");
        assert!(hdr["@id"].as_str().unwrap().contains("eaf-2026"));
        assert_eq!(hdr["coordinateSystem"], "Z-up");
        assert_eq!(hdr["units"], "mm");
    }
}
