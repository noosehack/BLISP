//! CSV verification with IEEE-754 awareness
//!
//! This module provides functionality to compare two CSV files with:
//! - IEEE-754 aware comparison (NaN=NaN, inf=inf)
//! - Configurable tolerance for numerical values
//! - Clear error reporting with row/column details

use std::fs::File;
use std::io::{BufRead, BufReader};

pub struct VerifyOptions {
    pub tolerance: f64,
    pub verbose: bool,
}

pub struct VerifyResults {
    pub rows_compared: usize,
    pub max_diff: f64,
    pub max_diff_row: usize,
    pub failures: Vec<VerifyFailure>,
}

#[derive(Debug)]
pub struct VerifyFailure {
    pub row: usize,
    pub col: String,
    pub expected: String,
    pub actual: String,
    pub diff: Option<f64>,
}

/// Verify that two CSV files match within tolerance
pub fn verify_csv(
    actual_path: &str,
    expected_path: &str,
    opts: &VerifyOptions,
) -> Result<VerifyResults, String> {
    // Read both files
    let actual_file =
        File::open(actual_path).map_err(|e| format!("Cannot open actual file: {}", e))?;
    let expected_file =
        File::open(expected_path).map_err(|e| format!("Cannot open expected file: {}", e))?;

    let actual_reader = BufReader::new(actual_file);
    let expected_reader = BufReader::new(expected_file);

    let mut actual_lines = actual_reader.lines();
    let mut expected_lines = expected_reader.lines();

    // Read headers
    let actual_header = actual_lines
        .next()
        .ok_or_else(|| "Actual file is empty".to_string())?
        .map_err(|e| format!("Error reading actual header: {}", e))?;

    let expected_header = expected_lines
        .next()
        .ok_or_else(|| "Expected file is empty".to_string())?
        .map_err(|e| format!("Error reading expected header: {}", e))?;

    // Detect separator (semicolon or comma)
    let separator = if actual_header.contains(';') { ';' } else { ',' };

    // Parse headers
    let actual_cols: Vec<&str> = actual_header.split(separator).collect();
    let expected_cols: Vec<&str> = expected_header.split(separator).collect();

    if actual_cols.len() != expected_cols.len() {
        return Err(format!(
            "Column count mismatch: actual has {} columns, expected has {}",
            actual_cols.len(),
            expected_cols.len()
        ));
    }

    // Verify column names match
    for (i, (actual_col, expected_col)) in actual_cols.iter().zip(expected_cols.iter()).enumerate()
    {
        if actual_col != expected_col {
            return Err(format!(
                "Column name mismatch at position {}: actual='{}', expected='{}'",
                i, actual_col, expected_col
            ));
        }
    }

    // Compare data rows
    let mut results = VerifyResults {
        rows_compared: 0,
        max_diff: 0.0,
        max_diff_row: 0,
        failures: Vec::new(),
    };

    let mut row_num = 1; // Start at 1 (header is row 0)

    loop {
        let actual_line = actual_lines.next();
        let expected_line = expected_lines.next();

        match (actual_line, expected_line) {
            (None, None) => break, // Both files ended
            (None, Some(_)) => {
                return Err(format!(
                    "Row count mismatch: expected file has more rows (actual ended at row {})",
                    row_num
                ));
            }
            (Some(_), None) => {
                return Err(format!(
                    "Row count mismatch: actual file has more rows (expected ended at row {})",
                    row_num
                ));
            }
            (Some(actual), Some(expected)) => {
                let actual = actual.map_err(|e| format!("Error reading actual row {}: {}", row_num, e))?;
                let expected = expected.map_err(|e| format!("Error reading expected row {}: {}", row_num, e))?;

                // Parse row values
                let actual_values: Vec<&str> = actual.split(separator).collect();
                let expected_values: Vec<&str> = expected.split(separator).collect();

                if actual_values.len() != expected_values.len() {
                    return Err(format!(
                        "Column count mismatch at row {}: actual has {} values, expected has {}",
                        row_num,
                        actual_values.len(),
                        expected_values.len()
                    ));
                }

                // Compare each value
                for (col_idx, (actual_val, expected_val)) in actual_values
                    .iter()
                    .zip(expected_values.iter())
                    .enumerate()
                {
                    if let Err(failure) = compare_values(
                        actual_val,
                        expected_val,
                        opts.tolerance,
                        row_num,
                        &actual_cols[col_idx],
                    ) {
                        // Track max diff
                        if let Some(diff) = failure.diff {
                            if diff > results.max_diff {
                                results.max_diff = diff;
                                results.max_diff_row = row_num;
                            }
                        }

                        // Record failure (limit to first 10 unless verbose)
                        if opts.verbose || results.failures.len() < 10 {
                            results.failures.push(failure);
                        }
                    }
                }

                results.rows_compared += 1;
                row_num += 1;
            }
        }
    }

    if results.failures.is_empty() {
        Ok(results)
    } else {
        Err(format_failures(&results, opts.verbose))
    }
}

