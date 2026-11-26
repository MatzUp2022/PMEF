//! The whole routing run behind `pmef route` and the WASM binding: scene, line lists, pipes,
//! cables, PMEF output and report. Files are read through a [`Reader`], so the run needs no
//! file system.

use crate::cable::{route_cables, RoutedCable, TrayFill};
use crate::line_list::{expand, SkippedLine};
use crate::pmef_out::{cables_to_pmef, to_pmef};
use crate::problem::RouteSpec;
use crate::route::{route_all, RoutedLine};
use crate::scene::{Reader, Scene, SceneCache, SceneError};
use crate::{RouteProblem, GENERATOR};
use serde_json::{json, Value};

/// Result of [`run`].
#[derive(Debug, Clone)]
pub struct RunOutput {
    /// PMEF objects (file header first when a plant was given).
    pub objects: Vec<Value>,
    pub routes: Vec<RoutedLine>,
    pub cables: Vec<RoutedCable>,
    pub tray_fill: Vec<TrayFill>,
    /// Lines of the line lists that were not routed, with the reason.
    pub skipped_lines: Vec<SkippedLine>,
    /// The routes generated from the line lists (as they were routed): a viewer copies one into
    /// `routes` to edit and re-route that line alone.
    pub line_list_routes: Vec<RouteSpec>,
    /// The problem had cables or line lists: the report is an object, not the route array.
    extended: bool,
}

impl RunOutput {
    /// Every route and cable was routed.
    pub fn ok(&self) -> bool {
        self.routes.iter().all(|r| r.ok) && self.cables.iter().all(|c| c.ok)
    }

    /// The PMEF objects as NDJSON.
    pub fn ndjson(&self) -> String {
        let mut out = String::new();
        for o in &self.objects {
            out.push_str(&o.to_string());
            out.push('\n');
        }
        out
    }

    /// The report: the array of routed lines, or `{routes, cables, trayFill, skippedLines,
    /// lineListRoutes}` when the problem has cables or line lists.
    pub fn report(&self) -> Value {
        if self.extended {
            json!({
                "routes": self.routes, "cables": self.cables, "trayFill": self.tray_fill,
                "skippedLines": self.skipped_lines, "lineListRoutes": self.line_list_routes,
            })
        } else {
            json!(self.routes)
        }
    }
}

/// Route a problem. `plant` adds a `pmef:FileHeader` for that plant `@id`.
pub fn run(
    problem: RouteProblem,
    plant: Option<&str>,
    read: &Reader<'_>,
) -> Result<RunOutput, SceneError> {
    run_cached(problem, plant, read, &mut SceneCache::default())
}

/// As [`run`], taking parsed scene files from `cache` and adding those it lacks; repeated
/// runs over the same building model parse it once.
pub fn run_cached(
    mut problem: RouteProblem,
    plant: Option<&str>,
    read: &Reader<'_>,
    cache: &mut SceneCache,
) -> Result<RunOutput, SceneError> {
    let scene = Scene::load_cached(&problem, read, cache)?;
    let explicit = problem.routes.len();
    let skipped_lines = expand(&mut problem, &scene, read, cache)?;
    let line_list_routes = problem.routes[explicit..].to_vec();
    let routes = route_all(&problem, &scene);
    let (cables, tray_fill) = if problem.cables.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        route_cables(&problem)
    };
    let mut objects = Vec::new();
    if let Some(plant) = plant {
        objects.push(json!({
            "@type": "pmef:FileHeader",
            "@id": format!("urn:pmef:pkg:{}:auto-route", problem.project),
            "pmefVersion": "0.9.0", "plantId": plant, "projectCode": problem.project,
            "coordinateSystem": "Z-up", "units": "mm", "revisionId": problem.revision_id,
            "changeState": "WIP", "authoringTool": GENERATOR,
        }));
    }
    objects.extend(to_pmef(&problem, &routes));
    objects.extend(cables_to_pmef(&problem, &cables));
    Ok(RunOutput {
        objects,
        routes,
        cables,
        tray_fill,
        skipped_lines,
        line_list_routes,
        extended: !problem.cables.is_empty() || !problem.line_lists.is_empty(),
    })
}
