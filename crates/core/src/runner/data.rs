//! Data files for data-driven runs: CSV with a header row, or a JSON array of objects. Each row
//! becomes a set of variables for one run of the selection.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::{Error, Result};

pub fn load_data(path: &Path) -> Result<Vec<BTreeMap<String, String>>> {
    let text = fs::read_to_string(path)
        .map_err(|err| Error::Invalid(format!("failed to read {}: {err}", path.display())))?;
    let json = path.extension().is_some_and(|ext| ext == "json");
    parse_data(&text, json)
}

pub fn parse_data(text: &str, json: bool) -> Result<Vec<BTreeMap<String, String>>> {
    // Excel's "CSV UTF-8" and some editors start files with a byte order mark.
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    if json || text.trim_start().starts_with('[') {
        return parse_json(text);
    }
    parse_csv(text)
}

fn parse_json(text: &str) -> Result<Vec<BTreeMap<String, String>>> {
    let value: Value = serde_json::from_str(text)?;
    let Value::Array(rows) = value else {
        return Err(Error::Invalid(
            "a JSON data file must be an array of objects".into(),
        ));
    };
    rows.into_iter()
        .map(|row| match row {
            Value::Object(fields) => Ok(fields
                .into_iter()
                .map(|(key, value)| {
                    let text = match value {
                        Value::String(text) => text,
                        Value::Null => String::new(),
                        other => other.to_string(),
                    };
                    (key, text)
                })
                .collect()),
            _ => Err(Error::Invalid(
                "every row of a JSON data file must be an object".into(),
            )),
        })
        .collect()
}

fn parse_csv(text: &str) -> Result<Vec<BTreeMap<String, String>>> {
    let mut records = csv_records(text)?.into_iter();
    let Some(header) = records.next() else {
        return Ok(Vec::new());
    };
    let header: Vec<String> = header.into_iter().map(|h| h.trim().to_string()).collect();
    records
        .filter(|record| !(record.len() == 1 && record[0].is_empty()))
        .enumerate()
        .map(|(i, record)| {
            if record.len() != header.len() {
                return Err(Error::Invalid(format!(
                    "data row {} has {} columns, the header has {}",
                    i + 1,
                    record.len(),
                    header.len()
                )));
            }
            Ok(header.iter().cloned().zip(record).collect())
        })
        .collect()
}

/// RFC 4180 records: commas, double-quoted fields with `""` escapes and embedded newlines.
fn csv_records(text: &str) -> Result<Vec<Vec<String>>> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            (true, '"') => quoted = false,
            (true, c) => field.push(c),
            (false, '"') if field.is_empty() => quoted = true,
            (false, ',') => record.push(std::mem::take(&mut field)),
            (false, '\r') => {}
            (false, '\n') => {
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
            }
            (false, c) => field.push(c),
        }
    }
    if quoted {
        return Err(Error::Invalid("data file has an unclosed quote".into()));
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_csv_with_quotes_and_blank_lines() {
        let rows = parse_data(
            "name,note\nAda,\"likes, commas\"\n\"Grace \"\"G\"\"\",\"two\nlines\"\n\n",
            false,
        )
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["note"], "likes, commas");
        assert_eq!(rows[1]["name"], "Grace \"G\"");
        assert_eq!(rows[1]["note"], "two\nlines");
    }

    #[test]
    fn ignores_a_byte_order_mark() {
        let csv = parse_data("\u{FEFF}user,id\r\nada,1\r\n", false).unwrap();
        assert_eq!(csv[0]["user"], "ada");
        let json = parse_data("\u{FEFF}[{\"user\":\"ada\"}]", false).unwrap();
        assert_eq!(json[0]["user"], "ada");
    }

    #[test]
    fn parses_json_arrays() {
        let rows = parse_data(
            r#"[{"id": 1, "name": "Ada", "admin": true, "x": null}]"#,
            true,
        )
        .unwrap();
        assert_eq!(rows[0]["id"], "1");
        assert_eq!(rows[0]["admin"], "true");
        assert_eq!(rows[0]["x"], "");
    }

    #[test]
    fn rejects_ragged_rows_and_bad_json() {
        assert!(parse_data("a,b\n1\n", false).is_err());
        assert!(parse_data("a\n\"open\n", false).is_err());
        assert!(parse_data("{\"a\":1}", true).is_err());
    }
}
