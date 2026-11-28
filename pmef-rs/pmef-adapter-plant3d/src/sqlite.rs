//! Minimal read-only reader for SQLite database files.
//!
//! AutoCAD Plant 3D keeps its project data (`Piping.dcf`, `ProcessPower.dcf`) and its spec
//! catalogs (`.pspc`) in SQLite files. This reader implements just enough of the published file
//! format (<https://www.sqlite.org/fileformat2.html>) to list tables and read their rows: the
//! database header, table b-trees (interior and leaf pages), overflow pages and the record format.
//! It does not read indexes, does not apply a write-ahead log and never writes. Pure Rust, so it
//! also runs in WebAssembly, where the C SQLite library is not available.

use std::collections::HashMap;

/// A value of one column.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// SQL NULL.
    Null,
    /// Integer.
    Integer(i64),
    /// IEEE 754 double.
    Real(f64),
    /// UTF-8 text.
    Text(String),
    /// Binary data.
    Blob(Vec<u8>),
}

impl Value {
    /// Number (integer, real or numeric text).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Integer(i) => Some(*i as f64),
            Value::Real(r) => Some(*r),
            Value::Text(t) => t.trim().parse().ok(),
            _ => None,
        }
    }
    /// Integer (integer, integral real or numeric text).
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Integer(i) => Some(*i),
            Value::Real(r) if r.fract() == 0.0 => Some(*r as i64),
            Value::Text(t) => t.trim().parse().ok(),
            _ => None,
        }
    }
    /// Text of a text value; `None` for NULL and empty text.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(t) if !t.is_empty() => Some(t),
            _ => None,
        }
    }
    /// Bytes of a blob value.
    pub fn as_blob(&self) -> Option<&[u8]> {
        match self {
            Value::Blob(b) => Some(b),
            _ => None,
        }
    }
}

/// Errors of the reader.
#[derive(Debug, thiserror::Error)]
pub enum SqliteError {
    /// The file has no SQLite 3 header.
    #[error("not an SQLite 3 database")]
    NotSqlite,
    /// The file structure is broken.
    #[error("corrupt database: {0}")]
    Corrupt(String),
    /// The table does not exist.
    #[error("no table {0}")]
    NoTable(String),
}

type Result<T> = std::result::Result<T, SqliteError>;

fn corrupt(msg: impl Into<String>) -> SqliteError {
    SqliteError::Corrupt(msg.into())
}

/// A table: column names and its root page.
#[derive(Debug, Clone)]
pub struct TableInfo {
    /// Table name as declared.
    pub name: String,
    /// Column names in declaration order.
    pub columns: Vec<String>,
    root_page: u32,
    /// Column that is an alias of the rowid (`INTEGER PRIMARY KEY`), stored as NULL in records.
    rowid_alias: Option<usize>,
}

/// An open database (the whole file in memory).
pub struct Database<'a> {
    data: &'a [u8],
    page_size: usize,
    usable: usize,
    tables: HashMap<String, TableInfo>,
}

/// One row: values in column order.
#[derive(Debug, Clone)]
pub struct Row {
    /// Rowid.
    pub rowid: i64,
    /// Values in column order.
    pub values: Vec<Value>,
}

/// Rows of a table with access by column name.
#[derive(Debug, Clone)]
pub struct Rows {
    /// Column names.
    pub columns: Vec<String>,
    /// The rows.
    pub rows: Vec<Row>,
    index: HashMap<String, usize>,
}

impl Rows {
    /// Position of a column (case-insensitive).
    pub fn col(&self, name: &str) -> Option<usize> {
        self.index.get(&name.to_ascii_lowercase()).copied()
    }
    /// Value of `column` in `row`; `Null` when the column does not exist or the record is short.
    pub fn get<'r>(&self, row: &'r Row, column: &str) -> &'r Value {
        const NULL: &Value = &Value::Null;
        self.col(column)
            .and_then(|i| row.values.get(i))
            .unwrap_or(NULL)
    }
}

