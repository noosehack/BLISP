//! IEEE-754 tripwire tests for dlog-obs operations
//!
//! These tests enforce that fused dlog-obs operations handle IEEE-754 edge cases
//! identically to unfused operations. Critical edge cases:
//! - ln(0) = -inf (not NaN!)
//! - ln(negative) = NaN
//! - 0/0 = NaN
//! - inf propagation
//!
//! If these tests fail, fusion is breaking IEEE-754 semantics.

use blisp::exec::{dlog_obs_column, fused_cs1_dlog_obs_column, fused_dlog_obs_elementwise_column};
use blisp::ir::NumericFunc;
use blawktrust::Column;

/// Helper: bitwise equality for IEEE-754 special values
/// - Both NaN → true
/// - Both +inf → true
/// - Both -inf → true
/// - Both finite → use epsilon (shouldn't happen in these tests)
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

/// Helper: compare two columns bitwise for IEEE-754 values
fn columns_ieee_equal(a: &Column, b: &Column) -> bool {
    match (a, b) {
        (Column::F64(a_data), Column::F64(b_data)) => {
            if a_data.len() != b_data.len() {
                return false;
            }
            a_data.iter().zip(b_data.iter()).all(|(a, b)| ieee_equal(*a, *b))
        }
        _ => false,
    }
}

#[test]
fn tripwire_dlog_obs_ln_zero_gives_neg_inf() {
    // Case: prev > 0, x = 0 → ln(0) - ln(prev) = -inf - finite = -inf
    // This is the case that was broken: fused was returning NaN instead of -inf

    let input = Column::F64(vec![1.0, 0.0]);

    let unfused = dlog_obs_column(&input, 1);
    let result = match unfused {
        Column::F64(data) => data,
        _ => panic!("Expected F64 column"),
    };

    assert!(result[0].is_nan(), "First value (no predecessor) should be NaN");
    assert!(result[1].is_infinite() && result[1].is_sign_negative(),
            "ln(0) - ln(1) should be -inf, got {}", result[1]);
    assert_eq!(result[1], f64::NEG_INFINITY, "Expected exactly -inf");
}

#[test]
fn tripwire_dlog_obs_zero_denominator_gives_pos_inf() {
    // Case: prev = 0, x > 0 → ln(x) - ln(0) = finite - (-inf) = +inf

    let input = Column::F64(vec![0.0, 1.0]);

    let unfused = dlog_obs_column(&input, 1);
    let result = match unfused {
        Column::F64(data) => data,
        _ => panic!("Expected F64 column"),
    };

    assert!(result[0].is_nan(), "First value (no predecessor) should be NaN");
    assert!(result[1].is_infinite() && result[1].is_sign_positive(),
            "ln(1) - ln(0) should be +inf, got {}", result[1]);
    assert_eq!(result[1], f64::INFINITY, "Expected exactly +inf");
}

#[test]
fn tripwire_dlog_obs_zero_over_zero_gives_nan() {
    // Case: prev = 0, x = 0 → ln(0) - ln(0) = -inf - (-inf) = NaN

    let input = Column::F64(vec![0.0, 0.0]);

    let unfused = dlog_obs_column(&input, 1);
    let result = match unfused {
        Column::F64(data) => data,
        _ => panic!("Expected F64 column"),
    };

    assert!(result[0].is_nan(), "First value (no predecessor) should be NaN");
    assert!(result[1].is_nan(), "ln(0) - ln(0) should be NaN, got {}", result[1]);
}

#[test]
fn tripwire_dlog_obs_negative_gives_nan() {
    // Case: prev < 0 or x < 0 → ln(negative) = NaN

    // Test negative current
    let input1 = Column::F64(vec![1.0, -1.0]);
    let result1 = dlog_obs_column(&input1, 1);
    let data1 = match result1 {
        Column::F64(data) => data,
        _ => panic!("Expected F64 column"),
    };
    assert!(data1[1].is_nan(), "ln(-1) should give NaN");

    // Test negative previous
    let input2 = Column::F64(vec![-1.0, 1.0]);
    let result2 = dlog_obs_column(&input2, 1);
    let data2 = match result2 {
        Column::F64(data) => data,
        _ => panic!("Expected F64 column"),
    };
    assert!(data2[1].is_nan(), "ln(1) - ln(-1) should give NaN");
}

#[test]
fn tripwire_dlog_obs_na_propagation() {
    // Case: OBS semantics skips NA and uses last valid observation
    // [1.0, NaN, 2.0] → [NaN, NaN, ln(2/1)]

    let input = Column::F64(vec![1.0, f64::NAN, 2.0]);
    let result = dlog_obs_column(&input, 1);
    let data = match result {
        Column::F64(data) => data,
        _ => panic!("Expected F64 column"),
    };

    assert!(data[0].is_nan(), "First value (no predecessor) should be NaN");
    assert!(data[1].is_nan(), "NA input should produce NA output");
    // OBS: skips NA, uses last_valid=1.0 for comparison with 2.0
    assert!(!data[2].is_nan() && data[2].is_finite(),
            "OBS should skip NA and compute ln(2/1), got {}", data[2]);
    assert!((data[2] - 2.0f64.ln()).abs() < 1e-10,
            "Expected ln(2) ≈ 0.693, got {}", data[2]);
}

