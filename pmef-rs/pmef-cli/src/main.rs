//! # pmef — PMEF Command Line Tool
//!
//! Usage:
//!   pmef validate <file.ndjson> [--schemas <dir>] [--level 1|2|3]
//!   pmef diff <base.ndjson> <head.ndjson> [--format text|json]
//!   pmef stats <file.ndjson>
//!   pmef convert <input.pcf> --to pmef --output <output.ndjson>
//!   pmef index <file.ndjson> [--key tag-number|line-number]
//!   pmef check <file.ndjson> [--json]
//!   pmef topology <file.ndjson> [--allow-external]
//!   pmef pack <main.ndjson> [more.ndjson…] -o <out.pmefx> [--bbox-lods] [--lod ID=LEVEL=FILE.glb] [--brep ID=FILE.stp]
//!   pmef unpack <file.pmefx> -o <dir>
//!   pmef verify <file.pmefx> [--json]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(
    name = "pmef",
    version = "0.9.0",
    about = "PMEF — Plant Model Exchange Format CLI",
    long_about = "Validate, diff, convert, and analyse PMEF NDJSON packages.\n\
                  Documentation: https://pmef.net/docs/cli"
)]
struct Cli {
    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate a PMEF NDJSON file against JSON Schemas.
    Validate {
        /// Path to the NDJSON file.
        file: PathBuf,
        /// Directory containing PMEF JSON Schema files.
        #[arg(long, default_value = "schemas")]
        schemas: PathBuf,
        /// Minimum conformance level to check (1, 2, or 3).
        #[arg(long, default_value_t = 1)]
        level: u8,
        /// Output format (text or json).
        #[arg(long, default_value = "text")]
        format: String,
        /// Fail on first error (default: report all errors).
        #[arg(long)]
        fail_fast: bool,
        /// Treat an unknown @type as an error. Default: skip with a warning (spec 06 CL1-03).
        #[arg(long)]
        strict: bool,
    },

    /// Compare two PMEF NDJSON files object by object (spec 03 §9).
    ///
    /// Objects are matched by @id; output is sorted by @id and field path, so it is
    /// deterministic. Exit code: 0 = no differences, 1 = differences, 2 = error.
    Diff {
        /// Base (old / design) file.
        base: PathBuf,
        /// Head (new / as-built) file.
        head: PathBuf,
        /// Output format: text or json.
        #[arg(long, default_value = "text", value_parser = ["text", "json"])]
        format: String,
        /// Compare numbers exactly instead of with the spec 06 §6.2 tolerances.
        #[arg(long)]
        exact: bool,
        /// Ignore a field path or path prefix (repeatable), e.g. --ignore revision.
        #[arg(long = "ignore", value_name = "PATH")]
        ignore: Vec<String>,
        /// Ignore the RFC-0001 recognition blocks (provenance, review, asBuilt) — for
        /// as-built vs. design comparisons that should show plant deviations only.
        #[arg(long)]
        ignore_recognition: bool,
        /// Match objects whose @id differs (tag, HasEquivalentIn, geometry, references; spec 03 §9.5).
        #[arg(long = "match")]
        match_objects: bool,
        /// Largest mean anchor distance for a geometric match [mm] (with --match).
        #[arg(long, default_value_t = 100.0, value_name = "MM")]
        match_distance: f64,
        /// Write the matches found by evidence as proposed as-built correspondences
        /// (pmef:IsRevisionOf, changeType AS_BUILT, review.status detected; RFC-0003) to FILE.
        #[arg(long, value_name = "FILE", requires = "match_objects")]
        emit_matches: Option<PathBuf>,
    },

    /// Three-way merge of PMEF NDJSON files (spec 03 §10).
    ///
    /// Takes changes made on only one side, reports differing changes on both sides as
    /// conflicts. Output: FileHeader first, then sorted by @id, keys sorted.
    /// Exit code: 0 = merged cleanly, 1 = conflicts (output written), 2 = error.
    Merge {
        /// Common ancestor.
        base: PathBuf,
        /// Our edit of base.
        ours: PathBuf,
        /// Their edit of base.
        theirs: PathBuf,
        /// Merged output file.
        #[arg(short, long)]
        output: PathBuf,
        /// Side written for conflicting values: ours or theirs.
        #[arg(long, default_value = "ours", value_parser = ["ours", "theirs"])]
        prefer: String,
        /// Compare numbers exactly instead of with the spec 06 §6.2 tolerances.
        #[arg(long)]
        exact: bool,
        /// Conflict report format: text or json.
        #[arg(long, default_value = "text", value_parser = ["text", "json"])]
        format: String,
    },

    /// Print statistics about a PMEF NDJSON file.
    Stats {
        /// Path to the NDJSON file.
        file: PathBuf,
        /// Show breakdown by @type.
        #[arg(long)]
        by_type: bool,
        /// Show breakdown by domain.
        #[arg(long)]
        by_domain: bool,
    },

    /// Convert between PMEF and other formats.
    Convert {
        /// Input: a PMEF NDJSON file (--to pcf) or a PCF file / directory of *.pcf (--to pmef).
        input: PathBuf,
        /// Target format: pcf (PMEF → AutoCAD Plant 3D) or pmef (Plant 3D PCF → PMEF).
        #[arg(long, value_parser = ["pcf", "pmef"])]
        to: String,
        /// Reference PMEF model updated from the PCF (required with --to pmef).
        #[arg(long, required_if_eq("to", "pmef"))]
        base: Option<PathBuf>,
        /// Output: a directory for PCF files (--to pcf) or an NDJSON file (--to pmef).
        #[arg(long, short)]
        output: PathBuf,
    },

    /// Build a lookup index from a PMEF package.
    Index {
        /// Path to the NDJSON file.
        file: PathBuf,
        /// Key field to index on.
        #[arg(long, default_value = "id")]
        key: String,
        /// Filter by @type.
        #[arg(long)]
        filter_type: Option<String>,
        /// Output file (default: stdout).
        #[arg(long, short)]
        output: Option<PathBuf>,
    },

