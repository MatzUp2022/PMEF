//! Core trait abstractions for the PMEF ecosystem.
//!
//! These traits define the contracts that all PMEF entity types, adapters,
//! and visitors must satisfy.

use crate::revision::RevisionMetadata;
use crate::types::PmefId;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;

// ────────────────────────────────────────────────────────────────────────────
// PmefEntityType
// ────────────────────────────────────────────────────────────────────────────

/// The full set of normatively defined PMEF entity types (v0.9).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PmefEntityType {
    // Plant hierarchy
    FileHeader,
    Plant,
    Unit,
    Area,
    // Piping
    PipingNetworkSystem,
    PipingSegment,
    Pipe,
    Elbow,
    Tee,
    Reducer,
    Flange,
    Valve,
    Olet,
    Gasket,
    Weld,
    PipeSupport,
    Spool,
    // Equipment
    Vessel,
    Tank,
    Pump,
    Compressor,
    HeatExchanger,
    Column,
    Reactor,
    Filter,
    Turbine,
    GenericEquipment,
    // E&I
    InstrumentObject,
    InstrumentLoop,
    PlcObject,
    CableObject,
    CableTrayRun,
    MtpModule,
    // Steel
    SteelSystem,
    SteelMember,
    SteelNode,
    SteelConnection,
    // Geometry
    ParametricGeometry,
    // Relationships
    IsPartOf,
    IsConnectedTo,
    IsDerivedFrom,
    Supports,
    ControlledBy,
    IsDocumentedBy,
    IsRevisionOf,
    HasEquivalentIn,
    IsCollocatedWith,
    ReplacedBy,
    // Unknown / extension
    Unknown(String),
}

impl PmefEntityType {
    /// Parse the `@type` string from a PMEF NDJSON object.
    pub fn from_type_str(s: &str) -> Self {
        match s {
            "pmef:FileHeader"            => Self::FileHeader,
            "pmef:Plant"                 => Self::Plant,
            "pmef:Unit"                  => Self::Unit,
            "pmef:Area"                  => Self::Area,
            "pmef:PipingNetworkSystem"   => Self::PipingNetworkSystem,
            "pmef:PipingSegment"         => Self::PipingSegment,
            "pmef:Pipe"                  => Self::Pipe,
            "pmef:Elbow"                 => Self::Elbow,
            "pmef:Tee"                   => Self::Tee,
            "pmef:Reducer"               => Self::Reducer,
            "pmef:Flange"                => Self::Flange,
            "pmef:Valve"                 => Self::Valve,
            "pmef:Olet"                  => Self::Olet,
            "pmef:Gasket"                => Self::Gasket,
            "pmef:Weld"                  => Self::Weld,
            "pmef:PipeSupport"           => Self::PipeSupport,
            "pmef:Spool"                 => Self::Spool,
            "pmef:Vessel"                => Self::Vessel,
            "pmef:Tank"                  => Self::Tank,
            "pmef:Pump"                  => Self::Pump,
            "pmef:Compressor"            => Self::Compressor,
            "pmef:HeatExchanger"         => Self::HeatExchanger,
            "pmef:Column"                => Self::Column,
            "pmef:Reactor"               => Self::Reactor,
            "pmef:Filter"                => Self::Filter,
            "pmef:Turbine"               => Self::Turbine,
            "pmef:GenericEquipment"      => Self::GenericEquipment,
            "pmef:InstrumentObject"      => Self::InstrumentObject,
            "pmef:InstrumentLoop"        => Self::InstrumentLoop,
            "pmef:PLCObject"             => Self::PlcObject,
            "pmef:CableObject"           => Self::CableObject,
            "pmef:CableTrayRun"          => Self::CableTrayRun,
            "pmef:MTPModule"             => Self::MtpModule,
            "pmef:SteelSystem"           => Self::SteelSystem,
            "pmef:SteelMember"           => Self::SteelMember,
            "pmef:SteelNode"             => Self::SteelNode,
            "pmef:SteelConnection"       => Self::SteelConnection,
            "pmef:ParametricGeometry"    => Self::ParametricGeometry,
            "pmef:IsPartOf"              => Self::IsPartOf,
            "pmef:IsConnectedTo"         => Self::IsConnectedTo,
            "pmef:IsDerivedFrom"         => Self::IsDerivedFrom,
            "pmef:Supports"              => Self::Supports,
            "pmef:ControlledBy"          => Self::ControlledBy,
            "pmef:IsDocumentedBy"        => Self::IsDocumentedBy,
            "pmef:IsRevisionOf"          => Self::IsRevisionOf,
            "pmef:HasEquivalentIn"       => Self::HasEquivalentIn,
            "pmef:IsCollocatedWith"      => Self::IsCollocatedWith,
            "pmef:ReplacedBy"            => Self::ReplacedBy,
            other                        => Self::Unknown(other.to_owned()),
        }
    }

