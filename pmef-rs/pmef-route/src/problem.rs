//! Routing problem description (JSON input of `pmef route`).

use crate::geom::{Aabb, P3};
use serde::{Deserialize, Serialize};

/// A complete routing job: scene, preferences and the lines to route (in order).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteProblem {
    /// Project code used in generated `@id`s (`urn:pmef:obj:<project>:…`).
    pub project: String,
    /// Revision id written to generated objects (`r<ISO-date>-<seq>`).
    pub revision_id: String,
    /// Parent (`Unit`/`Plant` `@id`) for lines the router creates.
    #[serde(default)]
    pub parent: Option<String>,
    /// Restrict all routes to this region [mm]. Default: terminals of each route ± margin.
    #[serde(default)]
    pub bounds: Option<Aabb>,
    #[serde(default)]
    pub grid: GridSettings,
    #[serde(default)]
    pub settings: Settings,
    /// PMEF NDJSON files whose objects form the scene (equipment boxes, existing piping) and
    /// whose nozzles can be used as terminals. Paths relative to the problem file.
    #[serde(default)]
    pub pmef_scene: Vec<String>,
    /// Scene objects to leave out (exact `@id` or prefix ending in `-` or `:`), e.g. a line
    /// that is being re-routed.
    #[serde(default)]
    pub pmef_ignore: Vec<String>,
    #[serde(default)]
    pub obstacles: Vec<ObstacleSpec>,
    #[serde(default)]
    pub zones: Vec<Zone>,
    /// Pipe racks: their levels are preferred edges of the graph.
    #[serde(default)]
    pub racks: Vec<Rack>,
    /// Place pipe supports on the routed lines (off when absent).
    #[serde(default)]
    pub supports: Option<SupportSettings>,
    pub routes: Vec<RouteSpec>,
}

/// Graph resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GridSettings {
    /// Node spacing [mm].
    #[serde(default = "d_step")]
    pub step: f64,
    /// Extra space around the terminals of a route when no `bounds` are given [mm].
    #[serde(default = "d_margin")]
    pub margin: f64,
    /// Coarse-to-fine search: route on a grid with this spacing first [mm], then on `step`
    /// only inside a corridor around the coarse route. Falls back to the full fine grid if the
    /// coarse or the corridor search fails. `None`: fine grid only.
    #[serde(default)]
    pub coarse_step: Option<f64>,
    /// Half-width of the corridor around the coarse route [mm] (default: `coarseStep`).
    #[serde(default)]
    pub corridor: Option<f64>,
}
fn d_step() -> f64 {
    100.0
}
fn d_margin() -> f64 {
    3000.0
}
impl Default for GridSettings {
    fn default() -> Self {
        GridSettings {
            step: d_step(),
            margin: d_margin(),
            coarse_step: None,
            corridor: None,
        }
    }
}

/// Cost model and default clearances.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    /// Equivalent length of one elbow [mm] (fewer bends = less fittings, welds, pressure drop).
    #[serde(default = "d_bend")]
    pub bend_penalty: f64,
    /// Cost factor for vertical runs (> 1 keeps lines on their elevation).
    #[serde(default = "d_one")]
    pub vertical_factor: f64,
    /// Free gap between the outer surface (incl. insulation) and any obstacle [mm].
    #[serde(default = "d_clear")]
    pub clearance: f64,
    /// Default penalty for passing through a soft obstacle (wall/slab sleeve) [mm equivalent].
    #[serde(default = "d_pen")]
    pub penetration_penalty: f64,
    /// Straight length required at a terminal before the first / after the last elbow, in
    /// addition to the elbow centre-to-face [mm].
    #[serde(default)]
    pub terminal_straight: f64,
    #[serde(default = "d_max")]
    pub max_expansions: usize,
    /// Rip-up and reroute rounds after the sequential pass (0 = sequential only). Each round
    /// takes a failed line, rips up the lines that block it, routes it first and re-routes
    /// them.
    #[serde(default = "d_rip")]
    pub rip_up_rounds: usize,
}
fn d_rip() -> usize {
    5
}
fn d_bend() -> f64 {
    1000.0
}
fn d_one() -> f64 {
    1.0
}
fn d_clear() -> f64 {
    50.0
}
fn d_pen() -> f64 {
    5000.0
}
fn d_max() -> usize {
    30_000_000
}
impl Default for Settings {
    fn default() -> Self {
        Settings {
            bend_penalty: d_bend(),
            vertical_factor: d_one(),
            clearance: d_clear(),
            penetration_penalty: d_pen(),
            terminal_straight: 0.0,
            max_expansions: d_max(),
            rip_up_rounds: d_rip(),
        }
    }
}

/// Obstacle kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ObstacleKind {
    /// Must not be touched (equipment, structure, other lines).
    #[default]
    Hard,
    /// May be crossed straight with a penalty (walls, slabs: sleeve / core drilling).
    Soft,
}

/// Explicit obstacle: a box or a Wavefront OBJ mesh.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObstacleSpec {
    pub id: String,
    #[serde(default)]
    pub kind: ObstacleKind,
    /// Penetration penalty for soft obstacles (default: `settings.penetrationPenalty`).
    #[serde(default)]
    pub penalty: Option<f64>,
    #[serde(default)]
    pub aabb: Option<Aabb>,
    /// OBJ file (triangles, mm), relative to the problem file.
    #[serde(default)]
    pub obj: Option<String>,
}