    /// Pack PMEF NDJSON plus geometry into a PMEFX container (spec 03 §3, RFC-0005).
    ///
    /// The first NDJSON file is the main file. Writes the object index (every @id with its
    /// record, bounding box, mesh LODs and B-rep) and a loose octree for streaming.
    Pack {
        /// NDJSON files; stored as model/<file name>.
        #[arg(required = true)]
        files: Vec<PathBuf>,
        /// Output .pmefx file.
        #[arg(short, long)]
        output: PathBuf,
        /// Generate a BBOX_ONLY box mesh for every object with a bounding box.
        #[arg(long)]
        bbox_lods: bool,
        /// Mesh level of detail: <@id>=<LEVEL>=<file.glb>, LEVEL e.g. LOD1_COARSE (repeatable).
        #[arg(long = "lod", value_name = "ID=LEVEL=FILE")]
        lods: Vec<String>,
        /// Exact B-rep (STEP AP242): <@id>=<file.stp> (repeatable).
        #[arg(long = "brep", value_name = "ID=FILE")]
        breps: Vec<String>,
        /// Additional file: <type>=<file>, type catalog|document|gltf|step|usd|other (repeatable).
        #[arg(long = "file", value_name = "TYPE=FILE")]
        extra: Vec<String>,
        /// Mesh origin [mm]: auto (extent centre, rounded to 1 m), none, or x,y,z.
        #[arg(long, default_value = "auto")]
        mesh_origin: String,
        /// manifest.createdAt (default: now; set it for reproducible archives).
        #[arg(long)]
        created_at: Option<String>,
        /// Do not write index/octree.json.
        #[arg(long)]
        no_octree: bool,
    },

    /// Extract a PMEFX container into a directory.
    Unpack {
        file: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Verify a PMEFX container: file list, sizes, SHA-256, object index, octree (spec 03 §3).
    /// Exit code: 0 = conformant, 1 = problems found, 2 = error.
    Verify {
        file: PathBuf,
        /// Output the report as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Structural check of a PMEF package: one FileHeader first, every line readable,
    /// unique @id, all references resolve. This checks *data*; implementation conformance
    /// (spec 06) is tested with the suite: `python tests/run_tests.py`.
    /// Route pipes automatically (graph + piping-class rules) and write them as PMEF.
    ///
    /// Input is a routing problem (JSON, see pmef-route/README.md); lines are routed in order
    /// and each routed line is an obstacle for the next. Exit code 1 if any route failed.
    Route {
        /// Routing problem (JSON).
        problem: PathBuf,
        /// Output NDJSON with the generated lines.
        #[arg(long, short)]
        output: PathBuf,
        /// Route report (JSON: polylines, lengths, bends, search statistics).
        #[arg(long)]
        report: Option<PathBuf>,
        /// Write a FileHeader with this plantId first, so the output is a complete package.
        #[arg(long)]
        plant: Option<String>,
    },

    Check {
        /// PMEF package to check.
        file: PathBuf,
        /// Output the report as JSON.
        #[arg(long)]
        json: bool,
        /// Report file (default: stdout).
        #[arg(long, short)]
        output: Option<PathBuf>,
    },

    /// Deprecated alias of `check --json` (kept for existing scripts).
    #[command(hide = true)]
    Conformance {
        file: PathBuf,
        #[arg(long)]
        dataset: Option<String>,
        #[arg(long)]
        level: Option<u8>,
        #[arg(long)]
        json: bool,
        #[arg(long, short)]
        output: Option<PathBuf>,
    },

    /// Resolve all @id references within a PMEF package.
    CheckRefs {
        /// Path to the NDJSON file.
        file: PathBuf,
        /// Report missing references as errors (default: warnings).
        #[arg(long)]
        strict: bool,
    },

    /// Check dataset topology: unique @id, connectedTo resolution, reciprocity.
    Topology {
        /// Path to the NDJSON file.
        file: PathBuf,
        /// Treat unresolved connectedTo targets as warnings (cross-dataset boundaries).
        #[arg(long)]
        allow_external: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Configure logging
    let level = match cli.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level)),
        )
        .with_writer(std::io::stderr)
        .init();

    match cli.command {
        Commands::Validate { file, schemas, level, format, fail_fast, strict } => {
            cmd_validate(file, schemas, level, &format, fail_fast, strict).await
        }
        Commands::Diff { base, head, format, exact, mut ignore, ignore_recognition, match_objects, match_distance, emit_matches } => {
            if ignore_recognition {
                ignore.extend(["provenance", "review", "asBuilt"].map(String::from));
            }
            let matching = match_objects.then(|| pmef_core::matching::MatchOptions {
                max_distance_mm: match_distance,
                ..Default::default()
            });
            cmd_diff(base, head, &format, exact, ignore, matching, emit_matches)
        }
        Commands::Merge { base, ours, theirs, output, prefer, exact, format } => {
            cmd_merge(base, ours, theirs, output, &prefer, exact, &format)
        }
        Commands::Stats { file, by_type, by_domain } => {
            cmd_stats(file, by_type, by_domain).await
        }
        Commands::Convert { input, to, base, output } => cmd_convert(input, &to, base, output),
        Commands::Index { file, key, filter_type, output } => {
            cmd_index(file, &key, filter_type, output).await
        }
        Commands::Check { file, json, output } => cmd_check(file, json, output),
        Commands::Route { problem, output, report, plant } => cmd_route(problem, output, report, plant),
        Commands::Pack { files, output, bbox_lods, lods, breps, extra, mesh_origin, created_at, no_octree } => {
            cmd_pack(files, output, bbox_lods, lods, breps, extra, &mesh_origin, created_at, no_octree)
        }
        Commands::Unpack { file, output } => cmd_unpack(file, output),
        Commands::Verify { file, json } => cmd_verify(file, json),
        Commands::Conformance { file, output, .. } => {
            eprintln!("note: `pmef conformance` is deprecated; use `pmef check` (package check) or \
                       `python tests/run_tests.py` (implementation conformance, spec 06)");
            cmd_check(file, true, output)
        }
        Commands::CheckRefs { file, strict } => {
            cmd_check_refs(file, strict).await
        }
        Commands::Topology { file, allow_external } => {
            cmd_topology(file, allow_external).await
        }
    }
}

