//! Common test utilities for IR equivalence testing
//!
//! This module provides:
//! - Frame builders (in-memory, no CSV I/O)
//! - Direct evaluator (uses SAME primitives as IR executor)
//! - Frame equivalence assertion (Arc identity + value equality)
//! - Expression generators (well-typed, join-safe)

use blisp::frame::{Frame, Tags, IndexColumn, ColData, map_numeric_preserve_tags, asofr, reindex_by};
use blisp::ast::{Expr, Interner, SymbolId};
use blisp::runtime::Runtime;
use blisp::value::Value;
use blawktrust::Column;
use std::sync::Arc;
use std::collections::HashMap;

const EPSILON: f64 = 1e-10;

/// Test environment: variable name → Frame
pub struct Env {
    pub frames: HashMap<String, Arc<Frame>>,
}

impl Env {
    pub fn new() -> Self {
        Self {
            frames: HashMap::new(),
        }
    }

    pub fn bind(&mut self, name: &str, frame: Arc<Frame>) {
        self.frames.insert(name.to_string(), frame);
    }

    pub fn get(&self, name: &str) -> Option<Arc<Frame>> {
        self.frames.get(name).cloned()
    }
}

/// Frame equivalence assertion
///
/// Checks per contracts.md:
/// 1. Same index type and values
/// 2. Same column names + ordering (Arc identity preferred)
/// 3. Same row count (I3)
/// 4. Values equal including NA sentinel semantics
pub fn assert_frame_equiv(a: &Frame, b: &Frame) {
    // I3: Same row count
    assert_eq!(
        a.nrows, b.nrows,
        "Frame row count mismatch: {} vs {}",
        a.nrows, b.nrows
    );

    // Same column count
    assert_eq!(
        a.ncols(), b.ncols(),
        "Frame column count mismatch: {} vs {}",
        a.ncols(), b.ncols()
    );

    // Index equivalence: type + values
    assert_index_equiv(&a.tags.index, &b.tags.index);

    // Colnames equivalence: Arc identity (I2) or value equality
    if Arc::ptr_eq(&a.tags.colnames, &b.tags.colnames) {
        // Preferred: Arc identity
    } else {
        // Fallback: value equality
        assert_eq!(
            *a.tags.colnames, *b.tags.colnames,
            "Column names differ (and Arc not shared)"
        );
    }

    // Value equivalence for all columns
    for col_idx in 0..a.ncols() {
        let a_col = a.get_col(col_idx).expect("Column missing in frame a");
        let b_col = b.get_col(col_idx).expect("Column missing in frame b");

        assert_column_equiv(a_col, b_col, col_idx);
    }
}

/// Assert two index columns are equivalent
fn assert_index_equiv(a: &IndexColumn, b: &IndexColumn) {
    match (a, b) {
        (IndexColumn::Date(a_vec), IndexColumn::Date(b_vec)) => {
            assert_eq!(**a_vec, **b_vec, "Date index values differ");
        }
        (IndexColumn::Timestamp(a_vec), IndexColumn::Timestamp(b_vec)) => {
            assert_eq!(**a_vec, **b_vec, "Timestamp index values differ");
        }
        (IndexColumn::String(a_vec), IndexColumn::String(b_vec)) => {
            assert_eq!(**a_vec, **b_vec, "String index values differ");
        }
        _ => panic!("Index types differ: {:?} vs {:?}",
            index_type_name(a), index_type_name(b)),
    }
}

fn index_type_name(idx: &IndexColumn) -> &str {
    match idx {
        IndexColumn::Date(_) => "Date",
        IndexColumn::Timestamp(_) => "Timestamp",
        IndexColumn::String(_) => "String",
    }
}

