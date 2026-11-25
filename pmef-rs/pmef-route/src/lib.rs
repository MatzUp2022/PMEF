//! # pmef-route — graph and rule kernel for automatic pipe routing
//!
//! Routing is solved in an abstract graph first and turned into geometry afterwards:
//!
//! 1. **Rules** ([`rules`]): outside diameter and 90° elbow centre-to-face for the line's DN come
//!    from the PMEF piping-class catalog; design pressure/temperature are checked against the
//!    class limits.
//! 2. **Graph** ([`grid`]): a rectilinear voxel graph over the region, with the terminals pinned
//!    as exact nodes. Obstacles (PMEF equipment boxes, existing piping, OBJ meshes from IFC/CAD)
//!    are rasterised grown by pipe radius + insulation + clearance. Soft obstacles (walls,
//!    slabs) may be crossed straight at a penalty.
//! 3. **Search** ([`search`]): orthogonal A* with elbow-fit constraints (minimum straight runs
//!    between bends from the catalog), bend penalty, vertical factor and preferred zones.
//! 4. **Geometry** ([`pmef_out`]): the centreline polyline becomes PMEF `Pipe` and `Elbow`
//!    components with ports, catalog references and provenance `sourceType: generated`.
//!
//! Lines are routed in order; each routed line is an obstacle for the next. Design and later
//! increments: `docs/auto-routing.md`.

pub mod cable;
pub mod geom;
pub mod grid;
pub mod pmef_out;
pub mod problem;
pub mod route;
pub mod rules;
pub mod scene;
pub mod search;

pub use problem::RouteProblem;
pub use route::{route_all, RoutedLine};
pub use scene::Scene;

/// Name and version written to generated objects.
pub const GENERATOR: &str = concat!("pmef-route ", env!("CARGO_PKG_VERSION"));
