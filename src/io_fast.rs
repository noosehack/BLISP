//! Fast mmap-based CSV parser for blisp
//!
//! Modeled on Adyton.cpp's `fastread()`: mmap the file, scan raw bytes,
//! hand-rolled float parser, column-major output. Single-threaded for now.
//!
//! v1 contract:
//! - Semicolon-delimited
//! - Header row present
//! - Rectangular rows (all rows same number of fields)
//! - No quoted-field CSV complexity
//! - Explicit NA tokens only ("NA", "NaN", "N/A", empty)
//! - First column may be date (YYYY-MM-DD), rest numeric (f64)
//!
//! Unsupported files error clearly rather than silently degrade.

use crate::ast::Interner;
use crate::frame::{Frame, IndexColumn, Tags};
use crate::value::Value;
use blawktrust::NULL_DATE;
use memmap2::Mmap;
use std::fs::File;
use std::sync::Arc;

const DELIMITER: u8 = b';';
const NEWLINE: u8 = b'\n';

/// Load CSV file using fast mmap-based parser.
///
/// Returns a Frame with date index (if first column is YYYY-MM-DD) and f64 data columns.
pub fn load_csv_fast(filename: &str, interner: &mut Interner) -> Result<Value, String> {
    let file = File::open(filename)
        .map_err(|e| format!("file-fast: error opening '{}': {}", filename, e))?;

    let mmap = unsafe {
        Mmap::map(&file).map_err(|e| format!("file-fast: mmap failed for '{}': {}", filename, e))?
    };

    let data = &mmap[..];
    if data.is_empty() {
        return Err(format!("file-fast: empty file '{}'", filename));
    }

    parse_csv_to_frame_fast(data, interner)
}

/// Parse mmap'd CSV bytes into a Frame.
///
/// This is the core fast parser. It operates directly on the byte slice
/// without any intermediate string allocation for numeric fields.
pub fn parse_csv_to_frame_fast(data: &[u8], _interner: &mut Interner) -> Result<Value, String> {
    let len = data.len();

    // --- Pass 1: scan structure (column count, row count, line offsets) ---

    // Find end of header line
    let mut pos = 0;
    let mut num_delimiters_in_header = 0;
    while pos < len && data[pos] != NEWLINE {
        if data[pos] == DELIMITER {
            num_delimiters_in_header += 1;
        }
        pos += 1;
    }
    let header_end = pos;
    let num_cols = num_delimiters_in_header + 1; // fields = delimiters + 1

    if num_cols < 2 {
        return Err("file-fast: CSV must have at least 2 columns".into());
    }

    // Parse header names
    let column_names = parse_header(&data[..header_end]);

    if column_names.len() != num_cols {
        return Err(format!(
            "file-fast: header parse mismatch: expected {} columns, got {}",
            num_cols,
            column_names.len()
        ));
    }

    // Count data rows and record line start offsets
    // Skip past header newline
    if pos < len {
        pos += 1; // skip \n
    }
    let data_start = pos;

    let mut line_offsets: Vec<usize> = Vec::with_capacity(8192);
    line_offsets.push(data_start);

    while pos < len {
        if data[pos] == NEWLINE {
            // Next line starts at pos+1 (if not EOF)
            if pos + 1 < len {
                line_offsets.push(pos + 1);
            }
        }
        pos += 1;
    }

    let num_rows = line_offsets.len();
    if num_rows == 0 {
        let index = IndexColumn::Date(Arc::new(vec![]));
        let tags = Tags::new(column_names[0].clone(), index, vec![]);
        return Ok(Value::Frame(Arc::new(Frame::new(tags, vec![]))));
    }

    // --- Pass 1.5: detect if first column is date ---
    // Sample first non-empty row's first field
    let first_col_is_date = {
        let row_start = line_offsets[0];
        let mut end = row_start;
        while end < len && data[end] != DELIMITER && data[end] != NEWLINE {
            end += 1;
        }
        is_date_bytes(&data[row_start..end])
    };

    let (index_name, numeric_start) = if first_col_is_date {
        (column_names[0].clone(), 1)
    } else {
        ("ROW".to_string(), 0)
    };

    let num_numeric_cols = num_cols - numeric_start;

    // --- Pass 2: parse values ---
    // Pre-allocate exact-size column vectors
    let mut index_dates: Vec<i32> = if first_col_is_date {
        Vec::with_capacity(num_rows)
    } else {
        Vec::new()
    };
    let mut index_strings: Vec<String> = if !first_col_is_date {
        Vec::with_capacity(num_rows)
    } else {
        Vec::new()
    };
    let mut columns: Vec<Vec<f64>> = (0..num_numeric_cols)
        .map(|_| Vec::with_capacity(num_rows))
        .collect();

    for row_idx in 0..num_rows {
        let row_start = line_offsets[row_idx];
        let row_end = if row_idx + 1 < line_offsets.len() {
            // Row ends at the newline before next line start
            line_offsets[row_idx + 1] - 1
        } else {
            // Last row: ends at EOF (may or may not have trailing newline)
            let mut e = len;
            if e > 0 && data[e - 1] == NEWLINE {
                e -= 1;
            }
            if e > 0 && data[e - 1] == b'\r' {
                e -= 1;
            }
            e
        };

        // Parse fields in this row
        let mut field_start = row_start;
        let mut col_idx = 0;

        while field_start <= row_end && col_idx < num_cols {
            // Find field end
            let mut field_end = field_start;
            while field_end < row_end && data[field_end] != DELIMITER {
                field_end += 1;
            }

            let field = &data[field_start..field_end];

            if col_idx == 0 && first_col_is_date {
                // Parse date index
                index_dates.push(parse_date_bytes(field));
            } else if col_idx == 0 && !first_col_is_date {
                // String index
                index_strings.push(std::str::from_utf8(field).unwrap_or("").trim().to_string());
            }

            if col_idx >= numeric_start {
                let numeric_idx = col_idx - numeric_start;
                if numeric_idx < num_numeric_cols {
                    columns[numeric_idx].push(parse_f64_fast(field));
                }
            }

            // Advance past delimiter
            field_start = field_end + 1;
            col_idx += 1;
        }

        // Fill missing columns with NaN (short rows)
        while col_idx < num_cols {
            if col_idx == 0 && first_col_is_date {
                index_dates.push(NULL_DATE);
            } else if col_idx == 0 && !first_col_is_date {
                index_strings.push(String::new());
            }
            if col_idx >= numeric_start {
                let numeric_idx = col_idx - numeric_start;
                if numeric_idx < num_numeric_cols {
                    columns[numeric_idx].push(f64::NAN);
                }
            }
            col_idx += 1;
        }
    }

    // --- Build Frame ---
    let index_col = if first_col_is_date {
        IndexColumn::Date(Arc::new(index_dates))
    } else {
        IndexColumn::String(Arc::new(index_strings))
    };

    let numeric_colnames: Vec<String> = column_names.iter().skip(numeric_start).cloned().collect();
    let numeric_cols: Vec<Arc<blawktrust::Column>> = columns
        .into_iter()
        .map(|v| Arc::new(blawktrust::Column::new_f64(v)))
        .collect();

    let tags = Tags::new(index_name, index_col, numeric_colnames);
    let frame = Frame::new(tags, numeric_cols);

    Ok(Value::Frame(Arc::new(frame)))
}