/// Region with a cost factor (< 1 = preferred corridor such as a pipe rack, > 1 = avoid).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Zone {
    pub aabb: Aabb,
    pub factor: f64,
}

/// Pipe support placement on horizontal runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SupportSettings {
    /// Maximum distance between supports [mm]. Default: suggested span of ASME B31.1
    /// Table 121.5 (water service) for the line's size.
    #[serde(default)]
    pub max_span: Option<f64>,
    /// Steel whose top lies at most this far below the pipe carries a support [mm].
    #[serde(default = "d_reach")]
    pub reach: f64,
}
fn d_reach() -> f64 {
    1000.0
}

/// Horizontal axis of a pipe rack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RackAxis {
    X,
    Y,
}

/// Pipe rack: a straight, horizontal corridor with one or more levels (tiers). Lines that run
/// along the rack axis on a level cost `factor` per mm instead of 1, so routes are drawn onto
/// the rack. The steel itself comes from the scene (PMEF steel, OBJ or boxes).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rack {
    pub id: String,
    pub axis: RackAxis,
    /// Plan footprint of the rack (x/y extent) [mm]; z is ignored.
    pub footprint: Aabb,
    /// Top-of-steel elevations of the levels [mm].
    pub levels: Vec<f64>,
    /// Cost factor along the rack (< 1 = preferred).
    #[serde(default = "d_rack_factor")]
    pub factor: f64,
    /// Height of the pipe shoe between top of steel and the bottom of the pipe (incl.
    /// insulation) [mm]. At least the clearance is kept.
    #[serde(default = "d_shoe")]
    pub shoe_height: f64,
}
fn d_rack_factor() -> f64 {
    0.5
}
fn d_shoe() -> f64 {
    100.0
}

impl Rack {
    /// Centreline elevations of a line with outer radius `outer` resting on each level.
    pub fn centrelines(&self, outer: f64, clearance: f64) -> Vec<f64> {
        let gap = self.shoe_height.max(clearance + 1.0);
        self.levels.iter().map(|l| l + gap + outer).collect()
    }
}

/// End point of a route.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Terminal {
    /// Connection point [mm].
    #[serde(default)]
    pub point: Option<P3>,
    /// Outward direction of the connection (nozzle axis, pointing away from the equipment).
    #[serde(default)]
    pub direction: Option<P3>,
    /// Nozzle of a scene object: `<@id>#<nozzleMark or nozzleId>`.
    #[serde(default)]
    pub nozzle: Option<String>,
    /// Written to the generated end port's `connectedTo`.
    #[serde(default)]
    pub connected_to: Option<String>,
    /// Obstacle id this terminal belongs to (equipment `@id`, fixture). Its clearance zone is
    /// waived along the terminal stub; every other obstacle still blocks the stub.
    /// Default: the equipment of `nozzle`, else `connectedTo`.
    #[serde(default)]
    pub owner: Option<String>,
    /// Straight length required at this terminal before the first elbow or tee, in addition
    /// to its centre-to-face [mm] (e.g. pump suction, flow meter inlet). Overrides
    /// `settings.terminalStraight`.
    #[serde(default)]
    pub straight: Option<f64>,
}

/// One line to route.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteSpec {
    /// Short route id, used in generated `@id`s.
    pub id: String,
    pub from: Terminal,
    pub to: Terminal,
    /// Piping class designation (PMEF catalog `pipeClass`), e.g. `A1A2`.
    pub pipe_class: String,
    /// Nominal diameter [mm].
    pub dn: u32,
    /// Existing `PipingNetworkSystem` `@id`; otherwise the router creates a line.
    #[serde(default)]
    pub line: Option<String>,
    /// Line number tag for a created line.
    #[serde(default)]
    pub line_number: Option<String>,
    /// Insulation thickness [mm].
    #[serde(default)]
    pub insulation: f64,
    /// Radial allowance for heat tracing on top of the insulation [mm].
    #[serde(default)]
    pub tracing: f64,
    /// Overrides `settings.clearance` [mm].
    #[serde(default)]
    pub clearance: Option<f64>,
    /// Design pressure [Pa] checked against the class.
    #[serde(default)]
    pub design_pressure: Option<f64>,
    /// Design temperature [K] checked against the class.
    #[serde(default)]
    pub design_temperature: Option<f64>,
    /// Further consumers of the same line, each connected by an equal tee to the line's tree
    /// (the run from `from` to `to` and the branches routed before it).
    #[serde(default)]
    pub branches: Vec<BranchSpec>,
}

impl RouteSpec {
    /// Outer radius incl. insulation and tracing allowance for outside diameter `od` [mm].
    pub fn outer(&self, od: f64) -> f64 {
        od / 2.0 + self.insulation + self.tracing
    }
}

/// A branch of a line: an equal tee on the line's tree and a run to `to`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BranchSpec {
    /// Short branch id, used in generated `@id`s.
    pub id: String,
    pub to: Terminal,
}