// ============================================================================
// FUSED vs UNFUSED: Bitwise equality for all edge cases
// ============================================================================

#[test]
fn tripwire_fused_cs1_dlog_obs_matches_unfused_edge_cases() {
    // This is the critical test: fused must match unfused for ALL edge cases

    let test_cases = vec![
        // (description, input)
        ("prev>0 x=0 → -inf", vec![1.0, 0.0]),
        ("prev=0 x>0 → +inf", vec![0.0, 1.0]),
        ("prev=0 x=0 → NaN", vec![0.0, 0.0]),
        ("negative → NaN", vec![1.0, -1.0]),
        ("NA handling", vec![1.0, f64::NAN, 2.0]),
        ("underflow sequence", vec![1.0, 1e-300, 1e-310, 0.0]),
        ("mixed edge cases", vec![1.0, 0.0, 1.0, -1.0, 0.0]),
    ];

    for (desc, input_data) in test_cases {
        let input = Column::F64(input_data.clone());

        // Compute fused: cs1(dlog-obs(x))
        let fused = fused_cs1_dlog_obs_column(&input);

        // Compute unfused: cs1(dlog-obs(x))
        let dlog = dlog_obs_column(&input, 1);
        let unfused = cs1_column(&dlog);

        assert!(
            columns_ieee_equal(&fused, &unfused),
            "FUSED vs UNFUSED mismatch for case '{}'\nInput: {:?}\nFused: {:?}\nUnfused: {:?}",
            desc,
            input_data,
            fused,
            unfused
        );
    }
}

#[test]
fn tripwire_fused_dlog_obs_elementwise_matches_unfused() {
    // Test dlog-obs ∘ elementwise fusion (e.g., abs(dlog-obs(x)))

    let test_cases = vec![
        ("prev>0 x=0 → abs(-inf) = +inf", vec![1.0, 0.0]),
        ("prev=0 x>0 → abs(+inf) = +inf", vec![0.0, 1.0]),
        ("prev=0 x=0 → abs(NaN) = NaN", vec![0.0, 0.0]),
        ("negative → abs(NaN) = NaN", vec![1.0, -1.0]),
    ];

    let ops = vec![NumericFunc::ABS];

    for (desc, input_data) in test_cases {
        let input = Column::F64(input_data.clone());

        // Compute fused: abs(dlog-obs(x))
        let fused = fused_dlog_obs_elementwise_column(&input, &ops);

        // Compute unfused: abs(dlog-obs(x))
        let dlog = dlog_obs_column(&input, 1);
        let unfused = apply_abs_column(&dlog);

        assert!(
            columns_ieee_equal(&fused, &unfused),
            "FUSED ELEMENTWISE vs UNFUSED mismatch for case '{}'\nInput: {:?}\nFused: {:?}\nUnfused: {:?}",
            desc,
            input_data,
            fused,
            unfused
        );
    }
}

// ============================================================================
// Helper functions for unfused comparison
// ============================================================================

/// Unfused cs1 implementation for comparison
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

/// Unfused abs implementation for comparison
fn apply_abs_column(col: &Column) -> Column {
    match col {
        Column::F64(data) => {
            let result = data.iter().map(|&x| x.abs()).collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

// ============================================================================
// Documentation test: Prevent regressions
// ============================================================================

#[test]
fn tripwire_regression_cs1_exp_inv_dlog_cs1() {
    // This is the exact case that proptest found (commit 47d3239)
    // Simplified: The key is that dlog-obs must handle ln(0) = -inf correctly
    // Original pipeline: [Cs1, Exp, Inv, DlogObs, Cs1] causes underflow to 0

    // Simplified test: directly test the ln(0) case in dlog-obs
    // If inv underflows to 0, dlog should produce -inf
    let input = Column::F64(vec![1e-300, 0.0]); // Tiny value, then zero

    let dlog = dlog_obs_column(&input, 1);
    let data = match dlog {
        Column::F64(data) => data,
        _ => panic!("Expected F64"),
    };

    // Position 0: first value, no predecessor → NaN
    assert!(data[0].is_nan(), "First value should be NaN");

    // Position 1: ln(0) - ln(1e-300) = -inf - large_negative = -inf
    assert!(data[1].is_infinite() && data[1].is_sign_negative(),
            "ln(0/tiny) should be -inf, got {}", data[1]);
    assert_eq!(data[1], f64::NEG_INFINITY,
               "Expected exactly -inf from ln(0)");

    // Now test with cs1 accumulation (the full fused case)
    let fused = fused_cs1_dlog_obs_column(&input);
    let fused_data = match fused {
        Column::F64(data) => data,
        _ => panic!("Expected F64"),
    };

    // cs1 should propagate -inf correctly: 1.0 + (-inf) = -inf
    assert!(fused_data[1].is_infinite() && fused_data[1].is_sign_negative(),
            "cs1 should propagate -inf, got {}", fused_data[1]);
}

// Unused helper functions removed - test was simplified