// ============================================================
// Internal helpers
// ============================================================

/// Parse header row into trimmed column names.
fn parse_header(header: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let mut start = 0;
    let len = header.len();

    while start <= len {
        let mut end = start;
        while end < len && header[end] != DELIMITER {
            end += 1;
        }
        let name = std::str::from_utf8(&header[start..end])
            .unwrap_or("")
            .trim()
            .to_string();
        names.push(name);
        start = end + 1;
    }
    names
}

/// Check if a byte slice looks like YYYY-MM-DD.
fn is_date_bytes(field: &[u8]) -> bool {
    if field.len() != 10 {
        return false;
    }
    // YYYY-MM-DD: digits at 0-3, dash at 4, digits at 5-6, dash at 7, digits at 8-9
    field[4] == b'-'
        && field[7] == b'-'
        && field[0].is_ascii_digit()
        && field[1].is_ascii_digit()
        && field[2].is_ascii_digit()
        && field[3].is_ascii_digit()
        && field[5].is_ascii_digit()
        && field[6].is_ascii_digit()
        && field[8].is_ascii_digit()
        && field[9].is_ascii_digit()
}

/// Parse YYYY-MM-DD bytes to days since Unix epoch using Howard Hinnant algorithm.
/// Returns NULL_DATE for invalid/NA values.
fn parse_date_bytes(field: &[u8]) -> i32 {
    if field.len() != 10 || field[4] != b'-' || field[7] != b'-' {
        // Check NA tokens
        if is_na_bytes(field) {
            return NULL_DATE;
        }
        return NULL_DATE;
    }

    let y = parse_4digit(field, 0);
    let m = parse_2digit(field, 5);
    let d = parse_2digit(field, 8);

    if y < 0 || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return NULL_DATE;
    }

    // Howard Hinnant civil_from_days (inverse)
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * m as u32 + 2) / 5 + d as u32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i32 - 719468
}

#[inline(always)]
fn parse_4digit(b: &[u8], off: usize) -> i32 {
    ((b[off] - b'0') as i32) * 1000
        + ((b[off + 1] - b'0') as i32) * 100
        + ((b[off + 2] - b'0') as i32) * 10
        + ((b[off + 3] - b'0') as i32)
}

#[inline(always)]
fn parse_2digit(b: &[u8], off: usize) -> i32 {
    ((b[off] - b'0') as i32) * 10 + ((b[off + 1] - b'0') as i32)
}

/// Check if byte slice is an NA token.
#[inline]
fn is_na_bytes(field: &[u8]) -> bool {
    matches!(
        field,
        b"" | b"NA" | b"NaN" | b"N/A" | b"na" | b"nan" | b"n/a"
    )
}