impl<'a> Database<'a> {
    /// Opens a database held in memory.
    pub fn open(data: &'a [u8]) -> Result<Self> {
        if data.len() < 100 || &data[..16] != b"SQLite format 3\0" {
            return Err(SqliteError::NotSqlite);
        }
        let raw = u16::from_be_bytes([data[16], data[17]]) as usize;
        let page_size = if raw == 1 { 65536 } else { raw };
        if !(512..=65536).contains(&page_size) || !page_size.is_power_of_two() {
            return Err(corrupt("page size"));
        }
        let usable = page_size - data[20] as usize;
        let mut db = Database {
            data,
            page_size,
            usable,
            tables: HashMap::new(),
        };
        // sqlite_schema: type, name, tbl_name, rootpage, sql.
        let mut rows = Vec::new();
        db.walk(1, &mut rows, 0)?;
        for row in rows {
            let v = &row.values;
            if v.first().and_then(Value::as_str) != Some("table") {
                continue;
            }
            let (Some(name), Some(root), Some(sql)) = (
                v.get(1).and_then(Value::as_str),
                v.get(3).and_then(Value::as_i64),
                v.get(4).and_then(Value::as_str),
            ) else {
                continue;
            };
            let (columns, rowid_alias) = parse_columns(sql);
            db.tables.insert(
                name.to_ascii_lowercase(),
                TableInfo {
                    name: name.to_string(),
                    columns,
                    root_page: root as u32,
                    rowid_alias,
                },
            );
        }
        Ok(db)
    }

    /// Names of all tables.
    pub fn table_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.tables.values().map(|t| t.name.as_str()).collect();
        names.sort_unstable();
        names
    }

    /// The table exists (case-insensitive).
    pub fn has_table(&self, name: &str) -> bool {
        self.tables.contains_key(&name.to_ascii_lowercase())
    }

    /// All rows of a table, in rowid order.
    pub fn rows(&self, table: &str) -> Result<Rows> {
        let info = self
            .tables
            .get(&table.to_ascii_lowercase())
            .ok_or_else(|| SqliteError::NoTable(table.to_string()))?;
        let mut rows = Vec::new();
        if info.root_page > 0 {
            self.walk(info.root_page, &mut rows, 0)?;
        }
        if let Some(alias) = info.rowid_alias {
            for row in &mut rows {
                if let Some(v) = row.values.get_mut(alias) {
                    if *v == Value::Null {
                        *v = Value::Integer(row.rowid);
                    }
                }
            }
        }
        let index = info
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| (c.to_ascii_lowercase(), i))
            .collect();
        Ok(Rows {
            columns: info.columns.clone(),
            rows,
            index,
        })
    }

    /// Rows of a table, or an empty set when the table does not exist.
    pub fn rows_or_empty(&self, table: &str) -> Rows {
        self.rows(table).unwrap_or_else(|_| Rows {
            columns: Vec::new(),
            rows: Vec::new(),
            index: HashMap::new(),
        })
    }

    fn page(&self, number: u32) -> Result<&'a [u8]> {
        let start = (number as usize)
            .checked_sub(1)
            .ok_or_else(|| corrupt("page 0"))?
            * self.page_size;
        self.data
            .get(start..start + self.page_size)
            .ok_or_else(|| corrupt(format!("page {number} beyond end of file")))
    }

    fn walk(&self, number: u32, out: &mut Vec<Row>, depth: usize) -> Result<()> {
        if depth > 64 {
            return Err(corrupt("b-tree too deep"));
        }
        let page = self.page(number)?;
        let head = if number == 1 { 100 } else { 0 };
        let kind = *page.get(head).ok_or_else(|| corrupt("page header"))?;
        let cells = u16::from_be_bytes([page[head + 3], page[head + 4]]) as usize;
        let (header_len, interior) = match kind {
            0x0D => (8, false),
            0x05 => (12, true),
            // Index pages are not part of table b-trees.
            _ => return Err(corrupt(format!("page {number} type {kind:#x}"))),
        };
        let ptrs = head + header_len;
        for i in 0..cells {
            let at = ptrs + 2 * i;
            let off = u16::from_be_bytes([
                *page.get(at).ok_or_else(|| corrupt("cell pointer"))?,
                *page.get(at + 1).ok_or_else(|| corrupt("cell pointer"))?,
            ]) as usize;
            if interior {
                let child = u32::from_be_bytes(
                    page.get(off..off + 4)
                        .ok_or_else(|| corrupt("child pointer"))?
                        .try_into()
                        .unwrap(),
                );
                self.walk(child, out, depth + 1)?;
            } else {
                out.push(self.leaf_cell(page, off)?);
            }
        }
        if interior {
            let right = u32::from_be_bytes(page[head + 8..head + 12].try_into().unwrap());
            self.walk(right, out, depth + 1)?;
        }
        Ok(())
    }

    fn leaf_cell(&self, page: &[u8], off: usize) -> Result<Row> {
        let (payload_len, n1) = varint(page, off)?;
        let (rowid, n2) = varint(page, off + n1)?;
        let start = off + n1 + n2;
        let p = payload_len as usize;
        let u = self.usable;
        let x = u - 35;
        let payload = if p <= x {
            page.get(start..start + p)
                .ok_or_else(|| corrupt("payload"))?
                .to_vec()
        } else {
            let m = ((u - 12) * 32 / 255) - 23;
            let k = m + ((p - m) % (u - 4));
            let local = if k <= x { k } else { m };
            let mut buf = page
                .get(start..start + local)
                .ok_or_else(|| corrupt("payload"))?
                .to_vec();
            let mut next = u32::from_be_bytes(
                page.get(start + local..start + local + 4)
                    .ok_or_else(|| corrupt("overflow pointer"))?
                    .try_into()
                    .unwrap(),
            );
            let mut guard = 0usize;
            while buf.len() < p && next != 0 {
                guard += 1;
                if guard > self.data.len() / self.page_size + 1 {
                    return Err(corrupt("overflow loop"));
                }
                let ov = self.page(next)?;
                next = u32::from_be_bytes(ov[..4].try_into().unwrap());
                let take = (p - buf.len()).min(u - 4);
                buf.extend_from_slice(&ov[4..4 + take]);
            }
            buf
        };
        Ok(Row {
            rowid,
            values: record(&payload)?,
        })
    }
}

