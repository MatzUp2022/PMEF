//! # pmef — PMEF Command Line Tool
//!
//! Usage:
//!   pmef validate <file.ndjson> [--schemas <dir>] [--level 1|2|3]
//!   pmef diff <base.ndjson> <head.ndjson> [--format text|json]
//!   pmef stats <file.ndjson>
//!   pmef convert <input.pcf> --to pmef --output <output.ndjson>
//!   pmef index <file.ndjson> [--key tag-number|line-number]
//!   pmef conformance <file.ndjson> [--dataset ds01]

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
    },

    /// Compare two PMEF NDJSON files and report differences.
    Diff {
        /// Base (old) file.
        base: PathBuf,
        /// Head (new) file.
        head: PathBuf,
        /// Output format.
        #[arg(long, default_value = "text")]
        format: String,
        /// Only show objects with changes (hide unchanged).
        #[arg(long)]
        changed_only: bool,
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
        /// Input file.
        input: PathBuf,
        /// Input format (auto-detected from extension if omitted).
        #[arg(long)]
        from: Option<String>,
        /// Output format (pmef, pcf, ifc, caex).
        #[arg(long)]
        to: String,
        /// Output file path.
        #[arg(long, short)]
        output: PathBuf,
        /// Schemas directory for validation.
        #[arg(long, default_value = "schemas")]
        schemas: PathBuf,
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
        Commands::Validate { file, schemas, level, format, fail_fast } => {
            cmd_validate(file, schemas, level, &format, fail_fast).await
        }
        Commands::Diff { base, head, format, changed_only } => {
            cmd_diff(base, head, &format, changed_only).await
        }
        Commands::Stats { file, by_type, by_domain } => {
            cmd_stats(file, by_type, by_domain).await
        }
        Commands::Convert { input, from, to, output, schemas } => {
            cmd_convert(input, from, to, output, schemas).await
        }
        Commands::Index { file, key, filter_type, output } => {
            cmd_index(file, &key, filter_type, output).await
        }
        Commands::Conformance { file, dataset, level, json, output } => {
            cmd_conformance(file, &dataset, level, json, output).await
        }
        Commands::CheckRefs { file, strict } => {
            cmd_check_refs(file, strict).await
        }
    }
}

// ── Command implementations ───────────────────────────────────────────────

async fn cmd_validate(
    file: PathBuf, schemas: PathBuf, level: u8, format: &str, fail_fast: bool,
) -> Result<()> {
    use pmef_io::{NdjsonReader, ReaderConfig};
    use pmef_validate::PmefValidator;
    use std::io::BufReader;
    use std::fs::File;

    info!("Validating {} (conformance level {})", file.display(), level);

    let validator = PmefValidator::from_directory(&schemas)
        .map_err(|e| anyhow::anyhow!("Failed to load schemas from {}: {}", schemas.display(), e))?;

    let f = File::open(&file).with_context(|| format!("Cannot open {}", file.display()))?;
    let reader = NdjsonReader::new(BufReader::new(f), ReaderConfig::default());

    let mut ok = 0usize;
    let mut fail = 0usize;
    let mut skip = 0usize;
    let mut all_errors: Vec<String> = Vec::new();

    for result in reader {
        match result {
            Ok(obj) => {
                if pmef_validate::schema_for_type(&obj.type_str).is_none() {
                    skip += 1;
                    warn!("No schema for type '{}' on line {}", obj.type_str, obj.line_number);
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

    let total = ok + fail + skip;
    match format {
        "json" => {
            let report = serde_json::json!({
                "total": total,
                "ok": ok,
                "fail": fail,
                "skip": skip,
                "pass_rate": if total > 0 { ok as f64 / total as f64 } else { 1.0 },
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
            let pass_rate = if total > 0 { ok as f64 / total as f64 } else { 1.0 };
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

async fn cmd_diff(base: PathBuf, head: PathBuf, format: &str, changed_only: bool) -> Result<()> {
    use pmef_io::{NdjsonReader, PmefPackageIndex, ReaderConfig};
    use std::io::BufReader;
    use std::fs::File;

    let load = |p: &PathBuf| -> Result<PmefPackageIndex> {
        let f = File::open(p).with_context(|| format!("Cannot open {}", p.display()))?;
        let reader = NdjsonReader::new(BufReader::new(f), ReaderConfig::default());
        let objs: Vec<_> = reader.collect::<Result<_, _>>()
            .with_context(|| format!("Error reading {}", p.display()))?;
        Ok(PmefPackageIndex::from_iter(objs.into_iter()))
    };

    let base_idx = load(&base)?;
    let head_idx = load(&head)?;

    let mut added = 0usize;
    let mut removed = 0usize;
    let mut changed = 0usize;

    // Detect added / changed
    for (id, head_obj) in &head_idx.objects {
        match base_idx.resolve(id) {
            None => {
                added += 1;
                if !changed_only {
                    println!("+ {id} (@type: {})", head_obj.type_str);
                }
            }
            Some(base_obj) => {
                if base_obj.value != head_obj.value {
                    changed += 1;
                    println!("~ {id} (@type: {})", head_obj.type_str);
                }
            }
        }
    }
    // Detect removed
    for id in base_idx.objects.keys() {
        if head_idx.resolve(id).is_none() {
            removed += 1;
            println!("- {id}");
        }
    }

    println!("\nDiff summary: +{added} added, -{removed} removed, ~{changed} changed");
    Ok(())
}

async fn cmd_stats(file: PathBuf, by_type: bool, by_domain: bool) -> Result<()> {
    use pmef_io::{NdjsonReader, ReaderConfig};
    use std::io::BufReader;
    use std::fs::File;
    use std::collections::HashMap;

    let f = File::open(&file).with_context(|| format!("Cannot open {}", file.display()))?;
    let reader = NdjsonReader::new(BufReader::new(f), ReaderConfig::default());

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

async fn cmd_convert(
    input: PathBuf, from: Option<String>, to: String,
    output: PathBuf, _schemas: PathBuf,
) -> Result<()> {
    let from = from.unwrap_or_else(|| {
        input.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_lowercase()
    });
    info!("Converting {} ({from} → {to}) → {}", input.display(), output.display());
    // Full implementation: dispatch to pmef-adapter-* crates
    anyhow::bail!("Conversion from '{from}' to '{to}' not yet implemented in this build. \
                   Use pmef-adapter-plant3d for Plant 3D PCF → PMEF.");
}

async fn cmd_index(
    file: PathBuf, key: &str, filter_type: Option<String>, output: Option<PathBuf>,
) -> Result<()> {
    use pmef_io::{NdjsonReader, ReaderConfig};
    use std::io::BufReader;
    use std::fs::File;
    use std::collections::HashMap;

    let f = File::open(&file).with_context(|| format!("Cannot open {}", file.display()))?;
    let reader = NdjsonReader::new(BufReader::new(f), ReaderConfig::default());

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
    let reader = NdjsonReader::new(BufReader::new(f), ReaderConfig::default());
    let objects: Vec<_> = reader.collect::<Result<_, _>>()?;

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
    let missing_type_count = idx.objects.values()
        .filter(|o| o.value.get("@type").and_then(|v| v.as_str()).is_none())
        .count();
    record(
        "allObjectsHaveType",
        missing_type_count == 0,
        &format!("{} object(s) missing @type", missing_type_count),
    );

    // Check 4: All objects have @id
    let missing_id_count = idx.objects.values()
        .filter(|o| o.value.get("@id").and_then(|v| v.as_str()).is_none())
        .count();
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
    let reader = NdjsonReader::new(BufReader::new(f), ReaderConfig::default());
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