    /// The canonical `@type` string for this entity type.
    pub fn as_type_str(&self) -> &str {
        match self {
            Self::FileHeader            => "pmef:FileHeader",
            Self::Plant                 => "pmef:Plant",
            Self::Unit                  => "pmef:Unit",
            Self::Area                  => "pmef:Area",
            Self::PipingNetworkSystem   => "pmef:PipingNetworkSystem",
            Self::PipingSegment         => "pmef:PipingSegment",
            Self::Pipe                  => "pmef:Pipe",
            Self::Elbow                 => "pmef:Elbow",
            Self::Tee                   => "pmef:Tee",
            Self::Reducer               => "pmef:Reducer",
            Self::Flange                => "pmef:Flange",
            Self::Valve                 => "pmef:Valve",
            Self::Olet                  => "pmef:Olet",
            Self::Gasket                => "pmef:Gasket",
            Self::Weld                  => "pmef:Weld",
            Self::PipeSupport           => "pmef:PipeSupport",
            Self::Spool                 => "pmef:Spool",
            Self::Vessel                => "pmef:Vessel",
            Self::Tank                  => "pmef:Tank",
            Self::Pump                  => "pmef:Pump",
            Self::Compressor            => "pmef:Compressor",
            Self::HeatExchanger         => "pmef:HeatExchanger",
            Self::Column                => "pmef:Column",
            Self::Reactor               => "pmef:Reactor",
            Self::Filter                => "pmef:Filter",
            Self::Turbine               => "pmef:Turbine",
            Self::GenericEquipment      => "pmef:GenericEquipment",
            Self::InstrumentObject      => "pmef:InstrumentObject",
            Self::InstrumentLoop        => "pmef:InstrumentLoop",
            Self::PlcObject             => "pmef:PLCObject",
            Self::CableObject           => "pmef:CableObject",
            Self::CableTrayRun          => "pmef:CableTrayRun",
            Self::MtpModule             => "pmef:MTPModule",
            Self::SteelSystem           => "pmef:SteelSystem",
            Self::SteelMember           => "pmef:SteelMember",
            Self::SteelNode             => "pmef:SteelNode",
            Self::SteelConnection       => "pmef:SteelConnection",
            Self::ParametricGeometry    => "pmef:ParametricGeometry",
            Self::IsPartOf              => "pmef:IsPartOf",
            Self::IsConnectedTo         => "pmef:IsConnectedTo",
            Self::IsDerivedFrom         => "pmef:IsDerivedFrom",
            Self::Supports              => "pmef:Supports",
            Self::ControlledBy          => "pmef:ControlledBy",
            Self::IsDocumentedBy        => "pmef:IsDocumentedBy",
            Self::IsRevisionOf          => "pmef:IsRevisionOf",
            Self::HasEquivalentIn       => "pmef:HasEquivalentIn",
            Self::IsCollocatedWith      => "pmef:IsCollocatedWith",
            Self::ReplacedBy            => "pmef:ReplacedBy",
            Self::Unknown(s)            => s.as_str(),
        }
    }

    /// Returns true for relationship entity types.
    pub fn is_relationship(&self) -> bool {
        matches!(self,
            Self::IsPartOf | Self::IsConnectedTo | Self::IsDerivedFrom |
            Self::Supports | Self::ControlledBy | Self::IsDocumentedBy |
            Self::IsRevisionOf | Self::HasEquivalentIn | Self::IsCollocatedWith |
            Self::ReplacedBy
        )
    }

