//! I/O operations for blisp
//!
//! Handles CSV file loading, stdin reading, and CSV writing.

use crate::value::{Value, Table};
use crate::ast::{Interner, SymbolId};
use std::sync::Arc;
use std::io::{self, Read};

// Import null sentinels from blawktrust (kdb-style null date/timestamp)
use blawktrust::{NULL_DATE, NULL_TIMESTAMP};

/// Number of rows to sample for type detection (bounded lookahead)
const TYPE_DETECTION_ROWS: usize = 8;

/// Load CSV file into a Table
///
/// Format: First row is headers (column names), rest are data rows.
/// All columns are assumed to be F64 for now.
///
/// Example CSV:
/// ```csv
/// date,px,vol
/// 2020-01-01,100.0,1000
/// 2020-01-02,102.0,1200
/// ```
pub fn load_csv(filename: &str, interner: &mut Interner) -> Result<Value, String> {
    parse_csv_from_file(filename, interner, None)
}

/// Load CSV file with row limit (preview mode)
///
/// Only parses header + first `row_limit` rows for fast display/pipelines.
pub fn load_csv_limit(filename: &str, interner: &mut Interner, row_limit: usize) -> Result<Value, String> {
    parse_csv_from_file(filename, interner, Some(row_limit))
}

/// Load CSV file with only selected columns (column subsetting)
///
/// Parses only requested columns by name. Dramatically faster for wide CSVs.
/// Returns Table with columns in the order requested.
/// Error if column name not found (lists available headers).
pub fn load_csv_cols(filename: &str, col_names: &[String], interner: &mut Interner) -> Result<Value, String> {
    let file = std::fs::File::open(filename)
        .map_err(|e| format!("Error opening file '{}': {}", filename, e))?;

    let mut csv_reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(b';')
        .from_reader(file);

    // Read headers
    let headers = csv_reader.headers()
        .map_err(|e| format!("Error reading CSV headers: {}", e))?;

    let all_column_names: Vec<String> = headers.iter()
        .map(|s| s.trim().to_string())
        .collect();

    // Map requested column names to indices
    let mut selected_indices: Vec<usize> = Vec::new();
    for col_name in col_names {
        match all_column_names.iter().position(|h| h == col_name) {
            Some(idx) => selected_indices.push(idx),
            None => {
                // Column not found - provide helpful error with available columns
                let available = all_column_names.join("\", \"");
                return Err(format!(
                    "file-cols: column '{}' not found. Available: \"{}\"",
                    col_name, available
                ));
            }
        }
    }

    let num_selected = selected_indices.len();

    // Sample first TYPE_DETECTION_ROWS rows to detect types of selected columns
    let mut sample_rows: Vec<csv::ByteRecord> = Vec::new();
    for result in csv_reader.byte_records().take(TYPE_DETECTION_ROWS) {
        sample_rows.push(result.map_err(|e| format!("Error reading CSV record: {}", e))?);
    }

    if sample_rows.is_empty() {
        // Empty CSV - return empty table with selected columns
        return Ok(Value::Table(Arc::new(Table::new())));
    }

    // Detect types for selected columns only
    let col_types: Vec<ColType> = selected_indices.iter()
        .map(|&col_idx| {
            // Look for any non-NA value that indicates type
            for row in &sample_rows {
                if col_idx < row.len() {
                    let field = row.get(col_idx).unwrap();
                    let field_str = std::str::from_utf8(field).unwrap_or("").trim();
                    // Skip NA tokens - they don't tell us the type
                    if !is_na_token(field_str) {
                        return detect_column_type(field_str);
                    }
                }
            }
            // All values were NA/empty - default to F64
            ColType::F64
        })
        .collect();

    // Initialize column data vectors
    let mut f64_columns: Vec<Vec<f64>> = (0..num_selected)
        .map(|_| Vec::with_capacity(256))
        .collect();
    let mut date_columns: Vec<Vec<i32>> = (0..num_selected)
        .map(|_| Vec::with_capacity(256))
        .collect();
    let mut timestamp_columns: Vec<Vec<i64>> = (0..num_selected)
        .map(|_| Vec::with_capacity(256))
        .collect();

    // Process sample rows (already collected)
    for record in &sample_rows {
        for (i, &col_idx) in selected_indices.iter().enumerate() {
            if col_idx < record.len() {
                let field = record.get(col_idx).unwrap();
                let field_str = std::str::from_utf8(field).unwrap_or("").trim();
                match col_types[i] {
                    ColType::F64 => {
                        f64_columns[i].push(parse_numeric_with_na(field_str));
                    }
                    ColType::Date => {
                        date_columns[i].push(parse_date_or_null(field_str));
                    }
                    ColType::Timestamp => {
                        timestamp_columns[i].push(parse_timestamp_or_null(field_str));
                    }
                }
            } else {
                // Missing field - treat as NA
                match col_types[i] {
                    ColType::F64 => f64_columns[i].push(f64::NAN),
                    ColType::Date => date_columns[i].push(NULL_DATE),
                    ColType::Timestamp => timestamp_columns[i].push(NULL_TIMESTAMP),
                }
            }
        }
    }

    // Process remaining rows
    for result in csv_reader.byte_records() {
        let record = result.map_err(|e| format!("Error reading CSV record: {}", e))?;
        for (i, &col_idx) in selected_indices.iter().enumerate() {
            if col_idx < record.len() {
                let field = record.get(col_idx).unwrap();
                let field_str = std::str::from_utf8(field).unwrap_or("").trim();
                match col_types[i] {
                    ColType::F64 => {
                        f64_columns[i].push(parse_numeric_with_na(field_str));
                    }
                    ColType::Date => {
                        date_columns[i].push(parse_date_or_null(field_str));
                    }
                    ColType::Timestamp => {
                        timestamp_columns[i].push(parse_timestamp_or_null(field_str));
                    }
                }
            } else {
                // Missing field - treat as NA
                match col_types[i] {
                    ColType::F64 => f64_columns[i].push(f64::NAN),
                    ColType::Date => date_columns[i].push(NULL_DATE),
                    ColType::Timestamp => timestamp_columns[i].push(NULL_TIMESTAMP),
                }
            }
        }
    }

    // Build table with selected columns in requested order
    let mut table = Table::new();
    for (i, col_name) in col_names.iter().enumerate() {
        let sym_id = interner.intern(col_name);
        let column = match col_types[i] {
            ColType::F64 => blawktrust::Column::new_f64(f64_columns[i].clone()),
            ColType::Date => blawktrust::Column::new_date(date_columns[i].clone()),
            ColType::Timestamp => blawktrust::Column::new_timestamp(timestamp_columns[i].clone()),
        };
        table.add_column(sym_id, column);
    }

    Ok(Value::Table(Arc::new(table)))
}