/// Compare two values with IEEE-754 awareness
fn compare_values(
    actual: &str,
    expected: &str,
    tolerance: f64,
    row: usize,
    col: &str,
) -> Result<(), VerifyFailure> {
    // Try to parse as f64
    let actual_f64 = actual.trim().parse::<f64>();
    let expected_f64 = expected.trim().parse::<f64>();

    match (actual_f64, expected_f64) {
        (Ok(a), Ok(e)) => {
            // Both are numbers - use IEEE-754 aware comparison
            if !ieee_equal(a, e, tolerance) {
                let diff = if a.is_finite() && e.is_finite() {
                    Some((a - e).abs())
                } else {
                    None
                };

                return Err(VerifyFailure {
                    row,
                    col: col.to_string(),
                    expected: expected.to_string(),
                    actual: actual.to_string(),
                    diff,
                });
            }
        }
        _ => {
            // Not both numbers - compare as strings
            if actual.trim() != expected.trim() {
                return Err(VerifyFailure {
                    row,
                    col: col.to_string(),
                    expected: expected.to_string(),
                    actual: actual.to_string(),
                    diff: None,
                });
            }
        }
    }

    Ok(())
}

/// IEEE-754 aware equality comparison
fn ieee_equal(a: f64, b: f64, tolerance: f64) -> bool {
    match (a.is_nan(), b.is_nan()) {
        (true, true) => true, // Both NaN → equal
        (false, false) => {
            if a.is_infinite() && b.is_infinite() {
                a.signum() == b.signum() // Same infinity
            } else if a.is_finite() && b.is_finite() {
                (a - b).abs() <= tolerance // Within tolerance
            } else {
                false // Mixed finite/infinite
            }
        }
        _ => false, // Mixed NaN/finite → not equal
    }
}

/// Format failure messages
fn format_failures(results: &VerifyResults, verbose: bool) -> String {
    let mut msg = String::new();

    msg.push_str(&format!(
        "\nFound {} differences in {} rows:\n",
        results.failures.len(),
        results.rows_compared
    ));

    let display_count = if verbose {
        results.failures.len()
    } else {
        10.min(results.failures.len())
    };

    for (i, failure) in results.failures.iter().take(display_count).enumerate() {
        msg.push_str(&format!("\n  [{}] Row {}, Column '{}':\n", i + 1, failure.row, failure.col));
        msg.push_str(&format!("      Expected: {}\n", failure.expected));
        msg.push_str(&format!("      Actual:   {}\n", failure.actual));
        if let Some(diff) = failure.diff {
            msg.push_str(&format!("      Diff:     {:.2e}\n", diff));
        }
    }

    if !verbose && results.failures.len() > 10 {
        msg.push_str(&format!(
            "\n  ... and {} more failures (use --verbose to see all)\n",
            results.failures.len() - 10
        ));
    }

    if results.max_diff > 0.0 {
        msg.push_str(&format!(
            "\nMax difference: {:.2e} at row {}\n",
            results.max_diff, results.max_diff_row
        ));
    }

    msg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ieee_equal_nan() {
        assert!(ieee_equal(f64::NAN, f64::NAN, 1e-6));
        assert!(!ieee_equal(f64::NAN, 1.0, 1e-6));
        assert!(!ieee_equal(1.0, f64::NAN, 1e-6));
    }

    #[test]
    fn test_ieee_equal_inf() {
        assert!(ieee_equal(f64::INFINITY, f64::INFINITY, 1e-6));
        assert!(ieee_equal(f64::NEG_INFINITY, f64::NEG_INFINITY, 1e-6));
        assert!(!ieee_equal(f64::INFINITY, f64::NEG_INFINITY, 1e-6));
        assert!(!ieee_equal(f64::INFINITY, 1.0, 1e-6));
    }

    #[test]
    fn test_ieee_equal_finite() {
        assert!(ieee_equal(1.0, 1.0, 1e-6));
        assert!(ieee_equal(1.0, 1.0000001, 1e-6));
        assert!(!ieee_equal(1.0, 1.001, 1e-6));
    }
}