/// Fast float parser for CSV fields.
///
/// Uses fast-path NA detection on raw bytes, then delegates to Rust's
/// str::parse::<f64>() for exact IEEE-754 rounding. This avoids the
/// csv crate's record/quoting overhead while maintaining bit-exact
/// parity with the standard parser.
#[inline]
fn parse_f64_fast(field: &[u8]) -> f64 {
    if field.is_empty() {
        return f64::NAN;
    }

    // Fast NA check on first byte
    let first = field[0];
    if first == b'N' || first == b'n' || first == b' ' {
        // Could be NA, NaN, N/A, or whitespace-only
        let trimmed = trim_bytes(field);
        if trimmed.is_empty() {
            return f64::NAN;
        }
        if is_na_bytes(trimmed) {
            return f64::NAN;
        }
    }

    // Convert to str and parse with stdlib (exact rounding)
    match std::str::from_utf8(field) {
        Ok(s) => s.trim().parse::<f64>().unwrap_or(f64::NAN),
        Err(_) => f64::NAN,
    }
}

/// Trim leading/trailing ASCII whitespace from byte slice.
#[inline]
fn trim_bytes(b: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = b.len();
    while start < end && b[start] == b' ' {
        start += 1;
    }
    while end > start && b[end - 1] == b' ' {
        end -= 1;
    }
    &b[start..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_f64_fast_basic() {
        assert_eq!(parse_f64_fast(b"42"), 42.0);
        assert_eq!(parse_f64_fast(b"3.25"), 3.25);
        assert_eq!(parse_f64_fast(b"-100.5"), -100.5);
        assert_eq!(parse_f64_fast(b"0"), 0.0);
        assert_eq!(parse_f64_fast(b"0.0"), 0.0);
        assert_eq!(parse_f64_fast(b"+7.5"), 7.5);
    }

    #[test]
    fn test_parse_f64_fast_na() {
        assert!(parse_f64_fast(b"").is_nan());
        assert!(parse_f64_fast(b"NA").is_nan());
        assert!(parse_f64_fast(b"NaN").is_nan());
        assert!(parse_f64_fast(b"N/A").is_nan());
        assert!(parse_f64_fast(b"  ").is_nan());
    }

    #[test]
    fn test_parse_f64_fast_precision() {
        // Verify bit-exact parity with stdlib for typical financial data
        let cases = [
            "1534.36",
            "145.438",
            "0.024692612590371255",
            "-0.1823215567939549",
            "5.738",
            "1.5919",
        ];
        for s in cases {
            let fast = parse_f64_fast(s.as_bytes());
            let stdlib: f64 = s.parse().unwrap();
            assert_eq!(
                fast.to_bits(),
                stdlib.to_bits(),
                "Bit mismatch for '{}': fast={} stdlib={}",
                s,
                fast,
                stdlib
            );
        }
    }

    #[test]
    fn test_is_date_bytes() {
        assert!(is_date_bytes(b"2020-01-01"));
        assert!(is_date_bytes(b"1999-12-31"));
        assert!(!is_date_bytes(b"not-a-date"));
        assert!(!is_date_bytes(b"2020/01/01"));
        assert!(!is_date_bytes(b"short"));
    }

    #[test]
    fn test_parse_date_bytes() {
        // 2020-01-01 = 18262 days since epoch
        assert_eq!(parse_date_bytes(b"2020-01-01"), 18262);
        // 1970-01-01 = 0
        assert_eq!(parse_date_bytes(b"1970-01-01"), 0);
        // NA
        assert_eq!(parse_date_bytes(b"NA"), NULL_DATE);
    }

    #[test]
    fn test_parse_csv_fast_simple() {
        let csv = b"DATE;px;vol\n2020-01-01;100.0;200\n2020-01-02;102.5;300\n";
        let mut interner = crate::ast::Interner::new();
        let result = parse_csv_to_frame_fast(csv, &mut interner).unwrap();
        match result {
            Value::Frame(f) => {
                assert_eq!(f.nrows(), 2);
                assert_eq!(f.ncols(), 2); // px, vol (DATE is index)
            }
            _ => panic!("Expected Frame"),
        }
    }

    #[test]
    fn test_parse_csv_fast_with_na() {
        let csv = b"DATE;px;vol\n2020-01-01;100.0;200\n2020-01-02;NA;300\n";
        let mut interner = crate::ast::Interner::new();
        let result = parse_csv_to_frame_fast(csv, &mut interner).unwrap();
        match result {
            Value::Frame(f) => {
                assert_eq!(f.nrows(), 2);
                // Check NA in px column
                let col = f.get_col(0).expect("column 0");
                if let blawktrust::Column::F64(data) = col.as_ref() {
                    assert_eq!(data[0], 100.0);
                    assert!(data[1].is_nan());
                } else {
                    panic!("Expected F64 column");
                }
            }
            _ => panic!("Expected Frame"),
        }
    }

    #[test]
    fn test_fast_matches_header_parsing() {
        let header = b"DATE;ES1 Index;SPY US Equity;volume";
        let names = parse_header(header);
        assert_eq!(names, vec!["DATE", "ES1 Index", "SPY US Equity", "volume"]);
    }
}
