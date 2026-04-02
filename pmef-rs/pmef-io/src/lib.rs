//! # pmef-io
//!
//! Streaming NDJSON reader and writer for PMEF packages.
//! Processes one object per line without loading the full file into memory.

use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use thiserror::Error;

/// Errors from PMEF I/O operations.
#[derive(Debug, Error)]
pub enum PmefIoError {
    #[error("IO error on line {line}: {source}")]
    Io { line: usize, #[source] source: std::io::Error },

    #[error("JSON parse error on line {line}: {source}")]
    Json { line: usize, #[source] source: serde_json::Error },

    #[error("Line {line}: object missing '@type' field")]
    MissingType { line: usize },

    #[error("Line {line}: object missing '@id' field")]
    MissingId { line: usize },

    #[error("Line {0} exceeds maximum length of {1} bytes")]
    LineTooLong(usize, usize),
}

/// A raw PMEF object parsed from NDJSON — `@type`, `@id`, and raw JSON.
#[derive(Debug)]
pub struct RawPmefObject {
    pub line_number: usize,
    pub type_str: String,
    pub id_str: String,
    pub value: Value,
}

impl RawPmefObject {
    /// Returns the `@id` value.
    pub fn id(&self) -> &str { &self.id_str }
    /// Returns the `@type` value.
    pub fn type_str(&self) -> &str { &self.type_str }
}

/// Configuration for the NDJSON reader.
#[derive(Debug, Clone)]
pub struct ReaderConfig {
    /// Maximum line length in bytes. Default: 8 MB.
    pub max_line_bytes: usize,
    /// If true, skip comment lines starting with `//`.
    pub skip_comments: bool,
    /// If true, skip empty lines silently.
    pub skip_empty_lines: bool,
    /// If true, stop on first parse error; otherwise collect errors.
    pub fail_fast: bool,
}

impl Default for ReaderConfig {
    fn default() -> Self {
        Self {
            max_line_bytes: 8 * 1024 * 1024, // 8 MB
            skip_comments: true,
            skip_empty_lines: true,
            fail_fast: false,
        }
    }
}

/// Streaming PMEF NDJSON reader.
///
/// Reads one object per line. The `@type` and `@id` fields are extracted
/// eagerly; the rest of the JSON is kept as a `serde_json::Value` for
/// downstream processing.
///
/// # Example
/// ```ignore
/// use pmef_io::NdjsonReader;
/// use std::fs::File;
///
/// let file = File::open("pump-skid.ndjson")?;
/// let reader = NdjsonReader::new(file, Default::default());
///
/// for result in reader {
///     let obj = result?;
///     println!("[{}] {}", obj.type_str, obj.id_str);
/// }
/// ```
pub struct NdjsonReader<R: BufRead> {
    inner: R,
    config: ReaderConfig,
    line_number: usize,
    errors: Vec<PmefIoError>,
}

impl<R: BufRead> NdjsonReader<R> {
    pub fn new(reader: R, config: ReaderConfig) -> Self {
        Self { inner: reader, config, line_number: 0, errors: Vec::new() }
    }

    /// Accumulated non-fatal parse errors (when `fail_fast = false`).
    pub fn errors(&self) -> &[PmefIoError] { &self.errors }

    /// Total number of lines consumed so far.
    pub fn lines_consumed(&self) -> usize { self.line_number }

    fn parse_line(&self, line: &str) -> Result<Option<RawPmefObject>, PmefIoError> {
        let ln = self.line_number;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            return Ok(None);
        }
        if self.config.skip_comments && trimmed.starts_with("//") {
            return Ok(None);
        }
        if trimmed.len() > self.config.max_line_bytes {
            return Err(PmefIoError::LineTooLong(ln, self.config.max_line_bytes));
        }

        let value: Value = serde_json::from_str(trimmed)
            .map_err(|e| PmefIoError::Json { line: ln, source: e })?;

        let type_str = value.get("@type")
            .and_then(Value::as_str)
            .ok_or(PmefIoError::MissingType { line: ln })?
            .to_owned();

        let id_str = value.get("@id")
            .and_then(Value::as_str)
            .ok_or(PmefIoError::MissingId { line: ln })?
            .to_owned();

        Ok(Some(RawPmefObject { line_number: ln, type_str, id_str, value }))
    }
}

impl<R: BufRead> Iterator for NdjsonReader<R> {
    type Item = Result<RawPmefObject, PmefIoError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut line = String::new();
            self.line_number += 1;
            match self.inner.read_line(&mut line) {
                Ok(0) => return None, // EOF
                Ok(_) => {}
                Err(e) => return Some(Err(PmefIoError::Io {
                    line: self.line_number, source: e,
                })),
            }

