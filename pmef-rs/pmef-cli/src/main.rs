//! # pmef — PMEF Command Line Tool
//!
//! Usage:
//!   pmef validate <file.ndjson> [--schemas <dir>] [--level 1|2|3]
//!   pmef diff <base.ndjson> <head.ndjson> [--format text|json]
//!   pmef stats <file.ndjson>
//!   pmef convert <input.pcf> --to pmef --output <output.ndjson>
//!   pmef index <file.ndjson> [--key tag-number|line-number]
//!   pmef conformance <file.ndjson> [--dataset ds01]
//!   pmef topology <file.ndjson> [--allow-external]

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

    /// Run the PMEF conformance test suite against a package.
    Conformance {
        /// PMEF package to test.
        file: PathBuf,
        /// Benchmark dataset (ds01, ds02, ds03).
        #[arg(long, default_value = "ds01")]
        dataset: String,
        /// Conformance level to test.
        #[arg(long, default_value_t = 2)]
        level: u8,
        /// Output report as JSON.
        #[arg(long)]
        json: bool,
        /// Output report file (default: stdout).
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
        Commands::Conformance { file, dataset, level, json, output } => {
            cmd_conformance(file, &dataset, level, json, output).await
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

async fn cmd_conformance(
    file: PathBuf, dataset: &str, level: u8, json_output: bool,
    output: Option<PathBuf>,
) -> Result<()> {
    use pmef_io::{NdjsonReader, PmefPackageIndex, ReaderConfig};
    use std::io::BufReader;
    use std::fs::File;
    use std::collections::HashSet;

    let f = File::open(&file).with_context(|| format!("Cannot open {}", file.display()))?;
    let mut reader = NdjsonReader::new(BufReader::new(f), ReaderConfig::default());
    let objects: Vec<_> = reader.by_ref().collect::<Result<_, _>>()?;
    // Lines the reader rejected (no string @type / @id, invalid JSON) feed checks 3–4 and 4b.
    let read_errors = reader.errors();
    let missing_type_lines = read_errors.iter()
        .filter(|e| matches!(e, pmef_io::PmefIoError::MissingType { .. })).count();
    let missing_id_lines = read_errors.iter()
        .filter(|e| matches!(e, pmef_io::PmefIoError::MissingId { .. })).count();
    let unparsable_lines = read_errors.len() - missing_type_lines - missing_id_lines;
    report_read_errors(read_errors);

    // Track ordering: FileHeader must be first object
    let first_type_owned = objects.first().map(|o| o.type_str.clone());
    let first_type = first_type_owned.as_deref();

    let idx = PmefPackageIndex::from_iter(objects.into_iter());

    let mut pass_count = 0usize;
    let mut fail_count = 0usize;
    let mut check_results: Vec<serde_json::Value> = Vec::new();

    // Helper to record a check result
    let mut record = |name: &str, passed: bool, detail: &str| {
        if passed { pass_count += 1; } else { fail_count += 1; }
        check_results.push(serde_json::json!({
            "check": name,
            "status": if passed { "PASS" } else { "FAIL" },
            "detail": detail,
        }));
    };

    // Check 1: FileHeader present
    let headers = idx.by_type("pmef:FileHeader");
    let has_header = headers.len() == 1;
    record(
        "fileHeaderPresent",
        has_header,
        &format!("Found {} FileHeader object(s)", headers.len()),
    );

    // Check 2: FileHeader is first object
    let header_first = first_type == Some("pmef:FileHeader");
    record(
        "fileHeaderFirst",
        header_first,
        &format!(
            "First object @type: {}",
            first_type.unwrap_or("<empty file>")
        ),
    );

    // Check 3: All objects have @type
    let missing_type_count = missing_type_lines;
    record(
        "allObjectsHaveType",
        missing_type_count == 0,
        &format!("{} object(s) missing @type", missing_type_count),
    );

    // Check 4: All objects have @id
    let missing_id_count = missing_id_lines;
    record(
        "allLinesParse",
        unparsable_lines == 0,
        &format!("{} line(s) are not valid JSON", unparsable_lines),
    );
    record(
        "allObjectsHaveId",
        missing_id_count == 0,
        &format!("{} object(s) missing @id", missing_id_count),
    );

    // Check 5: All @id references resolve
    let ref_fields = ["isPartOf", "isDerivedFrom", "sourceId", "targetId",
                      "connectedTo", "flowsTo", "references"];
    let known_ids: HashSet<&str> = idx.objects.keys().map(|s| s.as_str()).collect();
    let mut broken_refs = 0usize;
    let mut broken_ref_details: Vec<String> = Vec::new();
    for obj in idx.objects.values() {
        for field in &ref_fields {
            if let Some(ref_val) = obj.value.get(*field) {
                let refs: Vec<&str> = if let Some(s) = ref_val.as_str() {
                    vec![s]
                } else if let Some(arr) = ref_val.as_array() {
                    arr.iter().filter_map(|v| v.as_str()).collect()
                } else {
                    vec![]
                };
                for ref_str in refs {
                    let base_id = ref_str.split('#').next().unwrap_or(ref_str);
                    if !base_id.is_empty() && !known_ids.contains(base_id) {
                        broken_refs += 1;
                        if broken_ref_details.len() < 10 {
                            broken_ref_details.push(format!(
                                "{}.{} -> {}", obj.id_str, field, ref_str
                            ));
                        }
                    }
                }
            }
        }
    }
    record(
        "allReferencesResolve",
        broken_refs == 0,
        &format!("{} broken reference(s){}", broken_refs,
            if broken_ref_details.is_empty() { String::new() }
            else { format!(": {}", broken_ref_details.join(", ")) }),
    );

    // Check 6: All @id values are unique (guaranteed by HashMap, but check for duplicates
    // that may have been silently overwritten)
    // Since PmefPackageIndex deduplicates by @id via HashMap, we re-count from file
    // We already loaded into idx, so just note the object count
    let pump_count = idx.by_type("pmef:Pump").len();
    let vessel_count = idx.by_type("pmef:Vessel").len();
    let line_count = idx.by_type("pmef:PipingNetworkSystem").len();

    let total_checks = pass_count + fail_count;
    let overall_status = if fail_count == 0 { "PASS" } else { "FAIL" };

    let report = serde_json::json!({
        "pmefVersion": "0.9.0",
        "file": file.display().to_string(),
        "dataset": dataset,
        "conformanceLevel": level,
        "objectCount": idx.len(),
        "summary": {
            "totalChecks": total_checks,
            "pass": pass_count,
            "fail": fail_count,
            "status": overall_status,
        },
        "checks": check_results,
        "typeCounts": {
            "pumps": pump_count,
            "vessels": vessel_count,
            "pipingLines": line_count,
        },
    });

    let out = serde_json::to_string_pretty(&report)?;
    match output {
        Some(path) => std::fs::write(&path, &out)?,
        None => println!("{out}"),
    }
    if fail_count > 0 { std::process::exit(1); }
    Ok(())
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