// ── Command implementations ───────────────────────────────────────────────

/// Line number of a reader error (0 if unknown).
fn read_error_line(e: &pmef_io::PmefIoError) -> usize {
    use pmef_io::PmefIoError::*;
    match e {
        Io { line, .. } | Json { line, .. } | MissingType { line } | MissingId { line } => *line,
        LineTooLong(line, _) => *line,
        #[allow(unreachable_patterns)]
        _ => 0,
    }
}

/// Report lines that could not be read; returns their number.
fn report_read_errors(errors: &[pmef_io::PmefIoError]) -> usize {
    for e in errors {
        eprintln!("✗ {e}");
    }
    errors.len()
}

async fn cmd_validate(
    file: PathBuf, schemas: PathBuf, level: u8, format: &str, fail_fast: bool, strict: bool,
) -> Result<()> {
    use pmef_io::{NdjsonReader, ReaderConfig};
    use pmef_validate::PmefValidator;
    use std::io::BufReader;
    use std::fs::File;

    info!("Validating {} (conformance level {})", file.display(), level);

    let validator = PmefValidator::from_directory(&schemas)
        .map_err(|e| anyhow::anyhow!("Failed to load schemas from {}: {}", schemas.display(), e))?;

    let f = File::open(&file).with_context(|| format!("Cannot open {}", file.display()))?;
    let mut reader = NdjsonReader::new(BufReader::new(f), ReaderConfig::default());

    let mut ok = 0usize;
    let mut fail = 0usize;
    let mut skip = 0usize;
    let mut all_errors: Vec<String> = Vec::new();

    for result in reader.by_ref() {
        match result {
            Ok(obj) => {
                if pmef_validate::schema_for_type(&obj.type_str).is_none() {
                    if strict {
                        fail += 1;
                        let msg = format!("line {}: {} ({}): unknown @type",
                            obj.line_number, obj.id_str, obj.type_str);
                        error!("{}", msg);
                        all_errors.push(msg);
                        if fail_fast { break; }
                    } else {
                        // spec 06 CL1-03: skip + warn, do not fail
                        skip += 1;
                        warn!("No schema for type '{}' on line {}", obj.type_str, obj.line_number);
                    }
                    continue;
                }

                let errors = validator.validate_object(&obj.type_str, &obj.value);
                if errors.is_empty() {
                    ok += 1;
                } else {
                    fail += 1;
                    for e in &errors {
                        let msg = format!(
                            "line {}: {} ({}): {}",
                            obj.line_number, obj.id_str, obj.type_str, e
                        );
                        error!("{}", msg);
                        all_errors.push(msg);
                    }
                    if fail_fast {
                        break;
                    }
                }
            }
            Err(e) => {
                fail += 1;
                let msg = format!("Parse error: {e}");
                error!("{}", msg);
                all_errors.push(msg);
                if fail_fast {
                    break;
                }
            }
        }
    }

    // Lines the reader could not turn into objects (invalid JSON, missing @type/@id) are
    // errors, never dropped silently (AUDIT P3.8).
    for e in reader.errors() {
        fail += 1;
        let n = read_error_line(e);
        let text = e.to_string();
        let text = text.strip_prefix(&format!("Line {n}: ")).unwrap_or(&text);
        let msg = format!("line {n}: {text}");
        error!("{}", msg);
        all_errors.push(msg);
    }

    let total = ok + fail + skip;
    // Pass rate is over *validated* objects (ok + fail); objects skipped because
    // the tool has no schema for their @type are a validator gap, not a data
    // defect, and must not drag conformance below threshold.
    let validated = ok + fail;
    let pass_rate = if validated > 0 { ok as f64 / validated as f64 } else { 1.0 };
    match format {
        "json" => {
            let report = serde_json::json!({
                "total": total,
                "ok": ok,
                "fail": fail,
                "skip": skip,
                "pass_rate": pass_rate,
                "errors": all_errors,
            });
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        _ => {
            println!("PMEF Validation — {}", file.display());
            println!("────────────────────────────────────");
            println!("  Total:    {total}");
            println!("  ✓ Valid:  {ok}");
            println!("  ✗ Errors: {fail}");
            println!("  ○ Skipped:{skip}");
            println!("  Pass rate: {:.1}%", pass_rate * 100.0);
            let meets = pass_rate >= match level { 3 => 0.98, 2 => 0.95, _ => 1.0 };
            println!("  Conformance L{level}: {}", if meets { "✓ PASS" } else { "✗ FAIL" });
            if !all_errors.is_empty() {
                println!("\n  Errors:");
                for e in &all_errors {
                    println!("    - {e}");
                }
            }
        }
    }
    if fail > 0 { std::process::exit(1); }
    Ok(())
}

fn read_values(path: &PathBuf) -> Result<Vec<serde_json::Value>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("Cannot open {}", path.display()))?;
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with("//"))
        .map(|(n, l)| {
            serde_json::from_str(l).with_context(|| format!("{}:{}: invalid JSON", path.display(), n + 1))
        })
        .collect()
}