/// SQLite varint: 1–9 bytes, big endian, 7 bits per byte (8 in the ninth).
fn varint(data: &[u8], at: usize) -> Result<(i64, usize)> {
    let mut v: u64 = 0;
    for i in 0..9 {
        let b = *data.get(at + i).ok_or_else(|| corrupt("varint"))?;
        if i == 8 {
            v = (v << 8) | b as u64;
            return Ok((v as i64, 9));
        }
        v = (v << 7) | (b & 0x7F) as u64;
        if b & 0x80 == 0 {
            return Ok((v as i64, i + 1));
        }
    }
    unreachable!()
}

fn record(payload: &[u8]) -> Result<Vec<Value>> {
    let (header_len, mut pos) = varint(payload, 0)?;
    let header_len = header_len as usize;
    let mut types = Vec::new();
    while pos < header_len {
        let (t, n) = varint(payload, pos)?;
        types.push(t);
        pos += n;
    }
    let mut body = header_len;
    let mut values = Vec::with_capacity(types.len());
    for t in types {
        let take = |len: usize, body: &mut usize| -> Result<&[u8]> {
            let s = payload
                .get(*body..*body + len)
                .ok_or_else(|| corrupt("record body"))?;
            *body += len;
            Ok(s)
        };
        let int = |b: &[u8]| -> i64 {
            let mut v: i64 = if b[0] & 0x80 != 0 { -1 } else { 0 };
            for &x in b {
                v = (v << 8) | x as i64;
            }
            v
        };
        let v = match t {
            0 => Value::Null,
            1 => Value::Integer(int(take(1, &mut body)?)),
            2 => Value::Integer(int(take(2, &mut body)?)),
            3 => Value::Integer(int(take(3, &mut body)?)),
            4 => Value::Integer(int(take(4, &mut body)?)),
            5 => Value::Integer(int(take(6, &mut body)?)),
            6 => Value::Integer(int(take(8, &mut body)?)),
            7 => Value::Real(f64::from_be_bytes(take(8, &mut body)?.try_into().unwrap())),
            8 => Value::Integer(0),
            9 => Value::Integer(1),
            n if n >= 12 && n % 2 == 0 => {
                Value::Blob(take(((n - 12) / 2) as usize, &mut body)?.to_vec())
            }
            n if n >= 13 => Value::Text(
                String::from_utf8_lossy(take(((n - 13) / 2) as usize, &mut body)?).into_owned(),
            ),
            _ => return Err(corrupt(format!("serial type {t}"))),
        };
        values.push(v);
    }
    Ok(values)
}

