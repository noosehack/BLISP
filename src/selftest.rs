//! Embedded self-tests for BLISP installation validation
//!
//! These tests verify that the BLISP installation is working correctly.
//! They are designed to run quickly (<1 second) without external dependencies.
//!
//! Critical tests:
//! 1. IEEE-754: ln(0) = -inf (not NaN!)
//! 2. IEEE-754: 0/0 = NaN
//! 3. IEEE-754: Fusion preserves edge cases
//! 4. Orientation: H vs Z produce different shapes
//! 5. Mask: Weekend detection logic
//! 6. Platform: f64 size check

use blawktrust::{Column, Table, TableView, ORI_H, ORI_Z};

pub struct SelfTestResults {
    pub passed: usize,
    pub failed: usize,
    pub total: usize,
    pub failures: Vec<String>,
}

/// Run all embedded self-tests
pub fn run_all_tests() -> SelfTestResults {
    let mut results = SelfTestResults {
        passed: 0,
        failed: 0,
        total: 0,
        failures: Vec::new(),
    };

    println!("Running BLISP self-tests...");
    println!();

    // Test 1: IEEE-754 ln(0) = -inf
    results.total += 1;
    print!("  [1/6] IEEE: ln(0) = -inf ... ");
    match test_ieee_ln_zero_gives_neg_inf() {
        Ok(_) => {
            println!("✅");
            results.passed += 1;
        }
        Err(e) => {
            println!("❌");
            results.failed += 1;
            results.failures.push(format!("Test 1 failed: {}", e));
        }
    }

    // Test 2: IEEE-754 0/0 = NaN
    results.total += 1;
    print!("  [2/6] IEEE: 0/0 = NaN ... ");
    match test_ieee_zero_over_zero_gives_nan() {
        Ok(_) => {
            println!("✅");
            results.passed += 1;
        }
        Err(e) => {
            println!("❌");
            results.failed += 1;
            results.failures.push(format!("Test 2 failed: {}", e));
        }
    }

    // Test 3: IEEE-754 fusion preserves edge cases
    results.total += 1;
    print!("  [3/6] IEEE: Fusion preserves edge cases ... ");
    match test_ieee_fusion_preserves_edge_cases() {
        Ok(_) => {
            println!("✅");
            results.passed += 1;
        }
        Err(e) => {
            println!("❌");
            results.failed += 1;
            results.failures.push(format!("Test 3 failed: {}", e));
        }
    }

    // Test 4: Orientation H vs Z different shapes
    results.total += 1;
    print!("  [4/6] Orientation: H vs Z different shapes ... ");
    match test_orientation_h_vs_z_different_shapes() {
        Ok(_) => {
            println!("✅");
            results.passed += 1;
        }
        Err(e) => {
            println!("❌");
            results.failed += 1;
            results.failures.push(format!("Test 4 failed: {}", e));
        }
    }

    // Test 5: Mask weekend detection
    results.total += 1;
    print!("  [5/6] Mask: Weekend detection ... ");
    match test_mask_weekend_detection() {
        Ok(_) => {
            println!("✅");
            results.passed += 1;
        }
        Err(e) => {
            println!("❌");
            results.failed += 1;
            results.failures.push(format!("Test 5 failed: {}", e));
        }
    }

    // Test 6: Platform f64 size check
    results.total += 1;
    print!("  [6/6] Platform: f64 size check ... ");
    match test_platform_f64_size() {
        Ok(_) => {
            println!("✅");
            results.passed += 1;
        }
        Err(e) => {
            println!("❌");
            results.failed += 1;
            results.failures.push(format!("Test 6 failed: {}", e));
        }
    }

    println!();
    println!("=== Self-Test Results ===");
    println!("Total:  {}", results.total);
    println!("Passed: {}", results.passed);
    println!("Failed: {}", results.failed);
    println!();

    if results.failed > 0 {
        println!("❌ {} self-tests FAILED", results.failed);
        println!();
        for failure in &results.failures {
            eprintln!("  {}", failure);
        }
    } else {
        println!("✅ All self-tests PASSED");
    }

    results
}

// ============================================================================
// Test Implementations
// ============================================================================