fn cmd_merge(
    base: PathBuf,
    ours: PathBuf,
    theirs: PathBuf,
    output: PathBuf,
    prefer: &str,
    exact: bool,
    format: &str,
) -> Result<()> {
    use pmef_core::merge::{merge, MergeOptions, Prefer};
    let prefer = if prefer == "theirs" { Prefer::Theirs } else { Prefer::Ours };
    let run = || -> Result<pmef_core::merge::MergeResult> {
        let (b, o, t) = (read_values(&base)?, read_values(&ours)?, read_values(&theirs)?);
        let r = merge(&b, &o, &t, &MergeOptions { exact, prefer })?;
        let mut text = String::new();
        for v in &r.objects {
            text.push_str(&serde_json::to_string(v)?);
            text.push('\n');
        }
        std::fs::write(&output, text).with_context(|| format!("Cannot write {}", output.display()))?;
        Ok(r)
    };
    let r = match run() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(2);
        }
    };
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "objects": r.objects.len(), "conflicts": r.conflicts
        }))?);
    } else {
        for c in &r.conflicts {
            let v = |x: &Option<serde_json::Value>| x.as_ref().map_or_else(|| "(absent)".to_owned(), |v| v.to_string());
            println!("! {} {} [{}]: base {} | ours {} | theirs {}",
                c.id, if c.path.is_empty() { "(object)" } else { &c.path },
                serde_json::to_value(c.kind)?.as_str().unwrap_or(""), v(&c.base), v(&c.ours), v(&c.theirs));
        }
        println!("Merged {} objects into {}, {} conflict(s)", r.objects.len(), output.display(), r.conflicts.len());
    }
    if !r.conflicts.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_diff(
    base: PathBuf,
    head: PathBuf,
    format: &str,
    exact: bool,
    ignore: Vec<String>,
    matching: Option<pmef_core::matching::MatchOptions>,
    emit_matches: Option<PathBuf>,
) -> Result<()> {
    use pmef_core::diff::{diff, diff_matched, render_text, DiffOptions};

    let run = || -> Result<pmef_core::diff::DiffReport> {
        let (b, h) = (read_values(&base)?, read_values(&head)?);
        let opts = DiffOptions { exact, ignore };
        Ok(match &matching {
            Some(m) => diff_matched(&b, &h, &opts, m)?,
            None => diff(&b, &h, &opts)?,
        })
    };
    let report = match run() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(2);
        }
    };
    if let Some(path) = &emit_matches {
        use pmef_core::matching::{proposals, revision_id_today};
        let lines: Vec<String> = proposals(&report.matches, &revision_id_today())
            .iter()
            .map(|v| serde_json::to_string(&pmef_core::diff::canonical(v)))
            .collect::<Result<_, _>>()?;
        let text = if lines.is_empty() { String::new() } else { lines.join("\n") + "\n" };
        if let Err(e) = std::fs::write(path, text) {
            eprintln!("error: cannot write {}: {e}", path.display());
            std::process::exit(2);
        }
    }
    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", render_text(&report));
    }
    if !report.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

async fn cmd_stats(file: PathBuf, by_type: bool, by_domain: bool) -> Result<()> {
    use pmef_io::{NdjsonReader, ReaderConfig};
    use std::io::BufReader;
    use std::fs::File;
    use std::collections::HashMap;

    let f = File::open(&file).with_context(|| format!("Cannot open {}", file.display()))?;
    let config = ReaderConfig { fail_fast: true, ..ReaderConfig::default() };
    let reader = NdjsonReader::new(BufReader::new(f), config);

    let mut type_counts: HashMap<String, usize> = HashMap::new();
    let mut total = 0usize;

    for result in reader {
        let obj = result?;
        *type_counts.entry(obj.type_str.clone()).or_insert(0) += 1;
        total += 1;
    }

    println!("PMEF Package Statistics — {}", file.display());
    println!("────────────────────────────────────");
    println!("  Total objects: {total}");

    if by_type || by_domain {
        let mut pairs: Vec<_> = type_counts.iter().collect();
        pairs.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        println!("\n  By @type:");
        for (type_str, count) in &pairs {
            println!("  {:40} {:>6}", type_str, count);
        }
    }
    Ok(())
}

fn cmd_convert(input: PathBuf, to: &str, base: Option<PathBuf>, output: PathBuf) -> Result<()> {
    use pmef_adapter_plant3d::pcf_bridge::{apply_pcf, write_pcf};
    let run = || -> Result<()> {
        if to == "pcf" {
            let objs = read_values(&input)?;
            let export = write_pcf(&objs);
            std::fs::create_dir_all(&output).with_context(|| format!("Cannot create {}", output.display()))?;
            for (line, text) in &export.files {
                let name: String = line.chars().map(|c| if c.is_alphanumeric() || matches!(c, '-' | '_') { c } else { '_' }).collect();
                let path = output.join(format!("{name}.pcf"));
                std::fs::write(&path, text).with_context(|| format!("Cannot write {}", path.display()))?;
            }
            for w in &export.warnings {
                eprintln!("warning: {w}");
            }
            eprintln!("wrote {} PCF file(s) to {}", export.files.len(), output.display());
        } else {
            let base = base.as_ref().context("--base is required with --to pmef")?;
            let reference = read_values(base)?;
            let mut files: Vec<PathBuf> = if input.is_dir() {
                std::fs::read_dir(&input)?.filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pcf"))).collect()
            } else {
                vec![input.clone()]
            };
            files.sort();
            let pcfs: Vec<String> = files.iter()
                .map(|p| std::fs::read_to_string(p).with_context(|| format!("Cannot open {}", p.display())))
                .collect::<Result<_>>()?;
            let (objs, stats) = apply_pcf(&reference, &pcfs);
            let mut text = String::new();
            for o in &objs {
                text.push_str(&serde_json::to_string(o)?);
                text.push('\n');
            }
            std::fs::write(&output, text).with_context(|| format!("Cannot write {}", output.display()))?;
            for l in &stats.unknown_lines {
                eprintln!("warning: PIPELINE-REFERENCE {l} matches no line in the base model (skipped)");
            }
            eprintln!("updated {}, created {}, deleted {} component(s) from {} PCF file(s)",
                      stats.updated, stats.created, stats.deleted, pcfs.len());
        }
        Ok(())
    };
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(2);
    }
    Ok(())
}