/// Column names of a `CREATE TABLE` statement and the rowid-alias column, if any.
fn parse_columns(sql: &str) -> (Vec<String>, Option<usize>) {
    let (Some(open), Some(close)) = (sql.find('('), sql.rfind(')')) else {
        return (Vec::new(), None);
    };
    let body = &sql[open + 1..close];
    let mut parts = Vec::new();
    let (mut depth, mut start, mut quote) = (0i32, 0usize, None::<char>);
    for (i, ch) in body.char_indices() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'' | '`') => quote = Some(ch),
            (None, '[') => quote = Some(']'),
            (None, '(') => depth += 1,
            (None, ')') => depth -= 1,
            (None, ',') if depth == 0 => {
                parts.push(&body[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&body[start..]);
    let mut columns = Vec::new();
    let mut alias = None;
    let mut table_pk: Vec<String> = Vec::new();
    for part in parts {
        let part = part.trim();
        let upper = part.to_ascii_uppercase();
        if ["PRIMARY", "UNIQUE", "CONSTRAINT", "FOREIGN", "CHECK"]
            .iter()
            .any(|k| upper.starts_with(k))
        {
            if upper.starts_with("PRIMARY") {
                if let (Some(a), Some(b)) = (part.find('('), part.rfind(')')) {
                    table_pk = part[a + 1..b].split(',').map(unquote).collect();
                }
            }
            continue;
        }
        let (name, rest) = split_name(part);
        if name.is_empty() {
            continue;
        }
        let rest_up = rest.to_ascii_uppercase();
        if rest_up.trim_start().starts_with("INTEGER") && rest_up.contains("PRIMARY KEY") {
            alias = Some(columns.len());
        }
        columns.push(name);
    }
    if alias.is_none() && table_pk.len() == 1 {
        // `PRIMARY KEY ("PnPID")` on an `integer` column is a rowid alias too.
        if let Some(i) = columns
            .iter()
            .position(|c| c.eq_ignore_ascii_case(&table_pk[0]))
        {
            let decl = sql.to_ascii_uppercase();
            let col = columns[i].to_ascii_uppercase();
            let integer = decl
                .split(',')
                .any(|p| p.contains(&col) && p.contains("INTEGER") && !p.contains("PRIMARY"));
            if integer {
                alias = Some(i);
            }
        }
    }
    (columns, alias)
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix(['"', '`', '[', '\'']).unwrap_or(s);
    s.strip_suffix(['"', '`', ']', '\''])
        .unwrap_or(s)
        .to_string()
}

/// Column name (possibly quoted, may contain spaces when quoted) and the rest of the definition.
fn split_name(part: &str) -> (String, &str) {
    let close = match part.chars().next() {
        Some('"') => Some('"'),
        Some('`') => Some('`'),
        Some('[') => Some(']'),
        Some('\'') => Some('\''),
        _ => None,
    };
    match close {
        Some(c) => match part[1..].find(c) {
            Some(end) => (part[1..1 + end].to_string(), &part[2 + end..]),
            None => (unquote(part), ""),
        },
        None => match part.find(char::is_whitespace) {
            Some(end) => (part[..end].to_string(), &part[end..]),
            None => (part.to_string(), ""),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_names_from_create_table() {
        let (cols, alias) = parse_columns(
            r#"CREATE TABLE "EngineeringItems"("PnPID" integer,"Position X" Double,"Spec" VarChar (255),Primary key ("PnPID"))"#,
        );
        assert_eq!(cols, ["PnPID", "Position X", "Spec"]);
        assert_eq!(alias, Some(0));
        let (cols, alias) = parse_columns(
            "CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT, x REAL, UNIQUE(name))",
        );
        assert_eq!(cols, ["id", "name", "x"]);
        assert_eq!(alias, Some(0));
    }

    #[test]
    fn varints() {
        assert_eq!(varint(&[0x7F], 0).unwrap(), (127, 1));
        assert_eq!(varint(&[0x81, 0x00], 0).unwrap(), (128, 2));
    }

    #[test]
    fn rejects_other_files() {
        assert!(matches!(
            Database::open(b"hello"),
            Err(SqliteError::NotSqlite)
        ));
    }
}