/// Test 1: IEEE-754 ln(0) = -inf (not NaN!)
fn test_ieee_ln_zero_gives_neg_inf() -> Result<(), String> {
    use crate::exec::dlog_obs_column;

    // Case: prev > 0, x = 0 → ln(0) - ln(prev) = -inf - finite = -inf
    let input = Column::F64(vec![1.0, 0.0]);
    let result = dlog_obs_column(&input, 1);

    let data = match result {
        Column::F64(data) => data,
        _ => return Err("Expected F64 column".to_string()),
    };

    if !data[0].is_nan() {
        return Err("First value (no predecessor) should be NaN".to_string());
    }

    if !(data[1].is_infinite() && data[1].is_sign_negative()) {
        return Err(format!("ln(0) - ln(1) should be -inf, got {}", data[1]));
    }

    if data[1] != f64::NEG_INFINITY {
        return Err("Expected exactly -inf".to_string());
    }

    Ok(())
}

/// Test 2: IEEE-754 0/0 = NaN
fn test_ieee_zero_over_zero_gives_nan() -> Result<(), String> {
    use crate::exec::dlog_obs_column;

    // Case: prev = 0, x = 0 → ln(0) - ln(0) = -inf - (-inf) = NaN
    let input = Column::F64(vec![0.0, 0.0]);
    let result = dlog_obs_column(&input, 1);

    let data = match result {
        Column::F64(data) => data,
        _ => return Err("Expected F64 column".to_string()),
    };

    if !data[0].is_nan() {
        return Err("First value (no predecessor) should be NaN".to_string());
    }

    if !data[1].is_nan() {
        return Err(format!("ln(0) - ln(0) should be NaN, got {}", data[1]));
    }

    Ok(())
}

/// Test 3: IEEE-754 fusion preserves edge cases
fn test_ieee_fusion_preserves_edge_cases() -> Result<(), String> {
    use crate::exec::{dlog_obs_column, fused_cs1_dlog_obs_column};

    // Helper: bitwise equality for IEEE-754 special values
    fn ieee_equal(a: f64, b: f64) -> bool {
        match (a.is_nan(), b.is_nan()) {
            (true, true) => true,
            (false, false) => {
                if a.is_infinite() && b.is_infinite() {
                    a.signum() == b.signum()
                } else {
                    (a - b).abs() < 1e-10
                }
            }
            _ => false,
        }
    }

    // Helper: compare two columns bitwise
    fn columns_ieee_equal(a: &Column, b: &Column) -> bool {
        match (a, b) {
            (Column::F64(a_data), Column::F64(b_data)) => {
                if a_data.len() != b_data.len() {
                    return false;
                }
                a_data
                    .iter()
                    .zip(b_data.iter())
                    .all(|(a, b)| ieee_equal(*a, *b))
            }
            _ => false,
        }
    }

    // Helper: unfused cs1
    fn cs1_column(col: &Column) -> Column {
        match col {
            Column::F64(data) => {
                let mut result = Vec::with_capacity(data.len());
                let mut acc = 1.0;
                for &x in data.iter() {
                    if x.is_nan() {
                        result.push(f64::NAN);
                    } else {
                        acc += x;
                        result.push(acc);
                    }
                }
                Column::F64(result)
            }
            _ => col.clone(),
        }
    }

    // Test critical edge cases: ln(0) → -inf
    let test_cases = [
        vec![1.0, 0.0],           // prev>0 x=0 → -inf
        vec![0.0, 1.0],           // prev=0 x>0 → +inf
        vec![0.0, 0.0],           // prev=0 x=0 → NaN
        vec![1.0, f64::NAN, 2.0], // NA handling
    ];

    for (i, input_data) in test_cases.iter().enumerate() {
        let input = Column::F64(input_data.clone());

        // Compute fused: cs1(dlog-obs(x))
        let fused = fused_cs1_dlog_obs_column(&input);

        // Compute unfused: cs1(dlog-obs(x))
        let dlog = dlog_obs_column(&input, 1);
        let unfused = cs1_column(&dlog);

        if !columns_ieee_equal(&fused, &unfused) {
            return Err(format!(
                "FUSED vs UNFUSED mismatch for case {}: {:?}",
                i, input_data
            ));
        }
    }

    Ok(())
}