async fn cmd_index(
    file: PathBuf, key: &str, filter_type: Option<String>, output: Option<PathBuf>,
) -> Result<()> {
    use pmef_io::{NdjsonReader, ReaderConfig};
    use std::io::BufReader;
    use std::fs::File;
    use std::collections::HashMap;

    let f = File::open(&file).with_context(|| format!("Cannot open {}", file.display()))?;
    let config = ReaderConfig { fail_fast: true, ..ReaderConfig::default() };
    let reader = NdjsonReader::new(BufReader::new(f), config);

    let mut index: HashMap<String, serde_json::Value> = HashMap::new();

    for result in reader {
        let obj = result?;
        if let Some(ref ft) = filter_type {
            if &obj.type_str != ft { continue; }
        }
        let k = match key {
            "id" | "@id" => obj.id_str.clone(),
            "tag-number" | "tagNumber" => obj.value
                .get("equipmentBasic").or_else(|| obj.value.get("tagNumber"))
                .and_then(|v| v.get("tagNumber").or(Some(v)))
                .and_then(|v| v.as_str())
                .unwrap_or(&obj.id_str)
                .to_owned(),
            "line-number" | "lineNumber" => obj.value
                .get("lineNumber")
                .and_then(|v| v.as_str())
                .unwrap_or(&obj.id_str)
                .to_owned(),
            _ => obj.id_str.clone(),
        };
        index.insert(k, obj.value);
    }

    let out = serde_json::to_string_pretty(&index)?;
    match output {
        Some(path) => std::fs::write(&path, &out)
            .with_context(|| format!("Cannot write {}", path.display()))?,
        None => println!("{out}"),
    }
    Ok(())
}

