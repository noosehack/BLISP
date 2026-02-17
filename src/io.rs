//! I/O operations for blisp
//!
//! Handles CSV file loading, stdin reading, and CSV writing.

use crate::value::{Value, Table};
use crate::ast::{Interner, SymbolId};
use std::sync::Arc;
use std::io::{self, Read};

// Import NULL_TS sentinel from blawktrust (kdb-style null date)
use blawktrust::NULL_TS;

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
    let content = std::fs::read_to_string(filename)
        .map_err(|e| format!("Error reading file '{}': {}", filename, e))?;

    parse_csv(&content, interner, None)
}

/// Load CSV file with row limit (preview mode)
///
/// Only parses header + first `row_limit` rows for fast display/pipelines.
pub fn load_csv_limit(filename: &str, interner: &mut Interner, row_limit: usize) -> Result<Value, String> {
    let content = std::fs::read_to_string(filename)
        .map_err(|e| format!("Error reading file '{}': {}", filename, e))?;

    parse_csv(&content, interner, Some(row_limit))
}

/// Read CSV from stdin into a Table
pub fn load_stdin(interner: &mut Interner) -> Result<Value, String> {
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .map_err(|e| format!("Error reading stdin: {}", e))?;

    parse_csv(&buffer, interner, None)
}

/// Parse CSV content into a Table with optional row limit
///
/// If row_limit is Some(n), only parse header + first n data rows.
/// This is the "preview parser" fast path for display/pipelines.
fn parse_csv(content: &str, interner: &mut Interner, row_limit: Option<usize>) -> Result<Value, String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(b';')  // Use semicolon like clispi
        .from_reader(content.as_bytes());

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
    let mut ts_columns: Vec<Vec<i64>> = (0..num_cols)
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
                ColType::Ts => {
                    ts_columns[i].push(parse_date_or_null(field.trim()));
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
                ColType::Ts => {
                    // Parse dates (NA tokens → NULL_TS)
                    ts_columns[i].push(parse_date_or_null(field.trim()));
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
            ColType::Ts => blawktrust::Column::new_ts(ts_columns[i].clone()),
        };
        table.add_column(sym, col);
    }

    Ok(Value::Table(Arc::new(table)))
}

/// Column type detected from CSV
#[derive(Debug, Clone, Copy)]
enum ColType {
    F64,  // Numeric column
    Ts,   // Timestamp column (YYYY-MM-DD)
}

