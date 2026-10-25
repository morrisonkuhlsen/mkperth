//! The lines of an XER file: a header (`ERMHDR`), then tables, each its name (`%T`), its
//! columns (`%F`) and its rows (`%R`), the fields split by tabs, and an end (`%E`). As public
//! third-party descriptions give them (PRONOM's fmt/1455 and the descriptions it cites): Oracle
//! publishes the tables and their columns, not the lines.

use crate::XerError;

/// The most tables, columns in a table and rows in all a file may hold.
pub const MAX_TABLES: usize = 1_000;
pub const MAX_COLUMNS: usize = 1_000;
pub const MAX_ROWS: usize = 2_000_000;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(name: &str, columns: &[&str]) -> Table {
        Table { name: name.to_string(), columns: columns.iter().map(|c| c.to_string()).collect(), rows: Vec::new() }
    }

    /// Each row, its fields found by their column's name.
    pub fn records(&self) -> impl Iterator<Item = Record<'_>> {
        self.rows.iter().map(move |row| Record { table: self, row })
    }
}

/// One row of a table.
#[derive(Clone, Copy)]
pub struct Record<'a> {
    table: &'a Table,
    row: &'a [String],
}

impl<'a> Record<'a> {
    /// The trimmed field under `column`, if the row has one that isn't empty.
    pub fn get(&self, column: &str) -> Option<&'a str> {
        let k = self.table.columns.iter().position(|c| c == column)?;
        self.row.get(k).map(|v| v.trim()).filter(|v| !v.is_empty())
    }
}

/// A whole file: the header's fields after `ERMHDR`, and the tables in their order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    pub header: Vec<String>,
    pub tables: Vec<Table>,
}

impl File {
    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.name == name)
    }

    /// The rows of the table `name`; none when the file hasn't it.
    pub fn records(&self, name: &str) -> impl Iterator<Item = Record<'_>> {
        self.table(name).into_iter().flat_map(Table::records)
    }
}

/// Reads the lines of an XER file.
pub fn parse(text: &str) -> Result<File, XerError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.split('\n').map(|l| l.trim_end_matches('\r')).enumerate().filter(|(_, l)| !l.trim().is_empty());
    let header = match lines.next() {
        Some((_, first)) if first.starts_with("ERMHDR") => first.split('\t').skip(1).map(str::to_string).collect(),
        _ => return Err(XerError::NotXer),
    };
    let mut file = File { header, tables: Vec::new() };
    let mut rows = 0usize;
    for (n, line) in lines {
        let mut fields = line.split('\t');
        match fields.next().map(str::trim) {
            Some("%T") => {
                if file.tables.len() >= MAX_TABLES {
                    return Err(XerError::TooBig);
                }
                file.tables.push(Table { name: fields.next().unwrap_or_default().trim().to_string(), ..Table::default() });
            }
            Some("%F") => {
                let columns: Vec<String> = fields.map(|f| f.trim().to_string()).collect();
                if columns.len() > MAX_COLUMNS {
                    return Err(XerError::TooBig);
                }
                file.tables.last_mut().ok_or(XerError::Line(n + 1))?.columns = columns;
            }
            Some("%R") => {
                rows += 1;
                if rows > MAX_ROWS {
                    return Err(XerError::TooBig);
                }
                let table = file.tables.last_mut().ok_or(XerError::Line(n + 1))?;
                let row: Vec<String> = fields.take(MAX_COLUMNS).map(str::to_string).collect();
                table.rows.push(row);
            }
            Some("%E") => break,
            // Anything else isn't data: left alone.
            _ => {}
        }
    }
    Ok(file)
}

/// A field as a row may hold it: no tabs and no line breaks.
fn field(value: &str) -> String {
    value.replace(['\t', '\r', '\n'], " ")
}

/// Writes the lines of an XER file, with the line breaks Windows writes.
pub fn write(file: &File) -> String {
    let mut out = String::from("ERMHDR");
    for h in &file.header {
        out.push('\t');
        out.push_str(&field(h));
    }
    out.push_str("\r\n");
    for t in file.tables.iter().filter(|t| !t.rows.is_empty()) {
        out.push_str(&format!("%T\t{}\r\n%F", t.name));
        for c in &t.columns {
            out.push('\t');
            out.push_str(c);
        }
        out.push_str("\r\n");
        for row in &t.rows {
            out.push_str("%R");
            for v in row {
                out.push('\t');
                out.push_str(&field(v));
            }
            out.push_str("\r\n");
        }
    }
    out.push_str("%E\r\n");
    out
}

/// What Windows-1252 has from 0x80 to 0x9F; the five bytes it leaves undefined stand for the
/// control characters of the same number, as Windows reads them.
const WINDOWS_1252: [char; 32] = [
    '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{2c6}', '\u{2030}', '\u{160}', '\u{2039}',
    '\u{152}', '\u{8d}', '\u{17d}', '\u{8f}', '\u{90}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{9d}', '\u{17e}', '\u{178}',
];

/// A file's bytes as text: UTF-8 when they are, else Windows-1252, the code page XER files are
/// usually written in.
pub fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes
            .iter()
            .map(|&b| match b {
                0x80..=0x9f => WINDOWS_1252.get(usize::from(b - 0x80)).copied().unwrap_or('?'),
                _ => char::from(b),
            })
            .collect(),
    }
}

/// Text as Windows-1252 bytes; a character the code page hasn't becomes `?`.
pub fn encode(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| match u32::from(c) {
            n @ (0..=0x7f | 0xa0..=0xff) => u8::try_from(n).unwrap_or(b'?'),
            _ => WINDOWS_1252.iter().position(|w| *w == c).and_then(|k| u8::try_from(k + 0x80).ok()).unwrap_or(b'?'),
        })
        .collect()
}
