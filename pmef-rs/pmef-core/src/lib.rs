//! # pmef-core
//!
//! PMEF data model: entity types, property sets, relationships,
//! and the core trait abstractions.
//!
//! ## Crate structure
//!
//! ```text
//! pmef-core
//! ├── types/         — primitive value types (PmefId, Coordinate3D, …)
//! ├── revision/      — RevisionMetadata, ChangeState
//! ├── geometry/      — GeometryReference, GeometryLayer
//! ├── catalog/       — CatalogReference, Port
//! ├── plant/         — Plant, Unit, Area, FileHeader
//! ├── piping/        — PipingNetworkSystem, PipingComponent subtypes
//! ├── equipment/     — EquipmentObject subtypes, Nozzle
//! ├── ei/            — InstrumentObject, PLCObject, CableObject, …
//! ├── steel/         — SteelMember, SteelNode, SteelConnection
//! ├── relationships/ — typed relationship objects
//! └── traits/        — PmefObject, PmefVisitor, Adapter
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod catalog;
pub mod ei;
pub mod equipment;
pub mod geometry;
pub mod plant;
pub mod piping;
pub mod relationships;
pub mod revision;
pub mod steel;
pub mod traits;
pub mod types;

// Convenience re-exports
pub use catalog::{CatalogReference, DocumentLink, Port, VendorMapping};
pub use ei::{CableObject, CableTrayRun, InstrumentLoop, InstrumentObject, MtpModule, PlcObject};
pub use equipment::{
    Compressor, EquipmentBasic, Filter, GenericEquipment, HeatExchanger, Nozzle,
    Pump, Reactor, Tank, Turbine, Vessel,
};
pub use geometry::{GeometryLayer, GeometryReference, Lod};
pub use plant::{Area, FileHeader, Plant, Unit};
pub use piping::{
    Elbow, Flange, Gasket, Olet, Pipe, PipeSupport, PipingNetworkSystem,
    PipingSegment, Reducer, Spool, Tee, Valve, Weld,
};
pub use relationships::{
    ControlledBy, HasEquivalentIn, IsCollocatedWith, IsConnectedTo, IsDerivedFrom,
    IsDocumentedBy, IsPartOf, IsRevisionOf, ReplacedBy, Supports,
};
pub use revision::{ChangeState, RevisionMetadata};
pub use steel::{SteelConnection, SteelMember, SteelNode, SteelSystem};
pub use traits::{PmefAdapter, PmefEntity, PmefEntityType, PmefVisitor};
pub use types::{
    Coordinate3D, Iec81346Designation, PmefId, PmefVersion, RdlUri, UnitVector3D,
};