/// Read CSV from stdin into a Table
pub fn load_stdin(interner: &mut Interner) -> Result<Value, String> {
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .map_err(|e| format!("Error reading stdin: {}", e))?;

    parse_csv(&buffer, interner, None)
}

/// Parse CSV from file path with optional row limit (streaming, no full read)
///
/// Streams directly from file without reading entire contents into memory.
/// If row_limit is Some(n), only parse header + first n data rows.
fn parse_csv_from_file(filename: &str, interner: &mut Interner, row_limit: Option<usize>) -> Result<Value, String> {
    let file = std::fs::File::open(filename)
        .map_err(|e| format!("Error opening file '{}': {}", filename, e))?;

    parse_csv_from_reader(file, interner, row_limit)
}

/// Parse CSV from a reader with optional row limit
///
/// Generic function that works with any Read source (file, string, stdin, etc.)
fn parse_csv_from_reader<R: std::io::Read>(reader: R, interner: &mut Interner, row_limit: Option<usize>) -> Result<Value, String> {
    let mut csv_reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(b';')
        .from_reader(reader);

    parse_csv_from_csv_reader(&mut csv_reader, interner, row_limit)
}

/// Parse CSV content from string into a Table with optional row limit
///
/// If row_limit is Some(n), only parse header + first n data rows.
/// This is the "preview parser" fast path for display/pipelines.
fn parse_csv(content: &str, interner: &mut Interner, row_limit: Option<usize>) -> Result<Value, String> {
    parse_csv_from_reader(content.as_bytes(), interner, row_limit)
}