/// Assert two columns are equivalent (value + NA semantics)
fn assert_column_equiv(a: &Arc<Column>, b: &Arc<Column>, col_idx: usize) {
    match (&**a, &**b) {
        (Column::F64(a_data), Column::F64(b_data)) => {
            assert_eq!(
                a_data.len(), b_data.len(),
                "Column {} length mismatch",
                col_idx
            );

            for (row_idx, (&a_val, &b_val)) in a_data.iter().zip(b_data.iter()).enumerate() {
                if a_val.is_nan() && b_val.is_nan() {
                    // NA == NA is true
                    continue;
                } else if a_val.is_nan() || b_val.is_nan() {
                    panic!(
                        "Column {} row {}: NA mismatch: {} vs {}",
                        col_idx, row_idx, a_val, b_val
                    );
                } else if (a_val - b_val).abs() > EPSILON {
                    panic!(
                        "Column {} row {}: value mismatch: {} vs {} (diff: {})",
                        col_idx, row_idx, a_val, b_val, (a_val - b_val).abs()
                    );
                }
            }
        }
        _ => panic!("Non-F64 columns not yet supported in equivalence"),
    }
}

// ============================================================================
// Frame Builders (in-memory, deterministic)
// ============================================================================

/// Build a Date-indexed frame from seed
///
/// Returns a frame with:
/// - Date index (sorted, deterministic from seed)
/// - 1-3 numeric columns
/// - Some NA values sprinkled in
/// - Occasional duplicates in index (to test "last wins")
pub fn build_date_frame(
    seed: u64,
    name: &str,
    nrows: usize,
    ncols: usize,
    allow_duplicates: bool,
    na_rate: f64,
) -> Arc<Frame> {
    let mut rng = seed;

    // Generate sorted date index
    let mut dates: Vec<i32> = (0..nrows)
        .map(|i| {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            18000 + (i as i32) + ((rng % 10) as i32) // Gaps of 1-10 days
        })
        .collect();

    // Optionally add duplicates (test "last wins")
    if allow_duplicates && nrows > 2 {
        let dup_idx = (seed % (nrows as u64 / 2)) as usize;
        if dup_idx + 1 < nrows {
            dates[dup_idx + 1] = dates[dup_idx]; // Create duplicate
        }
    }

    let index = IndexColumn::Date(Arc::new(dates));

    // Generate numeric columns
    let mut cols = Vec::new();
    for col_idx in 0..ncols {
        let mut data = Vec::with_capacity(nrows);
        for row_idx in 0..nrows {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);

            // Inject NA at na_rate
            let inject_na = (rng as f64 / u64::MAX as f64) < na_rate;

            if inject_na {
                data.push(f64::NAN);
            } else {
                let base = 100.0 + (col_idx * 10) as f64;
                let noise = ((rng % 1000) as f64) / 100.0;
                data.push(base + noise + (row_idx as f64));
            }
        }
        cols.push(Arc::new(Column::new_f64(data)));
    }

    let colnames = (0..ncols)
        .map(|i| format!("col{}", i))
        .collect();

    let tags = Tags::new(name.to_string(), index, colnames);
    Arc::new(Frame::new(tags, cols))
}

/// Build a Timestamp-indexed frame
pub fn build_timestamp_frame(
    seed: u64,
    name: &str,
    nrows: usize,
    ncols: usize,
    allow_duplicates: bool,
    na_rate: f64,
) -> Arc<Frame> {
    let mut rng = seed;

    // Generate sorted timestamp index (nanoseconds)
    let base_ts = 1577836800_000_000_000i64; // 2020-01-01 00:00:00 UTC
    let mut timestamps: Vec<i64> = (0..nrows)
        .map(|i| {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let seconds = (i as i64) * 60 + ((rng % 60) as i64); // ~1 minute gaps
            base_ts + seconds * 1_000_000_000
        })
        .collect();

    // Optionally add duplicates
    if allow_duplicates && nrows > 2 {
        let dup_idx = (seed % (nrows as u64 / 2)) as usize;
        if dup_idx + 1 < nrows {
            timestamps[dup_idx + 1] = timestamps[dup_idx];
        }
    }

    let index = IndexColumn::Timestamp(Arc::new(timestamps));

    // Generate numeric columns (same logic as date)
    let mut cols = Vec::new();
    for col_idx in 0..ncols {
        let mut data = Vec::with_capacity(nrows);
        for row_idx in 0..nrows {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);

            let inject_na = (rng as f64 / u64::MAX as f64) < na_rate;

            if inject_na {
                data.push(f64::NAN);
            } else {
                let base = 100.0 + (col_idx * 10) as f64;
                let noise = ((rng % 1000) as f64) / 100.0;
                data.push(base + noise + (row_idx as f64));
            }
        }
        cols.push(Arc::new(Column::new_f64(data)));
    }

    let colnames = (0..ncols)
        .map(|i| format!("col{}", i))
        .collect();

    let tags = Tags::new(name.to_string(), index, colnames);
    Arc::new(Frame::new(tags, cols))
}

