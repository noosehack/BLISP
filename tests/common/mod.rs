//! Common test utilities for IR equivalence testing
//!
//! This module provides:
//! - Frame builders (in-memory, no CSV I/O)
//! - Direct evaluator (uses SAME primitives as IR executor)
//! - Frame equivalence assertion (Arc identity + value equality)
//! - Expression generators (well-typed, join-safe)

use blawktrust::Column;
use blisp::ast::{Expr, Interner, SymbolId};
use blisp::frame::{
    asofr, map_numeric_preserve_tags, reindex_by, ColData, Frame, IndexColumn, Tags,
};
use blisp::runtime::Runtime;
use blisp::value::Value;
use std::collections::HashMap;
use std::sync::Arc;

const EPSILON: f64 = 1e-10;

/// Test environment: variable name → Frame
#[derive(Clone)]
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
        a.ncols(),
        b.ncols(),
        "Frame column count mismatch: {} vs {}",
        a.ncols(),
        b.ncols()
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
        _ => panic!(
            "Index types differ: {:?} vs {:?}",
            index_type_name(a),
            index_type_name(b)
        ),
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
                a_data.len(),
                b_data.len(),
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
                        col_idx,
                        row_idx,
                        a_val,
                        b_val,
                        (a_val - b_val).abs()
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

    let colnames = (0..ncols).map(|i| format!("col{}", i)).collect();

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

    let colnames = (0..ncols).map(|i| format!("col{}", i)).collect();

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
                        let result = map_numeric_preserve_tags(&input, |col| ret_column(col, 1));
                        Ok(Arc::new(result))
                    }

                    "log" => {
                        if elements.len() != 2 {
                            return Err("log expects 1 argument".to_string());
                        }
                        let input = direct_eval(&elements[1], env, interner)?;
                        let result = map_numeric_preserve_tags(&input, |col| log_column(col));
                        Ok(Arc::new(result))
                    }

                    "shift" => {
                        if elements.len() != 3 {
                            return Err("shift expects 2 arguments".to_string());
                        }

                        // Parse k
                        let k = match &elements[1] {
                            Expr::Int(i) if *i >= 0 => *i as usize,
                            Expr::Int(i) => {
                                return Err(format!("shift k must be non-negative, got {}", i))
                            }
                            _ => return Err("shift k must be non-negative integer".to_string()),
                        };

                        let input = direct_eval(&elements[2], env, interner)?;
                        let result = map_numeric_preserve_tags(&input, |col| shift_column(col, k));
                        Ok(Arc::new(result))
                    }

                    "rolling-mean" => {
                        if elements.len() != 3 {
                            return Err("rolling-mean expects 2 arguments".to_string());
                        }

                        // Parse w
                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => {
                                return Err(format!("rolling-mean w must be positive, got {}", i))
                            }
                            _ => return Err("rolling-mean w must be positive integer".to_string()),
                        };

                        let input = direct_eval(&elements[2], env, interner)?;
                        let result =
                            map_numeric_preserve_tags(&input, |col| rolling_mean_column(col, w));
                        Ok(Arc::new(result))
                    }

                    "ft-mean" => {
                        if elements.len() != 3 {
                            return Err("ft-mean expects 2 arguments".to_string());
                        }

                        // Parse w
                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => {
                                return Err(format!("ft-mean w must be positive, got {}", i))
                            }
                            _ => return Err("ft-mean w must be positive integer".to_string()),
                        };

                        // ft-mean(w, x) = shift(1, rolling-mean(w, x))
                        let input = direct_eval(&elements[2], env, interner)?;
                        let rolling_result =
                            map_numeric_preserve_tags(&input, |col| rolling_mean_column(col, w));
                        let result =
                            map_numeric_preserve_tags(&rolling_result, |col| shift_column(col, 1));
                        Ok(Arc::new(result))
                    }

                    "rolling-std" => {
                        if elements.len() != 3 {
                            return Err("rolling-std expects 2 arguments".to_string());
                        }

                        // Parse w
                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => {
                                return Err(format!("rolling-std w must be positive, got {}", i))
                            }
                            _ => return Err("rolling-std w must be positive integer".to_string()),
                        };

                        let input = direct_eval(&elements[2], env, interner)?;
                        let result =
                            map_numeric_preserve_tags(&input, |col| rolling_std_column(col, w));
                        Ok(Arc::new(result))
                    }

                    "ft-std" => {
                        if elements.len() != 3 {
                            return Err("ft-std expects 2 arguments".to_string());
                        }

                        // Parse w
                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => {
                                return Err(format!("ft-std w must be positive, got {}", i))
                            }
                            _ => return Err("ft-std w must be positive integer".to_string()),
                        };

                        // ft-std(w, x) = shift(1, rolling-std(w, x))
                        let input = direct_eval(&elements[2], env, interner)?;
                        let rolling_result =
                            map_numeric_preserve_tags(&input, |col| rolling_std_column(col, w));
                        let result =
                            map_numeric_preserve_tags(&rolling_result, |col| shift_column(col, 1));
                        Ok(Arc::new(result))
                    }

                    "rolling-zscore" => {
                        if elements.len() != 3 {
                            return Err("rolling-zscore expects 2 arguments".to_string());
                        }

                        // Parse w
                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => {
                                return Err(format!("rolling-zscore w must be positive, got {}", i))
                            }
                            _ => {
                                return Err("rolling-zscore w must be positive integer".to_string())
                            }
                        };

                        // rolling-zscore(w, x) = (x - rolling_mean(w,x)) / rolling_std(w,x)
                        let input = direct_eval(&elements[2], env, interner)?;
                        let mean_result =
                            map_numeric_preserve_tags(&input, |col| rolling_mean_column(col, w));
                        let std_result =
                            map_numeric_preserve_tags(&input, |col| rolling_std_column(col, w));

                        // (x - mean) / std
                        let result = map_numeric_preserve_tags(&input, |col| {
                            match (
                                col,
                                mean_result.cols.iter().next(),
                                std_result.cols.iter().next(),
                            ) {
                                (
                                    Column::F64(x_data),
                                    Some(blisp::frame::ColData::Mat(mean_col)),
                                    Some(blisp::frame::ColData::Mat(std_col)),
                                ) => match (mean_col.as_ref(), std_col.as_ref()) {
                                    (Column::F64(mean_data), Column::F64(std_data)) => {
                                        let result: Vec<f64> = x_data
                                            .iter()
                                            .zip(mean_data.iter())
                                            .zip(std_data.iter())
                                            .map(|((&x, &mean), &std)| {
                                                if x.is_nan()
                                                    || mean.is_nan()
                                                    || std.is_nan()
                                                    || std == 0.0
                                                {
                                                    f64::NAN
                                                } else {
                                                    (x - mean) / std
                                                }
                                            })
                                            .collect();
                                        Column::F64(result)
                                    }
                                    _ => col.clone(),
                                },
                                _ => col.clone(),
                            }
                        });
                        Ok(Arc::new(result))
                    }

                    "ft-zscore" => {
                        if elements.len() != 3 {
                            return Err("ft-zscore expects 2 arguments".to_string());
                        }

                        // Parse w
                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => {
                                return Err(format!("ft-zscore w must be positive, got {}", i))
                            }
                            _ => return Err("ft-zscore w must be positive integer".to_string()),
                        };

                        // ft-zscore(w, x) = (x - ft_mean(w,x)) / ft_std(w,x)
                        // where ft_mean = shift(1, rolling_mean) and ft_std = shift(1, rolling_std)
                        let input = direct_eval(&elements[2], env, interner)?;

                        let rolling_mean =
                            map_numeric_preserve_tags(&input, |col| rolling_mean_column(col, w));
                        let ft_mean =
                            map_numeric_preserve_tags(&rolling_mean, |col| shift_column(col, 1));

                        let rolling_std =
                            map_numeric_preserve_tags(&input, |col| rolling_std_column(col, w));
                        let ft_std =
                            map_numeric_preserve_tags(&rolling_std, |col| shift_column(col, 1));

                        // (x - ft_mean) / ft_std
                        let result = map_numeric_preserve_tags(&input, |col| {
                            match (col, ft_mean.cols.iter().next(), ft_std.cols.iter().next()) {
                                (
                                    Column::F64(x_data),
                                    Some(blisp::frame::ColData::Mat(mean_col)),
                                    Some(blisp::frame::ColData::Mat(std_col)),
                                ) => match (mean_col.as_ref(), std_col.as_ref()) {
                                    (Column::F64(mean_data), Column::F64(std_data)) => {
                                        let result: Vec<f64> = x_data
                                            .iter()
                                            .zip(mean_data.iter())
                                            .zip(std_data.iter())
                                            .map(|((&x, &mean), &std)| {
                                                if x.is_nan()
                                                    || mean.is_nan()
                                                    || std.is_nan()
                                                    || std == 0.0
                                                {
                                                    f64::NAN
                                                } else {
                                                    (x - mean) / std
                                                }
                                            })
                                            .collect();
                                        Column::F64(result)
                                    }
                                    _ => col.clone(),
                                },
                                _ => col.clone(),
                            }
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

                    // Let bindings: (let ((name1 expr1) ...) body)
                    "let" => {
                        if elements.len() != 3 {
                            return Err("let expects 2 arguments".to_string());
                        }

                        // Parse bindings
                        let bindings_list = match &elements[1] {
                            Expr::List(bindings) => bindings,
                            _ => return Err("let expects list of bindings".to_string()),
                        };

                        // Create extended environment (sequential binding)
                        let mut extended_env = env.clone();

                        for binding in bindings_list {
                            match binding {
                                Expr::List(pair) if pair.len() == 2 => {
                                    let name = match &pair[0] {
                                        Expr::Sym(s) => interner.resolve(*s).to_string(),
                                        _ => return Err("let binding expects symbol".to_string()),
                                    };

                                    // Evaluate in current extended env (sequential semantics)
                                    let value = direct_eval(&pair[1], &extended_env, interner)?;
                                    extended_env.bind(&name, value);
                                }
                                _ => return Err("let binding must be (symbol expr)".to_string()),
                            }
                        }

                        // Evaluate body in extended environment
                        direct_eval(&elements[2], &extended_env, interner)
                    }

                    // Binary operations
                    "+" | "-" | "*" | "/" => {
                        if elements.len() != 3 {
                            return Err(format!("{} expects 2 arguments", func_name));
                        }

                        let lhs = direct_eval(&elements[1], env, interner)?;

                        // RHS can be scalar or frame
                        let result = match &elements[2] {
                            Expr::Float(scalar) => {
                                // Scalar RHS: broadcast
                                let result = map_numeric_preserve_tags(&lhs, |col| {
                                    binary_scalar_column(col, *scalar, func_name)
                                });
                                Arc::new(result)
                            }
                            Expr::Int(i) => {
                                // Integer scalar: convert to f64
                                let scalar = *i as f64;
                                let result = map_numeric_preserve_tags(&lhs, |col| {
                                    binary_scalar_column(col, scalar, func_name)
                                });
                                Arc::new(result)
                            }
                            _ => {
                                // Frame RHS: element-wise
                                let rhs = direct_eval(&elements[2], env, interner)?;
                                binary_frame_frame(&lhs, &rhs, func_name)?
                            }
                        };

                        Ok(result)
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
            let result = data
                .iter()
                .map(|&x| {
                    if x > 0.0 && !x.is_nan() {
                        x.ln()
                    } else {
                        f64::NAN
                    }
                })
                .collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

fn shift_column(col: &Column, k: usize) -> Column {
    match col {
        Column::F64(data) => {
            let nrows = data.len();
            let mut result = vec![f64::NAN; nrows];

            // output[i] = input[i-k] for i >= k, NA for i < k
            if k < nrows {
                result[k..].copy_from_slice(&data[0..nrows - k]);
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

fn rolling_mean_column(col: &Column, w: usize) -> Column {
    match col {
        Column::F64(data) => {
            let nrows = data.len();
            let mut result = vec![f64::NAN; nrows];

            // Trailing window [i-w+1 .. i], strict min_periods, skip NA
            for i in (w - 1)..nrows {
                let window_start = i + 1 - w;
                let window_end = i + 1;

                let mut sum = 0.0;
                let mut count = 0;

                for &x in &data[window_start..window_end] {
                    if !x.is_nan() {
                        sum += x;
                        count += 1;
                    }
                }

                if count >= w {
                    result[i] = sum / (w as f64);
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

fn rolling_std_column(col: &Column, w: usize) -> Column {
    match col {
        Column::F64(data) => {
            let nrows = data.len();
            let mut result = vec![f64::NAN; nrows];

            // Trailing window [i-w+1 .. i], strict min_periods, skip NA
            for i in (w - 1)..nrows {
                let window_start = i + 1 - w;
                let window_end = i + 1;

                // Collect valid values
                let mut values = Vec::with_capacity(w);
                for &x in &data[window_start..window_end] {
                    if !x.is_nan() {
                        values.push(x);
                    }
                }

                // Strict min_periods: require w valid values
                if values.len() >= w {
                    // Compute mean
                    let sum: f64 = values.iter().sum();
                    let mean = sum / (w as f64);

                    // Compute variance (population, ddof=0)
                    let variance: f64 = values
                        .iter()
                        .map(|&x| {
                            let diff = x - mean;
                            diff * diff
                        })
                        .sum::<f64>()
                        / (w as f64);

                    // Std is sqrt of variance
                    result[i] = variance.sqrt();
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

// Binary operation helpers
fn binary_scalar_column(col: &Column, scalar: f64, op: &str) -> Column {
    match col {
        Column::F64(data) => {
            let result = data
                .iter()
                .map(|&x| {
                    if x.is_nan() || scalar.is_nan() {
                        f64::NAN
                    } else {
                        match op {
                            "+" => x + scalar,
                            "-" => x - scalar,
                            "*" => x * scalar,
                            "/" => {
                                if scalar == 0.0 {
                                    f64::NAN
                                } else {
                                    x / scalar
                                }
                            }
                            _ => f64::NAN,
                        }
                    }
                })
                .collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

fn binary_frame_frame(lhs: &Frame, rhs: &Frame, op: &str) -> Result<Arc<Frame>, String> {
    if lhs.cols.len() != rhs.cols.len() {
        return Err(format!(
            "Binary op requires same column count: {} vs {}",
            lhs.cols.len(),
            rhs.cols.len()
        ));
    }
    if lhs.nrows != rhs.nrows {
        return Err(format!(
            "Binary op requires same row count: {} vs {}",
            lhs.nrows, rhs.nrows
        ));
    }

    let mut result_cols = Vec::with_capacity(lhs.cols.len());

    for (lhs_col, rhs_col) in lhs.cols.iter().zip(rhs.cols.iter()) {
        let lhs_data = match lhs_col {
            ColData::Mat(col) => col,
        };
        let rhs_data = match rhs_col {
            ColData::Mat(col) => col,
        };

        let result_col = binary_column_column(lhs_data, rhs_data, op)?;
        result_cols.push(ColData::Mat(Arc::new(result_col)));
    }

    Ok(Arc::new(Frame {
        tags: lhs.tags.clone(),
        cols: result_cols,
        nrows: lhs.nrows,
    }))
}

fn binary_column_column(lhs: &Column, rhs: &Column, op: &str) -> Result<Column, String> {
    match (lhs, rhs) {
        (Column::F64(lhs_data), Column::F64(rhs_data)) => {
            if lhs_data.len() != rhs_data.len() {
                return Err(format!(
                    "Binary op requires same length: {} vs {}",
                    lhs_data.len(),
                    rhs_data.len()
                ));
            }

            let result = lhs_data
                .iter()
                .zip(rhs_data.iter())
                .map(|(&x, &y)| {
                    if x.is_nan() || y.is_nan() {
                        f64::NAN
                    } else {
                        match op {
                            "+" => x + y,
                            "-" => x - y,
                            "*" => x * y,
                            "/" => {
                                if y == 0.0 {
                                    f64::NAN
                                } else {
                                    x / y
                                }
                            }
                            _ => f64::NAN,
                        }
                    }
                })
                .collect();

            Ok(Column::F64(result))
        }
        _ => Err("Binary op requires F64 columns".to_string()),
    }
}

// ============================================================================
// Expression Generators (well-typed, join-safe)
// ============================================================================

/// Generate a let* expression with well-typed bindings
///
/// Grammar: (let ((sym1 expr1) (sym2 expr2) ...) body)
///
/// Properties:
/// - Sequential scoping (sym2 can reference sym1)
/// - All bound expressions reference ONLY outer variables (x, y, z)
/// - Body references ONLY bound symbols
/// - This ensures type safety and prevents scope confusion
fn gen_let_expr(mut rng: u64, depth: usize, interner: &mut Interner) -> Expr {
    rng = rng.wrapping_mul(1103515245).wrapping_add(12345);

    // Generate 1-2 bindings for simplicity
    let num_bindings = 1 + (rng % 2) as usize;

    let mut bindings = Vec::new();
    let mut bound_symbols = Vec::new();

    for i in 0..num_bindings {
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);

        // Generate unique symbol name
        let sym_name = format!("_let{}", i);
        let sym = interner.intern(&sym_name);
        bound_symbols.push(sym);

        // Generate SIMPLE bound expression using ONLY outer variables
        // This prevents scope confusion
        let var_choice = rng % 3;
        let var_name = match var_choice {
            0 => "x",
            1 => "y",
            _ => "z",
        };

        // Optionally apply a simple unary operation
        let bound_expr = if (rng % 2) == 0 {
            // Just the variable
            Expr::Sym(interner.intern(var_name))
        } else {
            // Apply dlog to the variable
            Expr::List(vec![
                Expr::Sym(interner.intern("dlog")),
                Expr::Sym(interner.intern(var_name)),
            ])
        };

        bindings.push(Expr::List(vec![Expr::Sym(sym), bound_expr]));
    }

    // Generate body using ONLY bound symbols
    rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
    let which_sym = (rng % bound_symbols.len() as u64) as usize;
    let body = if num_bindings == 1 {
        // Single binding: just return it or apply unary
        if (rng % 2) == 0 {
            Expr::Sym(bound_symbols[0])
        } else {
            Expr::List(vec![
                Expr::Sym(interner.intern("dlog")),
                Expr::Sym(bound_symbols[0]),
            ])
        }
    } else {
        // Multiple bindings: join them or return one
        if (rng % 2) == 0 {
            // Join two bound symbols
            Expr::List(vec![
                Expr::Sym(interner.intern("mapr")),
                Expr::Sym(bound_symbols[0]),
                Expr::Sym(bound_symbols[1]),
            ])
        } else {
            // Just return one
            Expr::Sym(bound_symbols[which_sym])
        }
    };

    // Construct let expression
    Expr::List(vec![
        Expr::Sym(interner.intern("let")),
        Expr::List(bindings),
        body,
    ])
}

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
        // Internal node: unary, join, or let
        rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
        let choice = rng % 10;

        if choice < 4 {
            // Unary operation (40% probability)
            let op_choice = rng % 3;
            let op_name = match op_choice {
                0 => "dlog",
                1 => "ret",
                _ => "log",
            };

            let sub = gen_expr_date(rng.wrapping_add(1), depth - 1, interner);

            Expr::List(vec![Expr::Sym(interner.intern(op_name)), sub])
        } else if choice < 8 {
            // Join operation (40% probability)
            let join_op = if (rng % 2) == 0 { "mapr" } else { "asofr" };

            let sub1 = gen_expr_date(rng.wrapping_add(1), depth - 1, interner);
            let sub2 = gen_expr_date(rng.wrapping_add(2), depth - 1, interner);

            Expr::List(vec![Expr::Sym(interner.intern(join_op)), sub1, sub2])
        } else {
            // Let binding (20% probability, only at depth > 2 to ensure enough depth)
            if depth > 2 {
                gen_let_expr(rng, depth, interner)
            } else {
                // Fall back to unary at low depth
                let sub = gen_expr_date(rng.wrapping_add(1), depth - 1, interner);
                Expr::List(vec![Expr::Sym(interner.intern("dlog")), sub])
            }
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