            match self.parse_line(&line) {
                Ok(None) => continue, // comment or blank line
                Ok(Some(obj)) => return Some(Ok(obj)),
                Err(e) => {
                    if self.config.fail_fast {
                        return Some(Err(e));
                    } else {
                        self.errors.push(e);
                        continue;
                    }
                }
            }
        }
    }
}

/// Convenience: open and read a PMEF NDJSON file, collecting all raw objects.
pub fn read_ndjson_file(path: &std::path::Path) -> Result<Vec<RawPmefObject>, PmefIoError> {
    let file = std::fs::File::open(path)
        .map_err(|e| PmefIoError::Io { line: 0, source: e })?;
    let reader = BufReader::new(file);
    let ndjson = NdjsonReader::new(reader, ReaderConfig::default());
    ndjson.collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// NDJSON Writer
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for the NDJSON writer.
#[derive(Debug, Clone)]
pub struct WriterConfig {
    /// If true, sort JSON keys alphabetically (canonical form).
    pub canonical_key_order: bool,
    /// Line terminator (default: `"\n"`).
    pub line_terminator: String,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            canonical_key_order: true,
            line_terminator: "\n".to_owned(),
        }
    }
}

/// Streaming PMEF NDJSON writer.
///
/// # Example
/// ```ignore
/// use pmef_io::NdjsonWriter;
///
/// let mut writer = NdjsonWriter::new(std::io::stdout(), Default::default());
/// let pump_json = serde_json::json!({ "@type": "pmef:Pump", "@id": "urn:pmef:obj:x:P1" });
/// writer.write_value(&pump_json)?;
/// ```
pub struct NdjsonWriter<W: Write> {
    inner: W,
    config: WriterConfig,
    objects_written: usize,
}

impl<W: Write> NdjsonWriter<W> {
    pub fn new(writer: W, config: WriterConfig) -> Self {
        Self { inner: writer, config, objects_written: 0 }
    }

    /// Write a single PMEF object as one NDJSON line.
    pub fn write_value(&mut self, value: &Value) -> Result<(), PmefIoError> {
        let serialised = if self.config.canonical_key_order {
            canonical_json(value)
        } else {
            serde_json::to_string(value)
                .map_err(|e| PmefIoError::Json { line: self.objects_written + 1, source: e })?
        };
        self.inner.write_all(serialised.as_bytes())
            .map_err(|e| PmefIoError::Io { line: self.objects_written + 1, source: e })?;
        self.inner.write_all(self.config.line_terminator.as_bytes())
            .map_err(|e| PmefIoError::Io { line: self.objects_written + 1, source: e })?;
        self.objects_written += 1;
        Ok(())
    }

    /// Write a serialisable PMEF entity.
    pub fn write_entity<T: serde::Serialize>(&mut self, entity: &T) -> Result<(), PmefIoError> {
        let value = serde_json::to_value(entity)
            .map_err(|e| PmefIoError::Json { line: self.objects_written + 1, source: e })?;
        self.write_value(&value)
    }

    /// Flush the underlying writer.
    pub fn flush(&mut self) -> Result<(), std::io::Error> { self.inner.flush() }

    /// Number of objects written so far.
    pub fn objects_written(&self) -> usize { self.objects_written }
}