/// Core CSV parsing logic that works with any csv::Reader
///
/// This is where the actual parsing happens, shared by all input sources.
fn parse_csv_from_csv_reader<R: std::io::Read>(
    reader: &mut csv::Reader<R>,
    interner: &mut Interner,
    row_limit: Option<usize>
) -> Result<Value, String> {
    // Read headers (TASK C: trim whitespace)
    let headers = reader.headers()
        .map_err(|e| format!("Error reading CSV headers: {}", e))?;

    let column_names: Vec<String> = headers.iter()
        .map(|s| s.trim().to_string())  // Trim header names
        .collect();

    let num_cols = column_names.len();

    // Detect column types by sampling first K rows (robust type inference)
    // Collect first TYPE_DETECTION_ROWS rows for sampling (capped by row_limit if specified)
    let sample_size = row_limit.map(|lim| lim.min(TYPE_DETECTION_ROWS)).unwrap_or(TYPE_DETECTION_ROWS);
    let mut sample_rows: Vec<csv::StringRecord> = Vec::new();
    for result in reader.records().take(sample_size) {
        sample_rows.push(result.map_err(|e| format!("Error reading CSV record: {}", e))?);
    }

    if sample_rows.is_empty() {
        // Empty CSV
        return Ok(Value::Table(Arc::new(Table::new())));
    }

    // Detect types by checking first K rows, skipping NA/empty values
    let col_types: Vec<ColType> = (0..num_cols)
        .map(|col_idx| {
            // Look for any non-NA value that indicates type
            for row in &sample_rows {
                if col_idx < row.len() {
                    let field = row.get(col_idx).unwrap().trim();
                    // Skip NA tokens - they don't tell us the type
                    if !is_na_token(field) {
                        return detect_column_type(field);
                    }
                }
            }
            // All values were NA/empty - default to F64
            ColType::F64
        })
        .collect();

    // Initialize column data vectors with capacity based on row_limit (preview mode optimization)
    let initial_capacity = row_limit.unwrap_or(256); // If no limit, use reasonable default
    let mut f64_columns: Vec<Vec<f64>> = (0..num_cols)
        .map(|_| Vec::with_capacity(initial_capacity))
        .collect();
    let mut date_columns: Vec<Vec<i32>> = (0..num_cols)
        .map(|_| Vec::with_capacity(initial_capacity))
        .collect();
    let mut timestamp_columns: Vec<Vec<i64>> = (0..num_cols)
        .map(|_| Vec::with_capacity(initial_capacity))
        .collect();

    // Process sample rows
    let mut rows_parsed = 0;
    for record in sample_rows {
        for (i, field) in record.iter().enumerate() {
            match col_types[i] {
                ColType::F64 => {
                    f64_columns[i].push(parse_numeric_with_na(field.trim()));
                }
                ColType::Date => {
                    date_columns[i].push(parse_date_or_null(field.trim()));
                }
                ColType::Timestamp => {
                    timestamp_columns[i].push(parse_timestamp_or_null(field.trim()));
                }
            }
        }
        rows_parsed += 1;
    }

    // Read remaining data rows after the sample (respecting row_limit)
    for result in reader.records() {
        // Stop if we've hit the row limit
        if let Some(limit) = row_limit {
            if rows_parsed >= limit {
                break;
            }
        }
        let record = result.map_err(|e| format!("Error reading CSV record: {}", e))?;

        if record.len() != num_cols {
            return Err(format!(
                "CSV row has {} columns, expected {}",
                record.len(),
                num_cols
            ));
        }

        for (i, field) in record.iter().enumerate() {
            match col_types[i] {
                ColType::F64 => {
                    // Handle NA tokens
                    f64_columns[i].push(parse_numeric_with_na(field.trim()));
                }
                ColType::Date => {
                    // Parse dates (NA tokens → NULL_DATE)
                    date_columns[i].push(parse_date_or_null(field.trim()));
                }
                ColType::Timestamp => {
                    // Parse timestamps (NA tokens → NULL_TIMESTAMP)
                    timestamp_columns[i].push(parse_timestamp_or_null(field.trim()));
                }
            }
        }
        rows_parsed += 1;
    }

    // Build Table
    let mut table = Table::new();

    for (i, name) in column_names.iter().enumerate() {
        let sym = interner.intern(name);
        let col = match col_types[i] {
            ColType::F64 => blawktrust::Column::new_f64(f64_columns[i].clone()),
            ColType::Date => blawktrust::Column::new_date(date_columns[i].clone()),
            ColType::Timestamp => blawktrust::Column::new_timestamp(timestamp_columns[i].clone()),
        };
        table.add_column(sym, col);
    }

    Ok(Value::Table(Arc::new(table)))
}

/// Column type detected from CSV
#[derive(Debug, Clone, Copy)]
enum ColType {
    F64,        // Numeric column
    Date,       // Date column (YYYY-MM-DD, stored as i32 days)
    Timestamp,  // Timestamp column (YYYY-MM-DD HH:MM:SS[.nanos], stored as i64 nanoseconds)
}

/// Detect column type from a sample value
///
/// Priority: Timestamp > Date > F64
fn detect_column_type(sample: &str) -> ColType {
    let trimmed = sample.trim();

    // Check if it looks like a timestamp first (more specific)
    if is_timestamp_format(trimmed) {
        ColType::Timestamp
    } else if is_date_format(trimmed) {
        ColType::Date
    } else {
        ColType::F64
    }
}

/// Check if token is an NA marker
fn is_na_token(s: &str) -> bool {
    matches!(s, "" | "NA" | "NaN" | "N/A")
}