    /// Returns true for geometry entity types.
    pub fn is_geometry(&self) -> bool {
        matches!(self, Self::ParametricGeometry)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// PmefEntity — core trait for all PMEF objects
// ────────────────────────────────────────────────────────────────────────────

/// The core trait implemented by every PMEF entity type.
///
/// Provides identity, type, revision, and serialisation operations.
///
/// # Example
/// ```ignore
/// use pmef_core::traits::PmefEntity;
/// let pump = Pump::default(); // hypothetical
/// println!("{}", pump.id());
/// ```
pub trait PmefEntity: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// The entity type discriminator.
    fn entity_type() -> PmefEntityType
    where
        Self: Sized;

    /// The unique identifier of this object.
    fn id(&self) -> &PmefId;

    /// The revision metadata of this object, if present.
    fn revision(&self) -> Option<&RevisionMetadata>;

    /// Mutable access to revision metadata.
    fn revision_mut(&mut self) -> Option<&mut RevisionMetadata>;

    /// The `isPartOf` reference, if this object has one.
    fn is_part_of(&self) -> Option<&PmefId> { None }

    /// Custom attributes (project-specific extension fields).
    fn custom_attributes(&self) -> Option<&HashMap<String, serde_json::Value>> { None }

    /// Serialise to a compact JSON string (one line, for NDJSON output).
    fn to_ndjson_line(&self) -> Result<String, serde_json::Error>
    where
        Self: Sized,
    {
        serde_json::to_string(self)
    }
}

// ────────────────────────────────────────────────────────────────────────────
// PmefVisitor — visitor pattern over a PMEF package
// ────────────────────────────────────────────────────────────────────────────

/// Visitor pattern for processing PMEF objects without knowing their concrete types.
///
/// Implement this trait to build readers, indexers, exporters, or transformers.
///
/// # Example
/// ```ignore
/// struct TagCounter { count: usize }
///
/// impl PmefVisitor for TagCounter {
///     fn visit_pump(&mut self, pump: &Pump) -> VisitorResult {
///         self.count += 1;
///         Ok(VisitorControl::Continue)
///     }
/// }
/// ```
pub type VisitorResult = Result<VisitorControl, Box<dyn std::error::Error + Send + Sync>>;

/// Control flow returned by visitor methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisitorControl {
    /// Continue processing subsequent objects.
    Continue,
    /// Skip to the next top-level object (skip children).
    Skip,
    /// Stop processing the entire package.
    Stop,
}

/// Visitor trait over all PMEF entity types.
///
/// Default implementations do nothing and return `Ok(Continue)`.
#[allow(unused_variables)]
pub trait PmefVisitor {
    // ── Plant hierarchy ──────────────────────────────────────
    fn visit_file_header(&mut self, obj: &crate::plant::FileHeader) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_plant(&mut self, obj: &crate::plant::Plant) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_unit(&mut self, obj: &crate::plant::Unit) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_area(&mut self, obj: &crate::plant::Area) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }

    // ── Piping ───────────────────────────────────────────────
    fn visit_piping_network_system(
        &mut self, obj: &crate::piping::PipingNetworkSystem,
    ) -> VisitorResult { Ok(VisitorControl::Continue) }

    fn visit_piping_segment(
        &mut self, obj: &crate::piping::PipingSegment,
    ) -> VisitorResult { Ok(VisitorControl::Continue) }

    fn visit_pipe(&mut self, obj: &crate::piping::Pipe) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_elbow(&mut self, obj: &crate::piping::Elbow) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_tee(&mut self, obj: &crate::piping::Tee) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_reducer(&mut self, obj: &crate::piping::Reducer) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_flange(&mut self, obj: &crate::piping::Flange) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_valve(&mut self, obj: &crate::piping::Valve) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_olet(&mut self, obj: &crate::piping::Olet) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_gasket(&mut self, obj: &crate::piping::Gasket) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_weld(&mut self, obj: &crate::piping::Weld) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_pipe_support(&mut self, obj: &crate::piping::PipeSupport) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_spool(&mut self, obj: &crate::piping::Spool) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }

    // ── Equipment ────────────────────────────────────────────
    fn visit_pump(&mut self, obj: &crate::equipment::Pump) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_vessel(&mut self, obj: &crate::equipment::Vessel) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_heat_exchanger(
        &mut self, obj: &crate::equipment::HeatExchanger,
    ) -> VisitorResult { Ok(VisitorControl::Continue) }
    fn visit_compressor(&mut self, obj: &crate::equipment::Compressor) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_generic_equipment(
        &mut self, obj: &crate::equipment::GenericEquipment,
    ) -> VisitorResult { Ok(VisitorControl::Continue) }

    // ── E&I ─────────────────────────────────────────────────
    fn visit_instrument_object(
        &mut self, obj: &crate::ei::InstrumentObject,
    ) -> VisitorResult { Ok(VisitorControl::Continue) }
    fn visit_instrument_loop(
        &mut self, obj: &crate::ei::InstrumentLoop,
    ) -> VisitorResult { Ok(VisitorControl::Continue) }
    fn visit_plc_object(&mut self, obj: &crate::ei::PlcObject) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }

    // ── Steel ────────────────────────────────────────────────
    fn visit_steel_member(&mut self, obj: &crate::steel::SteelMember) -> VisitorResult {
        Ok(VisitorControl::Continue)
    }
    fn visit_steel_connection(
        &mut self, obj: &crate::steel::SteelConnection,
    ) -> VisitorResult { Ok(VisitorControl::Continue) }

    // ── Relationships ────────────────────────────────────────
    fn visit_has_equivalent_in(
        &mut self, obj: &crate::relationships::HasEquivalentIn,
    ) -> VisitorResult { Ok(VisitorControl::Continue) }
    fn visit_is_derived_from(
        &mut self, obj: &crate::relationships::IsDerivedFrom,
    ) -> VisitorResult { Ok(VisitorControl::Continue) }

    /// Catch-all for unknown entity types.
    fn visit_unknown(
        &mut self,
        type_str: &str,
        raw: &serde_json::Value,
    ) -> VisitorResult { Ok(VisitorControl::Continue) }
}