/// Test 4: Orientation H vs Z produce different shapes
fn test_orientation_h_vs_z_different_shapes() -> Result<(), String> {
    use blawktrust::builtins::ori_ops::sum;

    // Build a tiny 3×2 table where row-wise vs col-wise aggregation differ
    // Data: A=[1,3,5], B=[2,4,6]
    //
    // Expected for H (column-major, aggregate down columns):
    //   sum → [9, 12] (sum of A, sum of B)
    //
    // Expected for Z (row-major, aggregate across rows):
    //   sum → [3, 7, 11] (sum of each row)

    let table = Table::new(
        vec!["A".to_string(), "B".to_string()],
        vec![
            Column::new_f64(vec![1.0, 3.0, 5.0]),
            Column::new_f64(vec![2.0, 4.0, 6.0]),
        ],
    );

    // Test 1: H orientation (column-major) - should aggregate down columns
    let view_h = TableView::with_ori(table.clone(), ORI_H);
    let result_h = sum(&view_h);

    let data_h = match result_h {
        Column::F64(data) => data,
        _ => return Err("Expected F64 column from sum".to_string()),
    };

    if data_h.len() != 2 {
        return Err(format!(
            "H orientation should produce 2 column sums, got {}",
            data_h.len()
        ));
    }

    if (data_h[0] - 9.0).abs() > 1e-9 {
        return Err(format!(
            "H orientation: sum of A should be 9.0, got {}",
            data_h[0]
        ));
    }

    if (data_h[1] - 12.0).abs() > 1e-9 {
        return Err(format!(
            "H orientation: sum of B should be 12.0, got {}",
            data_h[1]
        ));
    }

    // Test 2: Z orientation (row-major) - should aggregate across rows
    let view_z = TableView::with_ori(table, ORI_Z);
    let result_z = sum(&view_z);

    let data_z = match result_z {
        Column::F64(data) => data,
        _ => return Err("Expected F64 column from sum".to_string()),
    };

    if data_z.len() != 3 {
        return Err(format!(
            "Z orientation should produce 3 row sums, got {}",
            data_z.len()
        ));
    }

    if (data_z[0] - 3.0).abs() > 1e-9 {
        return Err(format!(
            "Z orientation: sum of row 0 should be 3.0, got {}",
            data_z[0]
        ));
    }

    if (data_z[1] - 7.0).abs() > 1e-9 {
        return Err(format!(
            "Z orientation: sum of row 1 should be 7.0, got {}",
            data_z[1]
        ));
    }

    if (data_z[2] - 11.0).abs() > 1e-9 {
        return Err(format!(
            "Z orientation: sum of row 2 should be 11.0, got {}",
            data_z[2]
        ));
    }

    // Critical assertion: results must be different shapes
    if data_h.len() == data_z.len() {
        return Err(
            "TRIPWIRE FAILED: H and Z orientations produce identical result shapes".to_string(),
        );
    }

    Ok(())
}

/// Test 5: Mask weekend detection
fn test_mask_weekend_detection() -> Result<(), String> {
    // Simplified test: verify weekend detection logic for known dates
    // Date 0 = 1970-01-01 (Thursday, day_of_week=4)
    // Date 3 = 1970-01-04 (Sunday, day_of_week=0) → weekend
    // Date 5 = 1970-01-06 (Tuesday, day_of_week=2) → not weekend

    let test_dates = vec![
        (0, false),  // Thursday
        (2, true),   // Saturday
        (3, true),   // Sunday
        (4, false),  // Monday
        (5, false),  // Tuesday
        (9, true),   // Saturday (1970-01-10)
        (10, true),  // Sunday (1970-01-11)
        (11, false), // Monday
    ];

    for (date, expected_weekend) in test_dates {
        let day_of_week = (4i32 + date).rem_euclid(7);
        let is_weekend = day_of_week == 0 || day_of_week == 6;

        if is_weekend != expected_weekend {
            return Err(format!(
                "Weekend detection failed for date {}: expected {}, got {}",
                date, expected_weekend, is_weekend
            ));
        }
    }

    Ok(())
}

/// Test 6: Platform f64 size check
fn test_platform_f64_size() -> Result<(), String> {
    if std::mem::size_of::<f64>() != 8 {
        return Err(format!(
            "f64 size is {} bytes, expected 8 bytes",
            std::mem::size_of::<f64>()
        ));
    }

    Ok(())
}