/// Check if string matches YYYY-MM-DD format
fn is_date_format(s: &str) -> bool {
    if s.len() != 10 {
        return false;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    // Check format: YYYY-MM-DD
    parts[0].len() == 4 && parts[1].len() == 2 && parts[2].len() == 2
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && parts[2].chars().all(|c| c.is_ascii_digit())
}

/// Check if string matches YYYY-MM-DD HH:MM:SS[.fffffffff] format
fn is_timestamp_format(s: &str) -> bool {
    if s.len() < 19 {
        return false;
    }

    let space_pos = match s.find(' ') {
        Some(pos) if pos == 10 => pos,
        _ => return false,
    };

    let date_part = &s[..10];
    if !is_date_format(date_part) {
        return false;
    }

    let time_part = &s[11..];
    let base_len = if let Some(dot_pos) = time_part.find('.') {
        if dot_pos != 8 {
            return false;
        }
        let frac = &time_part[9..];
        if frac.is_empty() || frac.len() > 9 {
            return false;
        }
        if !frac.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
        8
    } else {
        if time_part.len() != 8 {
            return false;
        }
        8
    };

    let time_base = &time_part[..base_len];
    if time_base.len() != 8 {
        return false;
    }
    if &time_base[2..3] != ":" || &time_base[5..6] != ":" {
        return false;
    }

    let parts: Vec<&str> = time_base.split(':').collect();
    parts.len() == 3 && parts.iter().all(|p| {
        p.len() == 2 && p.chars().all(|c| c.is_ascii_digit())
    })
}

/// Parse numeric value, treating NA tokens as NaN
///
/// TASK A: Recognize {"NA", "NaN", "N/A", ""} as missing data
fn parse_numeric_with_na(s: &str) -> f64 {
    match s {
        "" | "NA" | "NaN" | "N/A" => f64::NAN,
        _ => s.parse::<f64>().unwrap_or(f64::NAN),
    }
}

/// Parse date or return NULL_DATE sentinel for missing dates (kdb-style)
///
/// Handles NA tokens and invalid formats by returning NULL_DATE.
fn parse_date_or_null(s: &str) -> i32 {
    // NA tokens → NULL_DATE
    if is_na_token(s) {
        return NULL_DATE;
    }

    // Try parsing as date
    parse_date_to_days(s).unwrap_or(NULL_DATE)
}

/// Parse YYYY-MM-DD to days since epoch using Howard Hinnant algorithm
///
/// This correctly handles all leap years including Feb 29.
/// Algorithm: https://howardhinnant.github.io/date_algorithms.html
///
/// Returns Ok(days) or Err for invalid format.
/// For CSV loading, use parse_date_or_null instead (returns NULL_DATE on error).
fn parse_date_to_days(s: &str) -> Result<i32, String> {
    // Validate format first
    if !is_date_format(s) {
        return Err(format!("Invalid date format '{}', expected YYYY-MM-DD", s));
    }

    let parts: Vec<&str> = s.split('-').collect();
    let year: i32 = parts[0].parse()
        .map_err(|_| format!("Invalid year in date '{}'", s))?;
    let month: u32 = parts[1].parse()
        .map_err(|_| format!("Invalid month in date '{}'", s))?;
    let day: u32 = parts[2].parse()
        .map_err(|_| format!("Invalid day in date '{}'", s))?;

    // Validate ranges
    if !(1..=12).contains(&month) {
        return Err(format!("Month {} out of range 1-12", month));
    }
    if !(1..=31).contains(&day) {
        return Err(format!("Day {} out of range 1-31", day));
    }

    // Howard Hinnant's civil_from_days algorithm
    // Shift month so March is month 0 (makes leap day last day of year)
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32; // year of era
    let m = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * m + 2) / 5 + day - 1; // day of year
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // day of era

    Ok(era * 146097 + doe as i32 - 719468)
}

/// Convert days since epoch to YYYY-MM-DD (inverse of parse_date_to_days)
///
/// Uses Howard Hinnant's inverse civil_to_days algorithm.
fn format_date_from_days(days: i32) -> String {
    if days == NULL_DATE {
        return "NA".to_string();
    }

    // Howard Hinnant's inverse algorithm
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32; // day of era
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era
    let y = yoe as i32 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year
    let mp = (5 * doy + 2) / 153; // month' (0=Mar, 1=Apr, ..., 9=Dec, 10=Jan, 11=Feb)
    let d = doy - (153 * mp + 2) / 5 + 1; // day
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month (1=Jan, 12=Dec)
    let year = if m <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02}", year, m, d)
}

/// Parse timestamp or return NULL_TIMESTAMP sentinel for missing timestamps (kdb-style)
///
/// Handles NA tokens and invalid formats by returning NULL_TIMESTAMP.
fn parse_timestamp_or_null(s: &str) -> i64 {
    // NA tokens → NULL_TIMESTAMP
    if is_na_token(s) {
        return NULL_TIMESTAMP;
    }

    // Try parsing as timestamp
    parse_timestamp_to_nanos(s).unwrap_or(NULL_TIMESTAMP)
}