// ────────────────────────────────────────────────────────────────────────────
// PmefAdapter — bidirectional adapter trait
// ────────────────────────────────────────────────────────────────────────────

/// Result type for adapter operations.
pub type AdapterResult<T> = Result<T, AdapterError>;

/// Errors that can occur during adapter operations.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Schema mapping error for field '{field}': {msg}")]
    Mapping { field: String, msg: String },

    #[error("Unit conversion error: {0}")]
    UnitConversion(String),

    #[error("Validation error: {object_id} — {msg}")]
    Validation { object_id: String, msg: String },

    #[error("Identity resolution failed for native ID '{native_id}': {msg}")]
    IdentityResolution { native_id: String, msg: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

/// Statistics reported by an adapter after a processing run.
#[derive(Debug, Default, Clone)]
pub struct AdapterStats {
    /// Number of objects successfully exported or imported.
    pub objects_ok: usize,
    /// Number of objects that failed validation or mapping.
    pub objects_failed: usize,
    /// Number of fields that could not be mapped.
    pub fields_unmapped: usize,
    /// Number of objects skipped (e.g. filtered by domain).
    pub objects_skipped: usize,
    /// Duration of the processing run.
    pub duration_ms: u64,
}

/// The bidirectional adapter trait.
///
/// Implementors translate between PMEF NDJSON and a specific engineering tool.
///
/// # Example
/// ```ignore
/// use pmef_core::traits::{PmefAdapter, AdapterStats};
///
/// struct Plant3DAdapter { /* connection state */ }
///
/// #[async_trait::async_trait]
/// impl PmefAdapter for Plant3DAdapter {
///     fn name(&self) -> &str { "pmef-adapter-plant3d" }
///     fn version(&self) -> &str { "0.9.0" }
///     fn target_system(&self) -> &str { "PLANT3D" }
///
///     async fn export_to_pmef(&self, output: &mut dyn tokio::io::AsyncWrite + Unpin)
///         -> Result<AdapterStats, AdapterError> { todo!() }
///
///     async fn import_from_pmef(&self, input: &mut dyn tokio::io::AsyncRead + Unpin)
///         -> Result<AdapterStats, AdapterError> { todo!() }
/// }
/// ```
pub trait PmefAdapter: Send + Sync {
    /// Short adapter identifier, e.g. `"pmef-adapter-plant3d"`.
    fn name(&self) -> &str;

    /// Adapter version string.
    fn version(&self) -> &str;

    /// Target system identifier, e.g. `"PLANT3D"`, `"AVEVA_E3D"`, `"CADMATIC"`.
    fn target_system(&self) -> &str;

    /// Domains supported by this adapter.
    fn supported_domains(&self) -> &[&str];

    /// Conformance level claimed by this adapter (1, 2, or 3).
    fn conformance_level(&self) -> u8;

    /// Human-readable description of the adapter.
    fn description(&self) -> &str { "" }
}