/// Serialise a `serde_json::Value` with alphabetically sorted keys.
/// This is the PMEF canonical serialisation form.
pub fn canonical_json(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys.iter().map(|k| {
                let v = canonical_json(&map[*k]);
                format!("{}:{}", serde_json::to_string(k).unwrap(), v)
            }).collect();
            format!("{{{}}}", inner.join(","))
        }
        Value::Array(arr) => {
            let inner: Vec<String> = arr.iter().map(canonical_json).collect();
            format!("[{}]", inner.join(","))
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Package Index
// ─────────────────────────────────────────────────────────────────────────────

/// An in-memory index of a PMEF package, keyed by `@id`.
///
/// Used for resolving references during adapter processing.
#[derive(Debug, Default)]
pub struct PmefPackageIndex {
    pub objects: std::collections::HashMap<String, RawPmefObject>,
    pub objects_by_type: std::collections::HashMap<String, Vec<String>>,
}

impl PmefPackageIndex {
    /// Build an index from an iterator of raw PMEF objects.
    pub fn from_iter<I: Iterator<Item = RawPmefObject>>(iter: I) -> Self {
        let mut idx = Self::default();
        for obj in iter {
            idx.objects_by_type
                .entry(obj.type_str.clone())
                .or_default()
                .push(obj.id_str.clone());
            idx.objects.insert(obj.id_str.clone(), obj);
        }
        idx
    }

    /// Resolve a `@id` reference, returning the raw object if found.
    pub fn resolve(&self, id: &str) -> Option<&RawPmefObject> {
        // Strip port fragment if present
        let id = id.split('#').next().unwrap_or(id);
        self.objects.get(id)
    }

    /// Returns all objects of a given `@type`.
    pub fn by_type(&self, type_str: &str) -> Vec<&RawPmefObject> {
        self.objects_by_type
            .get(type_str)
            .map(|ids| ids.iter().filter_map(|id| self.objects.get(id)).collect())
            .unwrap_or_default()
    }

    /// Total number of objects in the index.
    pub fn len(&self) -> usize { self.objects.len() }

    pub fn is_empty(&self) -> bool { self.objects.is_empty() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_reader_minimal() {
        let ndjson = r#"{"@type":"pmef:FileHeader","@id":"urn:pmef:pkg:test:hdr","pmefVersion":"0.9.0","plantId":"urn:pmef:plant:test:P","revisionId":"r2026-01-01-001","changeState":"WIP"}
{"@type":"pmef:Pump","@id":"urn:pmef:obj:test:P-201A","pmefVersion":"0.9.0","isPartOf":"urn:pmef:unit:test:U-100"}
"#;
        let reader = NdjsonReader::new(Cursor::new(ndjson), ReaderConfig::default());
        let objects: Vec<_> = reader.collect::<Result<_, _>>().unwrap();
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0].type_str, "pmef:FileHeader");
        assert_eq!(objects[1].id_str, "urn:pmef:obj:test:P-201A");
    }

    #[test]
    fn test_reader_skip_comments() {
        let ndjson = "// comment\n{\"@type\":\"pmef:Plant\",\"@id\":\"urn:pmef:plant:test:P\"}\n";
        let reader = NdjsonReader::new(Cursor::new(ndjson), ReaderConfig::default());
        let objects: Vec<_> = reader.collect::<Result<_, _>>().unwrap();
        assert_eq!(objects.len(), 1);
    }

    #[test]
    fn test_canonical_json_sorts_keys() {
        let v = serde_json::json!({"z": 1, "a": 2, "m": 3});
        let out = canonical_json(&v);
        assert_eq!(out, r#"{"a":2,"m":3,"z":1}"#);
    }

    #[test]
    fn test_writer_roundtrip() {
        let mut buf = Vec::new();
        let mut writer = NdjsonWriter::new(&mut buf, WriterConfig::default());
        let value = serde_json::json!({"@type":"pmef:Pump","@id":"urn:pmef:obj:test:P1"});
        writer.write_value(&value).unwrap();
        writer.flush().unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.ends_with('\n'));
        // Canonical: @id before @type (alphabetical)
        assert!(output.starts_with("{\"@id\""));
    }

    #[test]
    fn test_package_index() {
        let ndjson = "{\"@type\":\"pmef:Pump\",\"@id\":\"urn:pmef:obj:t:P1\"}\n\
                      {\"@type\":\"pmef:Pipe\",\"@id\":\"urn:pmef:obj:t:PIPE1\"}\n";
        let reader = NdjsonReader::new(Cursor::new(ndjson), ReaderConfig::default());
        let objects: Vec<_> = reader.collect::<Result<_,_>>().unwrap();
        let idx = PmefPackageIndex::from_iter(objects.into_iter());
        assert_eq!(idx.len(), 2);
        assert!(idx.resolve("urn:pmef:obj:t:P1").is_some());
        assert_eq!(idx.by_type("pmef:Pump").len(), 1);
    }
}