/// Structural package check (`pmef check`): data, not implementation conformance.
fn cmd_route(problem: PathBuf, output: PathBuf, report: Option<PathBuf>, plant: Option<String>) -> Result<()> {
    let text = std::fs::read_to_string(&problem)?;
    let prob: pmef_route::RouteProblem = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("{}: {e}", problem.display()))?;
    let base = problem.parent().map(PathBuf::from).unwrap_or_default();
    let run = pmef_route::run(prob, plant.as_deref(), &pmef_route::scene::fs_reader(&base))?;
    std::fs::write(&output, run.ndjson())?;
    for r in &run.routes {
        match &r.error {
            None => println!(
                "{}: DN{} {}  {:.0} mm, {} bends, {} penetrations, {} states, {} ms",
                r.id, r.dn, r.pipe_class, r.length_mm, r.bends, r.penetrations, r.expansions, r.millis
            ),
            Some(e) => println!("{}: FAILED: {e}", r.id),
        }
        for b in &r.branches {
            match &b.error {
                None => println!("  branch {}: tee at {:?}, {:.0} mm, {} bends", b.id, b.points[0], b.length_mm, b.bends),
                Some(e) => println!("  branch {}: FAILED: {e}", b.id),
            }
        }
    }
    for s in &run.skipped_lines {
        println!("line {}: not routed: {}", s.line_number, s.reason);
    }
    for c in &run.cables {
        match &c.error {
            None => println!(
                "cable {}: {:.0} mm ({:.0} mm drops) on {}",
                c.id, c.length_mm, c.drop_mm, c.trays.join(" > ")
            ),
            Some(e) => println!("cable {}: FAILED: {e}", c.id),
        }
    }
    for t in &run.tray_fill {
        println!("tray {}: {:.1} % fill, {} cable(s)", t.id, t.fill, t.cables.len());
    }
    if let Some(rep) = report {
        std::fs::write(rep, serde_json::to_string_pretty(&run.report())?)?;
    }
    if !run.ok() {
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_check(file: PathBuf, json_output: bool, output: Option<PathBuf>) -> Result<()> {
    use pmef_io::{NdjsonReader, PmefIoError, ReaderConfig};
    use std::collections::{HashMap, HashSet};
    use std::fs::File;
    use std::io::BufReader;

    let f = File::open(&file).with_context(|| format!("Cannot open {}", file.display()))?;
    let mut reader = NdjsonReader::new(BufReader::new(f), ReaderConfig::default());
    let objects: Vec<_> = reader.by_ref().collect::<Result<_, _>>()?;
    let read_errors = reader.errors();
    let count = |pred: fn(&PmefIoError) -> bool| read_errors.iter().filter(|e| pred(e)).count();
    let missing_type = count(|e| matches!(e, PmefIoError::MissingType { .. }));
    let missing_id = count(|e| matches!(e, PmefIoError::MissingId { .. }));
    let unparsable = read_errors.len() - missing_type - missing_id;

    let mut checks: Vec<(&str, bool, String)> = Vec::new();

    let headers: Vec<_> = objects.iter().filter(|o| o.type_str == "pmef:FileHeader").collect();
    checks.push(("fileHeaderPresent", headers.len() == 1, format!("{} FileHeader object(s)", headers.len())));
    let first = objects.first().map(|o| o.type_str.as_str()).unwrap_or("<empty file>");
    checks.push(("fileHeaderFirst", first == "pmef:FileHeader", format!("first object: {first}")));
    checks.push(("allLinesParse", unparsable == 0, format!("{unparsable} line(s) are not valid JSON")));
    checks.push(("allObjectsHaveType", missing_type == 0, format!("{missing_type} object(s) without a string @type")));
    checks.push(("allObjectsHaveId", missing_id == 0, format!("{missing_id} object(s) without a string @id")));

    let mut seen: HashMap<&str, usize> = HashMap::new();
    for o in &objects {
        *seen.entry(o.id_str.as_str()).or_insert(0) += 1;
    }
    let mut dups: Vec<&str> = seen.iter().filter(|(_, n)| **n > 1).map(|(id, _)| *id).collect();
    dups.sort_unstable();
    checks.push(("idsUnique", dups.is_empty(), if dups.is_empty() { "all @id unique".into() } else {
        format!("{} duplicate @id: {}", dups.len(), dups.iter().take(10).copied().collect::<Vec<_>>().join(", "))
    }));

    // connectedTo must resolve inside the package (spec 02 §2.6) unless it carries the
    // urn:pmef:external: prefix; isPartOf / isDerivedFrom / sourceId / targetId may point to
    // other layers (unit, functional model) outside the file and are reported, not failed —
    // the same rules as scripts/check-topology.py.
    let known: HashSet<&str> = seen.keys().copied().collect();
    let resolves = |t: &str| {
        let base = t.split('#').next().unwrap_or(t);
        base.is_empty() || base.starts_with("urn:pmef:external:") || known.contains(base)
    };
    let mut broken: Vec<String> = Vec::new();
    let mut outside: Vec<String> = Vec::new();
    for o in &objects {
        let mut conns: Vec<&str> = o.value.get("connectedTo").and_then(|v| v.as_str()).into_iter().collect();
        for p in o.value.get("ports").and_then(|v| v.as_array()).into_iter().flatten() {
            conns.extend(p.get("connectedTo").and_then(|v| v.as_str()));
        }
        for t in conns.into_iter().filter(|t| !resolves(t)) {
            broken.push(format!("{} -> {t}", o.id_str));
        }
        for field in ["isPartOf", "isDerivedFrom", "sourceId", "targetId"] {
            if let Some(t) = o.value.get(field).and_then(|v| v.as_str()).filter(|t| !resolves(t)) {
                outside.push(format!("{}.{field} -> {t}", o.id_str));
            }
        }
    }
    let list = |v: &[String]| v.iter().take(10).cloned().collect::<Vec<_>>().join(", ");
    checks.push(("connectionsResolve", broken.is_empty(), if broken.is_empty() {
        "all connectedTo targets resolve".into() } else { format!("{} unresolved connectedTo: {}", broken.len(), list(&broken)) }));
    let info = if outside.is_empty() { "none".to_owned() } else { format!("{}: {}", outside.len(), list(&outside)) };

    // mesh_ref targets (spec 04 §2.3): resolve the fragment when the glTF file is in the package.
    let (mesh_failed, mesh_notes) = check_mesh_refs(&file, &objects);
    checks.push(("meshRefsResolve", mesh_failed.is_empty(), if mesh_failed.is_empty() {
        "all mesh_ref fragments resolve (or their file is not in the package)".into()
    } else { format!("{} unresolved mesh_ref: {}", mesh_failed.len(), list(&mesh_failed)) }));

    let failed = checks.iter().filter(|c| !c.1).count();
    let version = headers.first().and_then(|h| h.value.get("pmefVersion")).and_then(|v| v.as_str()).unwrap_or("?");
    let mut by_type: Vec<(String, usize)> = objects.iter().fold(HashMap::new(), |mut m, o| {
        *m.entry(o.type_str.clone()).or_insert(0) += 1;
        m
    }).into_iter().collect();
    by_type.sort();

    let out = if json_output {
        serde_json::to_string_pretty(&serde_json::json!({
            "file": file.display().to_string(),
            "pmefVersion": version,
            "objectCount": objects.len(),
            "summary": { "totalChecks": checks.len(), "pass": checks.len() - failed, "fail": failed,
                         "status": if failed == 0 { "PASS" } else { "FAIL" } },
            "checks": checks.iter().map(|(n, ok, d)| serde_json::json!({
                "check": n, "status": if *ok { "PASS" } else { "FAIL" }, "detail": d })).collect::<Vec<_>>(),
            "referencesOutsidePackage": outside,
            "meshRefNotes": mesh_notes,
            "typeCounts": by_type.iter().map(|(t, n)| (t.clone(), serde_json::json!(n))).collect::<serde_json::Map<String, serde_json::Value>>(),
        }))?
    } else {
        let mut t = format!("PMEF package check — {} (pmefVersion {version}, {} objects)\n", file.display(), objects.len());
        for (n, ok, d) in &checks {
            t += &format!("  {} {n}: {d}\n", if *ok { "✓" } else { "✗" });
        }
        t += &format!("  i references outside the package (unit, functional model; not checked): {info}\n");
        if !mesh_notes.is_empty() {
            t += &format!("  i mesh_ref notes: {}: {}\n", mesh_notes.len(), list(&mesh_notes));
        }
        t += &format!("{} — {} of {} checks passed\n", if failed == 0 { "PASS" } else { "FAIL" }, checks.len() - failed, checks.len());
        t
    };
    match output {
        Some(path) => std::fs::write(&path, &out).with_context(|| format!("Cannot write {}", path.display()))?,
        None => print!("{out}{}", if json_output { "\n" } else { "" }),
    }
    if failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// Resolve `geometry.ref` of type `mesh_ref` and `MESH_REF.meshUri` (inside ParametricGeometry,
/// also in COMPOSITE children) against glTF files next to the package. Returns (failures, notes):
/// notes list stale indices and files that are not in the package.
fn check_mesh_refs(file: &std::path::Path, objects: &[pmef_io::RawPmefObject]) -> (Vec<String>, Vec<String>) {
    use pmef_io::mesh_ref;
    // (reference, declared mesh format); only glTF files are parsed and resolved.
    fn uris<'a>(prim: &'a serde_json::Value, out: &mut Vec<(&'a str, Option<&'a str>)>) {
        if prim.get("type").and_then(|v| v.as_str()) == Some("MESH_REF") {
            let format = prim.get("meshFormat").and_then(|v| v.as_str());
            out.extend(prim.get("meshUri").and_then(|v| v.as_str()).map(|u| (u, format)));
        }
        for c in prim.get("children").and_then(|v| v.as_array()).into_iter().flatten() {
            if let Some(p) = c.get("primitive") {
                uris(p, out);
            }
        }
    }
    let dir = file.parent().unwrap_or(std::path::Path::new("."));
    let mut docs: std::collections::HashMap<String, Result<serde_json::Value, String>> = Default::default();
    let (mut failed, mut notes) = (Vec::new(), Vec::new());
    for o in objects {
        let mut refs = Vec::new();
        let g = o.value.get("geometry");
        if g.and_then(|g| g.get("type")).and_then(|v| v.as_str()) == Some("mesh_ref") {
            refs.extend(g.and_then(|g| g.get("ref")).and_then(|v| v.as_str()).map(|r| (r, None)));
        }
        if let Some(p) = o.value.get("primitive") {
            uris(p, &mut refs);
        }
        for (r, format) in refs {
            let parsed = match mesh_ref::parse(r) {
                Ok(p) => p,
                Err(e) => { failed.push(format!("{} -> {r}: {e}", o.id_str)); continue; }
            };
            // Only package-relative paths are checked; URIs with a scheme point elsewhere.
            let rel = parsed.path.strip_prefix("file:").unwrap_or(&parsed.path);
            if rel.contains("://") || rel.starts_with('/') || rel.contains(':') {
                continue;
            }
            let rel = match mesh_ref::decode_path(rel) {
                Ok(p) => p,
                Err(e) => { failed.push(format!("{} -> {r}: {e}", o.id_str)); continue; }
            };
            let rel = rel.as_str();
            // OBJ, STL, USD, STEP ... are not glTF: neither parsed nor resolved.
            let gltf = match format {
                Some(f) => f.eq_ignore_ascii_case("GLTF") || f.eq_ignore_ascii_case("GLB"),
                None => mesh_ref::is_gltf_path(rel),
            };
            if !gltf {
                continue;
            }
            let path = dir.join(rel);
            if !path.is_file() {
                notes.push(format!("{} -> {rel}: file not in the package", o.id_str));
                continue;
            }
            let doc = docs.entry(rel.to_owned()).or_insert_with(|| {
                std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| mesh_ref::gltf_json(&b))
            });
            match doc.as_ref().map_err(Clone::clone).and_then(|d| mesh_ref::resolve(d, &parsed)) {
                Ok(res) => notes.extend(res.warnings.into_iter().map(|w| format!("{} -> {r}: {w}", o.id_str))),
                Err(e) => failed.push(format!("{} -> {r}: {e}", o.id_str)),
            }
        }
    }
    (failed, notes)
}