// ============================================================================
// Environment Generators
// ============================================================================

// Note: Environment generators removed - tests build env inline using Runtime's interner

// ============================================================================
// Direct Evaluator (uses SAME primitives as IR executor)
// ============================================================================

/// Direct evaluation using frozen primitives
///
/// CRITICAL: This MUST use the same primitives as the IR executor:
/// - map_numeric_preserve_tags for unary ops
/// - reindex_by/asofr for joins
///
/// Otherwise equivalence is meaningless.
pub fn direct_eval(expr: &Expr, env: &Env, interner: &Interner) -> Result<Arc<Frame>, String> {
    match expr {
        Expr::Sym(sym) => {
            let name = interner.resolve(*sym);
            env.get(name)
                .ok_or_else(|| format!("Undefined variable: {}", name))
        }

        Expr::List(elements) if !elements.is_empty() => {
            if let Expr::Sym(func_sym) = &elements[0] {
                let func_name = interner.resolve(*func_sym);

                match func_name {
                    // Unary ops: map_numeric_preserve_tags
                    "dlog" => {
                        if elements.len() != 2 {
                            return Err("dlog expects 1 argument".to_string());
                        }
                        let input = direct_eval(&elements[1], env, interner)?;
                        let result = map_numeric_preserve_tags(&input, |col| {
                            blawktrust::builtins::ops::dlog_column(col, 1)
                        });
                        Ok(Arc::new(result))
                    }

                    "ret" => {
                        if elements.len() != 2 {
                            return Err("ret expects 1 argument".to_string());
                        }
                        let input = direct_eval(&elements[1], env, interner)?;
                        let result = map_numeric_preserve_tags(&input, |col| {
                            ret_column(col, 1)
                        });
                        Ok(Arc::new(result))
                    }

                    "log" => {
                        if elements.len() != 2 {
                            return Err("log expects 1 argument".to_string());
                        }
                        let input = direct_eval(&elements[1], env, interner)?;
                        let result = map_numeric_preserve_tags(&input, |col| {
                            log_column(col)
                        });
                        Ok(Arc::new(result))
                    }

                    // Joins: reindex_by/asofr
                    "mapr" => {
                        if elements.len() != 3 {
                            return Err("mapr expects 2 arguments".to_string());
                        }
                        let x = direct_eval(&elements[1], env, interner)?;
                        let y = direct_eval(&elements[2], env, interner)?;
                        let result = reindex_by(&x, Arc::clone(&y.tags.index));
                        Ok(Arc::new(result))
                    }

                    "asofr" => {
                        if elements.len() != 3 {
                            return Err("asofr expects 2 arguments".to_string());
                        }
                        let x = direct_eval(&elements[1], env, interner)?;
                        let y = direct_eval(&elements[2], env, interner)?;
                        let result = asofr(&x, &y);
                        Ok(Arc::new(result))
                    }

                    _ => Err(format!("Unknown function in direct eval: {}", func_name)),
                }
            } else {
                Err("Function call must start with symbol".to_string())
            }
        }

        _ => Err(format!("Cannot direct-eval: {:?}", expr)),
    }
}

// Kernel functions (same as in exec.rs)
fn ret_column(col: &Column, lag: usize) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = vec![f64::NAN; data.len()];
            for i in lag..data.len() {
                let curr = data[i];
                let prev = data[i - lag];
                if !curr.is_nan() && !prev.is_nan() && prev != 0.0 {
                    result[i] = curr / prev - 1.0;
                } else {
                    result[i] = f64::NAN;
                }
            }
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