/// Parse YYYY-MM-DD HH:MM:SS[.fffffffff] to nanoseconds since epoch
///
/// Supports optional fractional seconds with 1-9 digits of precision.
/// Fractional seconds are scaled to nanoseconds (right-padded with zeros).
///
/// Examples:
/// - "2000-01-01 12:30:45" → no fractional seconds
/// - "2000-01-01 12:30:45.1" → 100,000,000 nanoseconds (0.1 seconds)
/// - "2000-01-01 12:30:45.123" → 123,000,000 nanoseconds (0.123 seconds)
/// - "2000-01-01 12:30:45.123456789" → 123,456,789 nanoseconds
fn parse_timestamp_to_nanos(s: &str) -> Result<i64, String> {
    if !is_timestamp_format(s) {
        return Err(format!("Invalid timestamp format '{}'", s));
    }

    let parts: Vec<&str> = s.splitn(2, ' ').collect();
    let days = parse_date_to_days(parts[0])? as i64;
    let time_part = parts[1];

    let (time_base, frac_nanos) = if let Some(dot_pos) = time_part.find('.') {
        let frac_str = &time_part[dot_pos + 1..];
        let frac_value: u64 = frac_str.parse()
            .map_err(|_| "Invalid fractional seconds")?;
        // Scale to nanoseconds by multiplying by 10^(9-len)
        let scale = 10u64.pow(9 - frac_str.len() as u32);
        (time_part[..dot_pos].to_string(), (frac_value * scale) as i64)
    } else {
        (time_part.to_string(), 0)
    };

    let time_parts: Vec<&str> = time_base.split(':').collect();
    let hour: i64 = time_parts[0].parse()
        .map_err(|_| "Invalid hour")?;
    let minute: i64 = time_parts[1].parse()
        .map_err(|_| "Invalid minute")?;
    let second: i64 = time_parts[2].parse()
        .map_err(|_| "Invalid second")?;

    if hour > 23 {
        return Err(format!("Hour {} out of range 0-23", hour));
    }
    if minute > 59 {
        return Err(format!("Minute {} out of range 0-59", minute));
    }
    if second > 59 {
        return Err(format!("Second {} out of range 0-59", second));
    }

    let nanos = days * 86400 * 1_000_000_000
        + hour * 3600 * 1_000_000_000
        + minute * 60 * 1_000_000_000
        + second * 1_000_000_000
        + frac_nanos;

    Ok(nanos)
}

/// Format nanoseconds as YYYY-MM-DD HH:MM:SS[.fffffffff]
///
/// Fractional seconds are included only when non-zero, trailing zeros removed.
fn format_timestamp_from_nanos(nanos: i64) -> String {
    if nanos == NULL_TIMESTAMP {
        return "NA".to_string();
    }

    let total_seconds = nanos / 1_000_000_000;
    let frac_nanos = nanos % 1_000_000_000;

    let days = total_seconds / 86400;
    let remaining_secs = total_seconds % 86400;

    let hour = remaining_secs / 3600;
    let minute = (remaining_secs % 3600) / 60;
    let second = remaining_secs % 60;

    let date_str = format_date_from_days(days as i32);

    if frac_nanos == 0 {
        format!("{} {:02}:{:02}:{:02}", date_str, hour, minute, second)
    } else {
        let frac_str = format!("{:09}", frac_nanos);
        let trimmed = frac_str.trim_end_matches('0');
        format!("{} {:02}:{:02}:{:02}.{}", date_str, hour, minute, second, trimmed)
    }
}