async fn cmd_check_refs(file: PathBuf, strict: bool) -> Result<()> {
    use pmef_io::{NdjsonReader, PmefPackageIndex, ReaderConfig};
    use std::io::BufReader;
    use std::fs::File;

    let f = File::open(&file).with_context(|| format!("Cannot open {}", file.display()))?;
    let config = ReaderConfig { fail_fast: true, ..ReaderConfig::default() };
    let reader = NdjsonReader::new(BufReader::new(f), config);
    let objects: Vec<_> = reader.collect::<Result<_, _>>()?;
    let idx = PmefPackageIndex::from_iter(objects.into_iter());

    let mut broken = 0usize;
    let ref_fields = ["isPartOf", "isDerivedFrom", "sourceId", "targetId"];

    for obj in idx.objects.values() {
        for field in ref_fields {
            if let Some(ref_val) = obj.value.get(field) {
                if let Some(ref_str) = ref_val.as_str() {
                    if idx.resolve(ref_str).is_none() {
                        broken += 1;
                        let msg = format!(
                            "{}: {}.{} → '{}' (not found)",
                            obj.id_str, obj.type_str, field, ref_str
                        );
                        if strict { error!("{msg}"); } else { warn!("{msg}"); }
                    }
                }
            }
        }
    }

    println!("Reference check: {} broken reference(s) found in {} objects",
             broken, idx.len());
    if strict && broken > 0 { std::process::exit(1); }
    Ok(())
}

async fn cmd_topology(file: PathBuf, allow_external: bool) -> Result<()> {
    use pmef_io::read_ndjson_file;
    use pmef_validate::check_topology;

    let objects: Vec<serde_json::Value> = read_ndjson_file(&file)
        .with_context(|| format!("Cannot read {}", file.display()))?
        .into_iter()
        .map(|o| o.value)
        .collect();

    let report = check_topology(&objects, allow_external);

    println!("PMEF Topology — {}", file.display());
    println!("────────────────────────────────────");
    println!("  Objects:     {}", report.object_count);
    println!("  Connections: {}", report.connection_count);
    println!("  ✗ Errors:    {}", report.error_count());
    println!("  ! Warnings:  {}", report.warning_count());
    for issue in &report.issues {
        let tag = if issue.is_error { "ERROR" } else { "warn " };
        println!("    {tag}: {}", issue.message);
    }
    if report.is_ok() {
        println!("\n  ✓ Topology consistent.");
    }

    if !report.is_ok() { std::process::exit(1); }
    Ok(())
}


// ── PMEFX (spec 03 §3, RFC-0005) ──────────────────────────────────────────────

fn file_name(p: &std::path::Path) -> Result<String> {
    Ok(p.file_name().and_then(|n| n.to_str()).context("file name is not UTF-8")?.to_string())
}