fn log_column(col: &Column) -> Column {
    match col {
        Column::F64(data) => {
            let result = data.iter().map(|&x| {
                if x > 0.0 && !x.is_nan() {
                    x.ln()
                } else {
                    f64::NAN
                }
            }).collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

// ============================================================================
// Expression Generators (well-typed, join-safe)
// ============================================================================

/// Generate a well-typed Date-indexed expression
///
/// Grammar (depth-bounded):
/// - Depth 0: Var("x") | Var("y") | Var("z")
/// - Depth > 0: Unary(op, sub) | Join(op, sub1, sub2)
///
/// Type safety: All subexpressions have DateIndex, no coercion
pub fn gen_expr_date(seed: u64, depth: usize, interner: &mut Interner) -> Expr {
    let mut rng = seed;

    if depth == 0 {
        // Leaf: variable reference
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        let var_idx = rng % 3;
        let var_name = match var_idx {
            0 => "x",
            1 => "y",
            _ => "z",
        };
        Expr::Sym(interner.intern(var_name))
    } else {
        // Internal node: unary or join
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        let choice = rng % 10;

        if choice < 5 {
            // Unary operation (50% probability)
            let op_choice = rng % 3;
            let op_name = match op_choice {
                0 => "dlog",
                1 => "ret",
                _ => "log",
            };

            let sub = gen_expr_date(rng.wrapping_add(1), depth - 1, interner);

            Expr::List(vec![
                Expr::Sym(interner.intern(op_name)),
                sub,
            ])
        } else {
            // Join operation (50% probability)
            let join_op = if (rng % 2) == 0 { "mapr" } else { "asofr" };

            let sub1 = gen_expr_date(rng.wrapping_add(1), depth - 1, interner);
            let sub2 = gen_expr_date(rng.wrapping_add(2), depth - 1, interner);

            Expr::List(vec![
                Expr::Sym(interner.intern(join_op)),
                sub1,
                sub2,
            ])
        }
    }
}

/// Generate a well-typed Timestamp-indexed expression
pub fn gen_expr_ts(seed: u64, depth: usize, interner: &mut Interner) -> Expr {
    // Same logic as Date (type system prevents cross-contamination)
    gen_expr_date(seed, depth, interner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_equiv_identical() {
        let frame = build_date_frame(42, "DATE", 10, 2, false, 0.1);
        assert_frame_equiv(&frame, &frame);
    }

    #[test]
    fn test_frame_equiv_cloned() {
        let f1 = build_date_frame(42, "DATE", 10, 2, false, 0.1);
        let f2 = build_date_frame(42, "DATE", 10, 2, false, 0.1);
        assert_frame_equiv(&f1, &f2);
    }

    #[test]
    #[should_panic(expected = "row count mismatch")]
    fn test_frame_equiv_different_rows() {
        let f1 = build_date_frame(42, "DATE", 10, 2, false, 0.1);
        let f2 = build_date_frame(42, "DATE", 15, 2, false, 0.1);
        assert_frame_equiv(&f1, &f2);
    }

    #[test]
    fn test_direct_eval_var() {
        let mut interner = Interner::new();
        let mut env = Env::new();
        let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
        env.bind("x", x);

        let expr = Expr::Sym(interner.intern("x"));
        let result = direct_eval(&expr, &env, &interner).unwrap();
        assert_eq!(result.nrows, 10);
    }

    #[test]
    fn test_direct_eval_dlog() {
        let mut interner = Interner::new();
        let mut env = Env::new();
        let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
        env.bind("x", x);

        let expr = Expr::List(vec![
            Expr::Sym(interner.intern("dlog")),
            Expr::Sym(interner.intern("x")),
        ]);
        let result = direct_eval(&expr, &env, &interner).unwrap();
        assert_eq!(result.nrows, 10);
    }

    #[test]
    fn test_expr_gen_depth_0() {
        let mut interner = Interner::new();
        let expr = gen_expr_date(42, 0, &mut interner);

        // Should be a variable
        match expr {
            Expr::Sym(_) => {}
            _ => panic!("Expected Sym at depth 0"),
        }
    }

    #[test]
    fn test_expr_gen_depth_1() {
        let mut interner = Interner::new();
        let expr = gen_expr_date(42, 1, &mut interner);

        // Should be a list (unary or join)
        match expr {
            Expr::List(_) => {}
            _ => panic!("Expected List at depth > 0"),
        }
    }
}