/// Detect column type from a sample value
fn detect_column_type(sample: &str) -> ColType {
    let trimmed = sample.trim();

    // Check if it looks like a date: YYYY-MM-DD
    if is_date_format(trimmed) {
        ColType::Ts
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

/// Parse numeric value, treating NA tokens as NaN
///
/// TASK A: Recognize {"NA", "NaN", "N/A", ""} as missing data
fn parse_numeric_with_na(s: &str) -> f64 {
    match s {
        "" | "NA" | "NaN" | "N/A" => f64::NAN,
        _ => s.parse::<f64>().unwrap_or(f64::NAN),
    }
}

/// Parse date or return NULL_TS sentinel for missing dates (kdb-style)
///
/// Handles NA tokens and invalid formats by returning NULL_TS.
fn parse_date_or_null(s: &str) -> i64 {
    // NA tokens → NULL_TS
    if is_na_token(s) {
        return NULL_TS;
    }

    // Try parsing as date
    parse_date_to_days_internal(s).unwrap_or(NULL_TS)
}

/// Parse YYYY-MM-DD date to days since Unix epoch (1970-01-01)
///
/// Returns Ok(days) or Err for invalid format.
/// For CSV loading, use parse_date_or_null instead (returns NULL_TS on error).
fn parse_date_to_days_internal(s: &str) -> Result<i64, String> {
    // Validate format first
    if !is_date_format(s) {
        return Err(format!("Invalid date format '{}', expected YYYY-MM-DD", s));
    }

    let parts: Vec<&str> = s.split('-').collect();

    let year: i32 = parts[0].parse()
        .map_err(|_| format!("Invalid year in date '{}'", s))?;
    let month: i32 = parts[1].parse()
        .map_err(|_| format!("Invalid month in date '{}'", s))?;
    let day: i32 = parts[2].parse()
        .map_err(|_| format!("Invalid day in date '{}'", s))?;

    // Simple date arithmetic (rough approximation - good enough for Bloomberg data)
    // Days since 1970-01-01
    let days_per_year = 365;
    let mut days: i64 = 0;

    // Add years since 1970
    days += (year - 1970) as i64 * days_per_year;

    // Add leap days (rough approximation)
    let leap_years = ((year - 1969) / 4 - (year - 1901) / 100 + (year - 1601) / 400) as i64;
    days += leap_years;

    // Add days for complete months
    let days_in_month = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 1..month {
        days += days_in_month[(m - 1) as usize] as i64;
    }

    // Add remaining days
    days += (day - 1) as i64;

    Ok(days)
}

/// Format days since epoch as YYYY-MM-DD string
///
/// Inverse of parse_date_to_days for CSV output
fn format_days_as_date(days: i64) -> String {
    // Simple reverse calculation (rough approximation)
    let days_per_year = 365;
    let mut remaining = days;

    // Estimate year
    let year = 1970 + (remaining / days_per_year) as i32;

    // Adjust for leap years (rough)
    let leap_years = ((year - 1969) / 4 - (year - 1901) / 100 + (year - 1601) / 400) as i64;
    remaining -= leap_years;
    remaining -= (year - 1970) as i64 * days_per_year;

    // Find month and day
    let days_in_month = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1;
    for &days_in_m in &days_in_month {
        if remaining < days_in_m as i64 {
            break;
        }
        remaining -= days_in_m as i64;
        month += 1;
    }
    let day = remaining + 1;

    format!("{:04}-{:02}-{:02}", year, month, day)
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
                blawktrust::Column::Ts(data) => {
                    if row_idx < data.len() {
                        let days = data[row_idx];
                        if days == NULL_TS {
                            row.push("NA".to_string());
                        } else {
                            // Convert days since epoch back to date string
                            row.push(format_days_as_date(days));
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

    // TASK B: Test date column support with NULL_TS sentinel
    #[test]
    fn test_parse_csv_with_dates() {
        let mut interner = Interner::new();
        let csv = "date;px;vol\n2000-01-03;100.0;1000\n2000-01-10;102.0;1200\nNA;105.0;1300";

        let result = parse_csv(csv, &mut interner, None).unwrap();

        if let Value::Table(table) = result {
            assert_eq!(table.row_count, 3);
            assert_eq!(table.columns.len(), 3);

            // Check date column is Ts type
            let date_col = table.get_column(interner.intern("date")).unwrap();
            match date_col {
                blawktrust::Column::Ts(data) => {
                    assert_eq!(data.len(), 3);
                    // Valid dates should be positive days since epoch
                    assert!(data[0] > 0 && data[0] != NULL_TS, "Date should be positive days");
                    assert!(data[1] > data[0] && data[1] != NULL_TS, "Later date should have more days");
                    // NA date should be NULL_TS
                    assert_eq!(data[2], NULL_TS, "NA date should be NULL_TS sentinel");
                }
                _ => panic!("Date column should be Ts type"),
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

            // Check date column (with NA → NULL_TS)
            let date_col = table.get_column(interner.intern("date")).unwrap();
            match date_col {
                blawktrust::Column::Ts(data) => {
                    assert!(data[0] != NULL_TS, "Valid date");
                    assert!(data[1] != NULL_TS, "Valid date");
                    assert_eq!(data[2], NULL_TS, "NA date should be NULL_TS");
                }
                _ => panic!("Expected Ts column"),
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

            // Should detect as Ts despite leading NAs (K-row lookahead)
            let date_col = table.get_column(interner.intern("date")).unwrap();
            match date_col {
                blawktrust::Column::Ts(data) => {
                    assert_eq!(data[0], NULL_TS, "First NA");
                    assert_eq!(data[1], NULL_TS, "Second NA");
                    assert!(data[2] != NULL_TS, "Valid date");
                    assert!(data[3] != NULL_TS, "Valid date");
                }
                _ => panic!("Should detect as Ts despite leading NAs"),
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
    fn test_parse_date_to_days_internal() {
        // 1970-01-01 is day 0
        assert_eq!(parse_date_to_days_internal("1970-01-01").unwrap(), 0);

        // 2000-01-01 should be ~30 years later
        let days_2000 = parse_date_to_days_internal("2000-01-01").unwrap();
        assert!(days_2000 > 10000 && days_2000 < 11000);

        // Invalid formats return error
        assert!(parse_date_to_days_internal("2000-1-1").is_err());
        assert!(parse_date_to_days_internal("not-a-date").is_err());
    }

    #[test]
    fn test_parse_date_or_null() {
        // Valid dates
        assert_eq!(parse_date_or_null("1970-01-01"), 0);
        assert!(parse_date_or_null("2000-01-01") > 0);

        // NA tokens → NULL_TS
        assert_eq!(parse_date_or_null("NA"), NULL_TS);
        assert_eq!(parse_date_or_null("NaN"), NULL_TS);
        assert_eq!(parse_date_or_null(""), NULL_TS);

        // Invalid format → NULL_TS
        assert_eq!(parse_date_or_null("invalid"), NULL_TS);
        assert_eq!(parse_date_or_null("2000-1-1"), NULL_TS);
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
}