/// Save Table to CSV file
///
/// Format: First row is headers, rest are data rows.
/// Columns are written in the order they were added to the table.
pub fn save_csv(filename: &str, table: &Table, interner: &Interner) -> Result<(), String> {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b';')
        .from_path(filename)
        .map_err(|e| format!("Error creating CSV file '{}': {}", filename, e))?;

    if table.columns.is_empty() {
        return Err("Cannot save empty table".to_string());
    }

    // Write headers
    let headers: Vec<String> = table.columns.iter()
        .map(|(sym_id, _)| interner.resolve(*sym_id).to_string())
        .collect();

    writer.write_record(&headers)
        .map_err(|e| format!("Error writing CSV headers: {}", e))?;

    // Write data rows
    let row_count = table.row_count;

    for row_idx in 0..row_count {
        let mut row: Vec<String> = Vec::new();

        for (_, col) in &table.columns {
            match col {
                blawktrust::Column::F64(data) => {
                    if row_idx < data.len() {
                        let val = data[row_idx];
                        if val.is_nan() {
                            row.push("NaN".to_string());
                        } else {
                            row.push(format!("{}", val));
                        }
                    } else {
                        row.push("NaN".to_string());
                    }
                }
                blawktrust::Column::Date(data) => {
                    if row_idx < data.len() {
                        let days = data[row_idx];
                        if days == NULL_DATE {
                            row.push("NA".to_string());
                        } else {
                            row.push(format_date_from_days(days));
                        }
                    } else {
                        row.push("NA".to_string());
                    }
                }
                blawktrust::Column::Timestamp(data) => {
                    if row_idx < data.len() {
                        let nanos = data[row_idx];
                        if nanos == NULL_TIMESTAMP {
                            row.push("NA".to_string());
                        } else {
                            row.push(format_timestamp_from_nanos(nanos));
                        }
                    } else {
                        row.push("NA".to_string());
                    }
                }
            }
        }

        writer.write_record(&row)
            .map_err(|e| format!("Error writing CSV row {}: {}", row_idx, e))?;
    }

    writer.flush()
        .map_err(|e| format!("Error flushing CSV file: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_csv_simple() {
        let mut interner = Interner::new();
        // Use only numeric data for now
        let csv = "px;vol\n100.0;1000\n102.0;1200";

        let result = parse_csv(csv, &mut interner, None).unwrap();

        if let Value::Table(table) = result {
            assert_eq!(table.row_count, 2);
            assert_eq!(table.columns.len(), 2);

            // Check column names
            let names: Vec<String> = table.columns.iter()
                .map(|(sym, _)| interner.resolve(*sym).to_string())
                .collect();
            assert_eq!(names, vec!["px", "vol"]);
        } else {
            panic!("Expected Table");
        }
    }

    // TASK A: Test NA handling
    #[test]
    fn test_parse_csv_with_na() {
        let mut interner = Interner::new();
        let csv = "px;vol\n100.0;1000\nNA;1200\n102.0;NA\nNaN;N/A";

        let result = parse_csv(csv, &mut interner, None).unwrap();

        if let Value::Table(table) = result {
            assert_eq!(table.row_count, 4);

            // Check that NA values became NaN
            let px_col = table.get_column(interner.intern("px")).unwrap();
            match px_col {
                blawktrust::Column::F64(data) => {
                    assert_eq!(data[0], 100.0);
                    assert!(data[1].is_nan(), "NA should become NaN");
                    assert_eq!(data[2], 102.0);
                    assert!(data[3].is_nan(), "NaN should stay NaN");
                }
                _ => panic!("Expected F64 column"),
            }

            let vol_col = table.get_column(interner.intern("vol")).unwrap();
            match vol_col {
                blawktrust::Column::F64(data) => {
                    assert_eq!(data[0], 1000.0);
                    assert_eq!(data[1], 1200.0);
                    assert!(data[2].is_nan(), "NA should become NaN");
                    assert!(data[3].is_nan(), "N/A should become NaN");
                }
                _ => panic!("Expected F64 column"),
            }
        } else {
            panic!("Expected Table");
        }
    }

    // TASK B: Test date column support with NULL_DATE sentinel
    #[test]
    fn test_parse_csv_with_dates() {
        let mut interner = Interner::new();
        let csv = "date;px;vol\n2000-01-03;100.0;1000\n2000-01-10;102.0;1200\nNA;105.0;1300";

        let result = parse_csv(csv, &mut interner, None).unwrap();

        if let Value::Table(table) = result {
            assert_eq!(table.row_count, 3);
            assert_eq!(table.columns.len(), 3);

            // Check date column is Date type
            let date_col = table.get_column(interner.intern("date")).unwrap();
            match date_col {
                blawktrust::Column::Date(data) => {
                    assert_eq!(data.len(), 3);
                    // Valid dates should be positive days since epoch
                    assert!(data[0] > 0 && data[0] != NULL_DATE, "Date should be positive days");
                    assert!(data[1] > data[0] && data[1] != NULL_DATE, "Later date should have more days");
                    // NA date should be NULL_DATE
                    assert_eq!(data[2], NULL_DATE, "NA date should be NULL_DATE sentinel");
                }
                _ => panic!("Date column should be Date type"),
            }

            // Check numeric columns still work
            let px_col = table.get_column(interner.intern("px")).unwrap();
            match px_col {
                blawktrust::Column::F64(data) => {
                    assert_eq!(data[0], 100.0);
                    assert_eq!(data[1], 102.0);
                    assert_eq!(data[2], 105.0);
                }
                _ => panic!("Expected F64 column"),
            }
        } else {
            panic!("Expected Table");
        }
    }

    // TASK C: Test header trimming
    #[test]
    fn test_parse_csv_header_trimming() {
        let mut interner = Interner::new();
        // Headers with trailing spaces (Bloomberg style)
        let csv = "ES2 Index ;SPY US Equity \n100.0;145.0\n102.0;146.0";

        let result = parse_csv(csv, &mut interner, None).unwrap();

        if let Value::Table(table) = result {
            // Check that headers were trimmed
            let names: Vec<String> = table.columns.iter()
                .map(|(sym, _)| interner.resolve(*sym).to_string())
                .collect();

            assert_eq!(names[0], "ES2 Index", "Trailing space should be trimmed");
            assert_eq!(names[1], "SPY US Equity", "Trailing space should be trimmed");

            // Verify we can access by trimmed name
            assert!(table.get_column(interner.intern("ES2 Index")).is_some());
            assert!(table.get_column(interner.intern("SPY US Equity")).is_some());
        } else {
            panic!("Expected Table");
        }
    }

    // TASK D: Test string-based column lookup + robust type detection
    #[test]
    fn test_bloomberg_style_csv() {
        let mut interner = Interner::new();
        // Full Bloomberg-style CSV with NA in various positions
        let csv = "date;ES1 Index;SPY US Equity\n2000-01-03;1534.36;145.438\n2000-01-10;1542.98;NA\nNA;1550.00;147.500";

        let result = parse_csv(csv, &mut interner, None).unwrap();

        if let Value::Table(table) = result {
            assert_eq!(table.row_count, 3);
            assert_eq!(table.columns.len(), 3);

            // Check date column (with NA → NULL_DATE)
            let date_col = table.get_column(interner.intern("date")).unwrap();
            match date_col {
                blawktrust::Column::Date(data) => {
                    assert!(data[0] != NULL_DATE, "Valid date");
                    assert!(data[1] != NULL_DATE, "Valid date");
                    assert_eq!(data[2], NULL_DATE, "NA date should be NULL_DATE");
                }
                _ => panic!("Expected Date column"),
            }

            // Check numeric column with space in name
            let spy_col = table.get_column(interner.intern("SPY US Equity")).unwrap();
            match spy_col {
                blawktrust::Column::F64(data) => {
                    assert_eq!(data[0], 145.438);
                    assert!(data[1].is_nan(), "NA should become NaN");
                    assert_eq!(data[2], 147.500);
                }
                _ => panic!("Expected F64 column"),
            }
        } else {
            panic!("Expected Table");
        }
    }

    // Test robust type detection (K-row lookahead)
    #[test]
    fn test_type_detection_with_leading_na() {
        let mut interner = Interner::new();
        // Date column starts with NA - should still detect as Ts
        let csv = "date;value\nNA;100\nNA;200\n2000-01-03;300\n2000-01-10;400";

        let result = parse_csv(csv, &mut interner, None).unwrap();

        if let Value::Table(table) = result {
            assert_eq!(table.row_count, 4);

            // Should detect as Date despite leading NAs (K-row lookahead)
            let date_col = table.get_column(interner.intern("date")).unwrap();
            match date_col {
                blawktrust::Column::Date(data) => {
                    assert_eq!(data[0], NULL_DATE, "First NA");
                    assert_eq!(data[1], NULL_DATE, "Second NA");
                    assert!(data[2] != NULL_DATE, "Valid date");
                    assert!(data[3] != NULL_DATE, "Valid date");
                }
                _ => panic!("Should detect as Date despite leading NAs"),
            }
        } else {
            panic!("Expected Table");
        }
    }

    #[test]
    fn test_is_date_format() {
        assert!(is_date_format("2000-01-03"));
        assert!(is_date_format("2025-12-31"));
        assert!(!is_date_format("100.0"));
        assert!(!is_date_format("NA"));
        assert!(!is_date_format("2000-1-3")); // Wrong format
        assert!(!is_date_format("20-01-03")); // Wrong format
    }

    #[test]
    fn test_parse_numeric_with_na() {
        assert_eq!(parse_numeric_with_na("100.0"), 100.0);
        assert!(parse_numeric_with_na("NA").is_nan());
        assert!(parse_numeric_with_na("NaN").is_nan());
        assert!(parse_numeric_with_na("N/A").is_nan());
        assert!(parse_numeric_with_na("").is_nan());
        assert!(parse_numeric_with_na("invalid").is_nan());
    }

    #[test]
    fn test_parse_date_to_days() {
        // 1970-01-01 is day 0
        assert_eq!(parse_date_to_days("1970-01-01").unwrap(), 0);

        // 2000-01-01 should be ~30 years later
        let days_2000 = parse_date_to_days("2000-01-01").unwrap();
        assert!(days_2000 > 10000 && days_2000 < 11000);

        // Invalid formats return error
        assert!(parse_date_to_days("2000-1-1").is_err());
        assert!(parse_date_to_days("not-a-date").is_err());
    }

    #[test]
    fn test_parse_date_or_null() {
        // Valid dates
        assert_eq!(parse_date_or_null("1970-01-01"), 0);
        assert!(parse_date_or_null("2000-01-01") > 0);

        // NA tokens → NULL_DATE
        assert_eq!(parse_date_or_null("NA"), NULL_DATE);
        assert_eq!(parse_date_or_null("NaN"), NULL_DATE);
        assert_eq!(parse_date_or_null(""), NULL_DATE);

        // Invalid format → NULL_DATE
        assert_eq!(parse_date_or_null("invalid"), NULL_DATE);
        assert_eq!(parse_date_or_null("2000-1-1"), NULL_DATE);
    }

    #[test]
    fn test_save_and_load_csv() {
        let mut interner = Interner::new();

        // Create a table
        let mut table = Table::new();
        let px_sym = interner.intern("px");
        let vol_sym = interner.intern("vol");

        let px_col = blawktrust::Column::new_f64(vec![100.0, 102.0, 101.5]);
        let vol_col = blawktrust::Column::new_f64(vec![1000.0, 1200.0, 800.0]);

        table.add_column(px_sym, px_col);
        table.add_column(vol_sym, vol_col);

        // Save to file
        let filename = "/tmp/blisp_test_io.csv";
        save_csv(filename, &table, &interner).unwrap();

        // Load back
        let loaded = load_csv(filename, &mut interner).unwrap();

        if let Value::Table(loaded_table) = loaded {
            assert_eq!(loaded_table.row_count, 3);
            assert_eq!(loaded_table.columns.len(), 2);
        } else {
            panic!("Expected Table");
        }

        // Cleanup
        std::fs::remove_file(filename).ok();
    }

    #[test]
    fn test_parse_date_leap_years() {
        // Test Feb 29 in leap years
        assert_eq!(parse_date_to_days("2000-02-29").unwrap(), 11016);
        assert_eq!(parse_date_to_days("2004-02-29").unwrap(), 12477);
        assert_eq!(parse_date_to_days("1972-02-29").unwrap(), 789);
    }

    #[test]
    fn test_parse_date_roundtrip() {
        let test_dates = vec![
            "1970-01-01",
            "2000-01-01",
            "2000-02-29",
            "2024-12-31",
        ];

        for date_str in test_dates {
            let days = parse_date_to_days(date_str).unwrap();
            let formatted = format_date_from_days(days);
            assert_eq!(date_str, formatted);
        }
    }

    #[test]
    fn test_parse_timestamp_fractional_seconds() {
        // Test .1 (tenths)
        let ts1 = parse_timestamp_to_nanos("1970-01-01 00:00:00.1").unwrap();
        assert_eq!(ts1, 100_000_000);

        // Test .123 (milliseconds)
        let ts2 = parse_timestamp_to_nanos("1970-01-01 00:00:00.123").unwrap();
        assert_eq!(ts2, 123_000_000);

        // Test .123456789 (full nanoseconds)
        let ts3 = parse_timestamp_to_nanos("1970-01-01 00:00:00.123456789").unwrap();
        assert_eq!(ts3, 123_456_789);
    }

    #[test]
    fn test_parse_timestamp_roundtrip() {
        let test_timestamps = vec![
            "1970-01-01 00:00:00",
            "2000-01-01 12:30:45",
            "2000-01-01 12:30:45.1",
            "2000-01-01 12:30:45.123",
        ];

        for ts_str in test_timestamps {
            let nanos = parse_timestamp_to_nanos(ts_str).unwrap();
            let formatted = format_timestamp_from_nanos(nanos);
            assert_eq!(ts_str, formatted);
        }
    }

    #[test]
    fn test_detect_date_vs_timestamp() {
        assert!(is_date_format("2000-01-01"));
        assert!(!is_date_format("2000-01-01 12:30:00"));

        assert!(is_timestamp_format("2000-01-01 12:30:00"));
        assert!(is_timestamp_format("2000-01-01 12:30:00.123"));
        assert!(!is_timestamp_format("2000-01-01"));
    }

    #[test]
    fn test_load_csv_with_dates() {
        let mut interner = Interner::new();
        let csv = "date;value\n2000-01-01;100\n2000-01-02;200";

        let result = parse_csv(csv, &mut interner, None).unwrap();

        if let Value::Table(table) = result {
            assert_eq!(table.row_count, 2);
            assert_eq!(table.columns.len(), 2);

            // Check that first column is Date type
            match &table.columns[0].1 {
                blawktrust::Column::Date(data) => {
                    assert_eq!(data.len(), 2);
                }
                _ => panic!("Expected Date column"),
            }
        } else {
            panic!("Expected Table");
        }
    }

    #[test]
    fn test_load_csv_with_timestamps() {
        let mut interner = Interner::new();
        let csv = "timestamp;value\n2000-01-01 12:30:00;100\n2000-01-01 12:30:01.5;200";

        let result = parse_csv(csv, &mut interner, None).unwrap();

        if let Value::Table(table) = result {
            assert_eq!(table.row_count, 2);
            assert_eq!(table.columns.len(), 2);

            // Check that first column is Timestamp type
            match &table.columns[0].1 {
                blawktrust::Column::Timestamp(data) => {
                    assert_eq!(data.len(), 2);
                }
                _ => panic!("Expected Timestamp column"),
            }
        } else {
            panic!("Expected Table");
        }
    }

    #[test]
    fn test_load_csv_mixed_types() {
        let mut interner = Interner::new();
        let csv = "date;timestamp;value\n2000-01-01;2000-01-01 12:30:00;100";

        let result = parse_csv(csv, &mut interner, None).unwrap();

        if let Value::Table(table) = result {
            assert_eq!(table.row_count, 1);
            assert_eq!(table.columns.len(), 3);

            // Check column types
            match &table.columns[0].1 {
                blawktrust::Column::Date(_) => {},
                _ => panic!("Expected Date column for 'date'"),
            }
            match &table.columns[1].1 {
                blawktrust::Column::Timestamp(_) => {},
                _ => panic!("Expected Timestamp column for 'timestamp'"),
            }
            match &table.columns[2].1 {
                blawktrust::Column::F64(_) => {},
                _ => panic!("Expected F64 column for 'value'"),
            }
        } else {
            panic!("Expected Table");
        }
    }
}
