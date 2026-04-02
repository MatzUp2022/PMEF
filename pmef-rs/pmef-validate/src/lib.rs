//! PMEF JSON Schema validation and conformance checking.
pub fn schema_for_type(type_str: &str) -> Option<&'static str> {
    match type_str {
        "pmef:PipingNetworkSystem" | "pmef:PipingSegment" | "pmef:Pipe" |
        "pmef:Elbow" | "pmef:Tee" | "pmef:Reducer" | "pmef:Flange" |
        "pmef:Valve" | "pmef:Olet" | "pmef:Gasket" | "pmef:Weld" |
        "pmef:PipeSupport" | "pmef:Spool"
            => Some("pmef-piping-component.schema.json"),
        "pmef:Vessel" | "pmef:Tank" | "pmef:Pump" | "pmef:Compressor" |
        "pmef:HeatExchanger" | "pmef:Column" | "pmef:Reactor" |
        "pmef:Filter" | "pmef:Turbine" | "pmef:GenericEquipment"
            => Some("pmef-equipment.schema.json"),
        "pmef:InstrumentObject" | "pmef:InstrumentLoop" | "pmef:PLCObject" |
        "pmef:CableObject" | "pmef:CableTrayRun" | "pmef:MTPModule"
            => Some("pmef-ei.schema.json"),
        "pmef:SteelSystem" | "pmef:SteelMember" | "pmef:SteelNode" | "pmef:SteelConnection"
            => Some("pmef-steel.schema.json"),
        "pmef:ParametricGeometry" => Some("pmef-geometry.schema.json"),
        _ => None
    }
}