#[allow(clippy::too_many_arguments)]
fn cmd_pack(
    files: Vec<PathBuf>, output: PathBuf, bbox_lods: bool, lods: Vec<String>, breps: Vec<String>,
    extra: Vec<String>, mesh_origin: &str, created_at: Option<String>, no_octree: bool,
) -> Result<()> {
    use pmef_io::pmefx::{FileKind, FinishOptions, Lod, LodStats, PmefxWriter, Point3};
    let out = std::fs::File::create(&output).with_context(|| format!("creating {}", output.display()))?;
    let mut w = PmefxWriter::new(std::io::BufWriter::new(out));
    for f in &files {
        let bytes = std::fs::read(f).with_context(|| format!("reading {}", f.display()))?;
        let n = w.add_ndjson(&format!("model/{}", file_name(f)?), &bytes)?;
        info!("{}: {} objects", f.display(), n);
    }
    let origin = match mesh_origin {
        "none" => None,
        "auto" => w.extent().map(|b| {
            let c = b.center();
            Point3 { x: (c.x / 1000.0).round() * 1000.0, y: (c.y / 1000.0).round() * 1000.0, z: (c.z / 1000.0).round() * 1000.0 }
        }),
        xyz => {
            let v: Vec<f64> = xyz.split(',').map(|t| t.trim().parse::<f64>()).collect::<Result<_, _>>()
                .context("--mesh-origin expects auto, none or x,y,z")?;
            anyhow::ensure!(v.len() == 3, "--mesh-origin expects x,y,z");
            Some(Point3 { x: v[0], y: v[1], z: v[2] })
        }
    };
    if let Some(o) = origin {
        w.set_mesh_origin(o)?;
    }
    for spec in &lods {
        let mut it = spec.splitn(3, '=');
        let (Some(id), Some(level), Some(path)) = (it.next(), it.next(), it.next()) else {
            anyhow::bail!("--lod expects ID=LEVEL=FILE, got {spec}");
        };
        let lod: Lod = serde_json::from_value(serde_json::Value::String(level.to_string()))
            .with_context(|| format!("unknown LOD level {level}"))?;
        let p = PathBuf::from(path);
        let bytes = std::fs::read(&p).with_context(|| format!("reading {path}"))?;
        let stem = pmef_io::pmefx::file_stem_for_id(id);
        w.add_lod(id, lod, &format!("meshes/{stem}.{}.glb", level.to_ascii_lowercase()), &bytes, LodStats::default())?;
    }
    for spec in &breps {
        let (id, path) = spec.split_once('=').with_context(|| format!("--brep expects ID=FILE, got {spec}"))?;
        let bytes = std::fs::read(path).with_context(|| format!("reading {path}"))?;
        let stem = pmef_io::pmefx::file_stem_for_id(id);
        w.set_brep(id, &format!("brep/{stem}.stp"), "step", &bytes)?;
    }
    for spec in &extra {
        let (kind, path) = spec.split_once('=').with_context(|| format!("--file expects TYPE=FILE, got {spec}"))?;
        let kind: FileKind = serde_json::from_value(serde_json::Value::String(kind.to_string()))
            .with_context(|| format!("unknown file type {kind}"))?;
        let dir = match kind {
            FileKind::Catalog => "catalogs",
            FileKind::Document => "documents",
            FileKind::Gltf | FileKind::Step | FileKind::Usd => "geometry",
            _ => "other",
        };
        let p = PathBuf::from(path);
        w.add_file(&format!("{dir}/{}", file_name(&p)?), kind, &std::fs::read(&p).with_context(|| format!("reading {path}"))?)?;
    }
    let generated = if bbox_lods { w.generate_bbox_lods()? } else { 0 };
    let objects = w.objects().count();
    let defaults = FinishOptions::default();
    w.finish(FinishOptions {
        created_at: created_at.unwrap_or_else(chrono_now),
        octree: if no_octree { None } else { defaults.octree },
        ..defaults
    })?;
    println!("{}: {} objects, {} bbox meshes generated", output.display(), objects, generated);
    Ok(())
}

/// Current UTC time as ISO 8601 (seconds precision) without a date crate.
fn chrono_now() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil-from-days (H. Hinnant).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn cmd_unpack(file: PathBuf, output: PathBuf) -> Result<()> {
    let f = std::fs::File::open(&file).with_context(|| format!("opening {}", file.display()))?;
    let mut r = pmef_io::pmefx::PmefxReader::open(std::io::BufReader::new(f))?;
    let n = r.extract_to(&output)?;
    println!("{} entries extracted to {}", n, output.display());
    Ok(())
}

fn cmd_verify(file: PathBuf, json: bool) -> Result<()> {
    let f = std::fs::File::open(&file).with_context(|| format!("opening {}", file.display()))?;
    let mut r = match pmef_io::pmefx::PmefxReader::open(std::io::BufReader::new(f)) {
        Ok(r) => r,
        Err(e) => {
            error!("{e}");
            std::process::exit(2);
        }
    };
    let issues = r.verify();
    let m = r.manifest().clone();
    let index = r.object_index().unwrap_or_default();
    let lods: usize = index.iter().map(|o| o.lods.len()).sum();
    let breps = index.iter().filter(|o| o.brep.is_some()).count();
    if json {
        let report = serde_json::json!({
            "file": file.display().to_string(),
            "pmefxVersion": m.pmefx_version,
            "packageId": m.package_id,
            "files": m.files.len(),
            "objects": index.len(),
            "lods": lods,
            "breps": breps,
            "spatialIndex": m.spatial_index.is_some(),
            "conformant": issues.is_empty(),
            "issues": issues,
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}  pmefx {}  {}", file.display(), m.pmefx_version.as_deref().unwrap_or("0.9 (legacy)"), m.package_id);
        println!("  files {}  objects {}  mesh LODs {}  B-reps {}  octree {}", m.files.len(), index.len(), lods, breps,
            if m.spatial_index.is_some() { "yes" } else { "no" });
        for i in &issues {
            println!("  ✗ {i}");
        }
        println!("{}", if issues.is_empty() { "conformant" } else { "NOT conformant" });
    }
    if !issues.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}
