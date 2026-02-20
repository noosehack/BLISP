//! Builtin functions for blisp
//!
//! Implements arithmetic, math, and utility functions with
//! dispatch for scalars and columns.

use crate::runtime::Runtime;
use crate::value::Value;
use crate::frame;
use std::sync::Arc;

// Import blawktrust's optimized operations
use blawktrust::builtins::ops::{dlog_column, wstd, wstd0, wzscore};

// Import orientation support
use blawktrust::lookup_ori;

/// Convert Table to TableView automatically
fn ensure_tableview(v: &Value, rt: &Runtime) -> Result<Arc<blawktrust::TableView>, String> {
    match v {
        Value::TableView(tv) => Ok(tv.view_arc()),
        Value::Table(t) => {
            let mut names = Vec::new();
            let mut columns = Vec::new();
            for (sym, col) in &t.columns {
                names.push(rt.interner.resolve(*sym).to_string());
                columns.push(col.clone());
            }
            let bt = blawktrust::Table::new(names, columns);
            Ok(Arc::new(blawktrust::TableView::new(bt)))
        }
        _ => Err(format!("Expected table, got {}", v.type_name())),
    }
}

/// Builtin function signature
pub type BuiltinFn = fn(&mut Runtime, &[Value]) -> Result<Value, String>;

/// Register all builtin functions
pub fn register_builtins(rt: &mut Runtime) {
    // Arithmetic
    rt.register_builtin("+", builtin_add);
    rt.register_builtin("-", builtin_sub);
    rt.register_builtin("*", builtin_mul);
    rt.register_builtin("/", builtin_div);

    // Math
    rt.register_builtin("log", builtin_log);
    rt.register_builtin("exp", builtin_exp);
    rt.register_builtin("abs", builtin_abs);

    // Column Operations (Step 6) - Surface names point to table versions
    rt.register_builtin("dlog", builtin_dlog_cols);   // Table version (default lag=1)
    rt.register_builtin("shift", builtin_shift_cols); // Table version (default lag=1)
    rt.register_builtin("diff", builtin_diff_cols);   // Table version (default lag=1)

    // Single-column kernels (renamed to avoid collision)
    rt.register_builtin("dlog-col", builtin_dlog);
    rt.register_builtin("shift-col", builtin_shift);
    rt.register_builtin("diff-col", builtin_diff);

    // Aggregations (kdb-style)
    rt.register_builtin("sum", builtin_sum);
    rt.register_builtin("sum0", builtin_sum0);
    rt.register_builtin("mean", builtin_mean);
    rt.register_builtin("mean0", builtin_mean0);
    rt.register_builtin("std", builtin_std);
    rt.register_builtin("std0", builtin_std0);

    // I/O Operations (Step 8)
    rt.register_builtin("file", builtin_file);
    rt.register_builtin("file-head", builtin_file_head);
    rt.register_builtin("stdin", builtin_stdin);
    rt.register_builtin("save", builtin_save);
    rt.register_builtin("col", builtin_col);
    rt.register_builtin("setcol", builtin_setcol);
    rt.register_builtin("withcol", builtin_withcol);
    rt.register_builtin("w", builtin_w);
    rt.register_builtin("make-col", builtin_make_col);

    // Table Operations
    rt.register_builtin("cols", builtin_cols);
    rt.register_builtin("select", builtin_select);
    rt.register_builtin("select-num", builtin_select_num);
    rt.register_builtin("map-cols", builtin_map_cols);
    rt.register_builtin("apply-cols", builtin_apply_cols);
    rt.register_builtin("dlog-cols", builtin_dlog_cols);
    rt.register_builtin("shift-cols", builtin_shift_cols);
    rt.register_builtin("diff-cols", builtin_diff_cols);

    // Comparison Operations (GLD_NUM Tier 1)
    rt.register_builtin(">", builtin_gt_cols);      // Surface name → table version
    rt.register_builtin(">-cols", builtin_gt_cols); // Explicit table version
    rt.register_builtin(">-col", builtin_gt);       // Single-column kernel
    rt.register_builtin("<", builtin_lt);
    rt.register_builtin(">=", builtin_gte);
    rt.register_builtin("<=", builtin_lte);
    rt.register_builtin("==", builtin_eq);
    rt.register_builtin("!=", builtin_neq);

    // GLD_NUM Tier 2: Shape/Null Operations
    rt.register_builtin("locf", builtin_locf);
    rt.register_builtin("locf-cols", builtin_locf_cols);
    rt.register_builtin("keep-shape", builtin_keep_shape);
    rt.register_builtin("keep-shape-cols", builtin_keep_shape_cols);

    // GLD_NUM Tier 3: Table Transforms
    rt.register_builtin("w5", builtin_w5);
    rt.register_builtin("xminus", builtin_xminus);
    rt.register_builtin("cs1", builtin_cs1_cols);      // Surface name → table version
    rt.register_builtin("cs1-cols", builtin_cs1_cols); // Explicit table version
    rt.register_builtin("cs1-col", builtin_cs1);       // Single-column kernel
    rt.register_builtin("ecs1", builtin_ecs1_cols);    // Surface name → table version
    rt.register_builtin("ecs1-cols", builtin_ecs1_cols);
    rt.register_builtin("ecs1-col", builtin_ecs1);

    // GLD_NUM Tier 4: Advanced Operations (JOIN, Finance)
    rt.register_builtin("mapr", builtin_mapr);
    rt.register_builtin("ur", builtin_ur_cols);      // Surface name → table version
    rt.register_builtin("ur-cols", builtin_ur_cols); // Explicit table version
    rt.register_builtin("ur-col", builtin_ur);       // Single-column kernel
    rt.register_builtin("wz0", builtin_wz0);
    rt.register_builtin("wz0-cols", builtin_wz0_cols);
    rt.register_builtin("wzs", builtin_wzs);  // Composite: locf(keep-shape(wz0))

    // Rolling Statistics
    rt.register_builtin("wstd", builtin_wstd);
    rt.register_builtin("wstd0", builtin_wstd0);
    rt.register_builtin("wstd-cols", builtin_wstd_cols);
    rt.register_builtin("wstd0-cols", builtin_wstd0_cols);
    rt.register_builtin("wv", builtin_wv);
    rt.register_builtin("wv-cols", builtin_wv_cols);

    // Data Transformations
    rt.register_builtin("zscore", builtin_zscore);
    rt.register_builtin("chop", builtin_chop);

    // Utility
    rt.register_builtin("print", builtin_print);
    rt.register_builtin("type-of", builtin_type_of);
    rt.register_builtin("len", builtin_len);

    // Orientation
    rt.register_builtin("o", builtin_o);
}

// ============================================================================
// Arithmetic Builtins
// ============================================================================

/// (+ a b) - Addition
fn builtin_add(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("+ expects 2 arguments, got {}", args.len()));
    }

    match (&args[0], &args[1]) {
        // Scalar + Scalar
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f64)),

        // Col + Col
        (Value::Col(a), Value::Col(b)) => {
            // Use blawktrust's column addition
            let result = add_columns(a, b)?;
            Ok(Value::Col(Arc::new(result)))
        }

        // Col + Scalar (broadcast)
        (Value::Col(c), Value::Float(s)) => {
            let result = add_column_scalar(c, *s)?;
            Ok(Value::Col(Arc::new(result)))
        }
        (Value::Col(c), Value::Int(s)) => {
            let result = add_column_scalar(c, *s as f64)?;
            Ok(Value::Col(Arc::new(result)))
        }

        // Scalar + Col (broadcast)
        (Value::Float(s), Value::Col(c)) => {
            let result = add_column_scalar(c, *s)?;
            Ok(Value::Col(Arc::new(result)))
        }
        (Value::Int(s), Value::Col(c)) => {
            let result = add_column_scalar(c, *s as f64)?;
            Ok(Value::Col(Arc::new(result)))
        }

        _ => Err(format!("+ cannot add {} and {}", args[0].type_name(), args[1].type_name())),
    }
}

/// (- a b) - Subtraction
fn builtin_sub(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("- expects 2 arguments, got {}", args.len()));
    }

    match (&args[0], &args[1]) {
        // Scalar - Scalar
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 - b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - *b as f64)),

        // Col - Scalar
        (Value::Col(c), Value::Float(s)) => {
            let result = add_column_scalar(c, -*s)?;
            Ok(Value::Col(Arc::new(result)))
        }
        (Value::Col(c), Value::Int(s)) => {
            let result = add_column_scalar(c, -(*s as f64))?;
            Ok(Value::Col(Arc::new(result)))
        }

        _ => Err(format!("- cannot subtract {} and {}", args[0].type_name(), args[1].type_name())),
    }
}

/// (* a b) - Multiplication
fn builtin_mul(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("* expects 2 arguments, got {}", args.len()));
    }

    match (&args[0], &args[1]) {
        // Scalar * Scalar
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * (*b as f64))),

        // Col * Scalar (broadcast)
        (Value::Col(c), Value::Float(s)) => {
            let result = mul_column_scalar(c, *s)?;
            Ok(Value::Col(Arc::new(result)))
        }
        (Value::Col(c), Value::Int(s)) => {
            let result = mul_column_scalar(c, *s as f64)?;
            Ok(Value::Col(Arc::new(result)))
        }

        // Scalar * Col (broadcast)
        (Value::Float(s), Value::Col(c)) => {
            let result = mul_column_scalar(c, *s)?;
            Ok(Value::Col(Arc::new(result)))
        }
        (Value::Int(s), Value::Col(c)) => {
            let result = mul_column_scalar(c, *s as f64)?;
            Ok(Value::Col(Arc::new(result)))
        }

        // Col * Col (element-wise)
        (Value::Col(a), Value::Col(b)) => {
            let result = mul_columns(a, b)?;
            Ok(Value::Col(Arc::new(result)))
        }

        // TableView * TableView (element-wise, column-aligned)
        (Value::TableView(tv_a), Value::TableView(tv_b)) => {
            // Get numeric columns from both tables
            let cols_a: Vec<_> = tv_a.table.columns.iter()
                .filter(|c| matches!(c, blawktrust::Column::F64(_)))
                .collect();
            let cols_b: Vec<_> = tv_b.table.columns.iter()
                .filter(|c| matches!(c, blawktrust::Column::F64(_)))
                .collect();

            if cols_a.len() != cols_b.len() {
                return Err(format!("* cannot multiply tables with different number of numeric columns: {} vs {}",
                    cols_a.len(), cols_b.len()));
            }

            // Multiply corresponding columns
            let mut result_names = Vec::new();
            let mut result_columns = Vec::new();

            for (col_a, col_b) in cols_a.iter().zip(cols_b.iter()) {
                let mul_result = mul_columns(&Arc::new((*col_a).clone()), &Arc::new((*col_b).clone()))?;
                result_columns.push(mul_result);
            }

            // Use names from first table
            for (i, col) in tv_a.table.columns.iter().enumerate() {
                if matches!(col, blawktrust::Column::F64(_)) {
                    result_names.push(tv_a.table.names[i].clone());
                }
            }

            let result_table = blawktrust::Table::new(result_names, result_columns);
            Ok(Value::tableview(blawktrust::TableView::new(result_table)))
        }

        _ => Err(format!("* cannot multiply {} and {}", args[0].type_name(), args[1].type_name())),
    }
}

/// (/ a b) - Division
fn builtin_div(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("/ expects 2 arguments, got {}", args.len()));
    }

    match (&args[0], &args[1]) {
        // Scalar / Scalar
        (Value::Int(a), Value::Int(b)) => {
            if *b == 0 {
                return Err("Division by zero".to_string());
            }
            Ok(Value::Float(*a as f64 / *b as f64))
        }
        (Value::Float(a), Value::Float(b)) => {
            if *b == 0.0 {
                return Err("Division by zero".to_string());
            }
            Ok(Value::Float(a / b))
        }
        (Value::Int(a), Value::Float(b)) => {
            if *b == 0.0 {
                return Err("Division by zero".to_string());
            }
            Ok(Value::Float(*a as f64 / b))
        }
        (Value::Float(a), Value::Int(b)) => {
            if *b == 0 {
                return Err("Division by zero".to_string());
            }
            Ok(Value::Float(a / (*b as f64)))
        }

        // Col / Scalar
        (Value::Col(c), Value::Float(s)) => {
            if *s == 0.0 {
                return Err("Division by zero".to_string());
            }
            let result = mul_column_scalar(c, 1.0 / s)?;
            Ok(Value::Col(Arc::new(result)))
        }
        (Value::Col(c), Value::Int(s)) => {
            if *s == 0 {
                return Err("Division by zero".to_string());
            }
            let result = mul_column_scalar(c, 1.0 / (*s as f64))?;
            Ok(Value::Col(Arc::new(result)))
        }

        _ => Err(format!("/ cannot divide {} and {}", args[0].type_name(), args[1].type_name())),
    }
}

// ============================================================================
// Math Builtins
// ============================================================================

/// (log x) - Natural logarithm
fn builtin_log(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("log expects 1 argument, got {}", args.len()));
    }

    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.ln())),
        Value::Int(n) => Ok(Value::Float((*n as f64).ln())),
        Value::Col(c) => {
            let result = log_column(c)?;
            Ok(Value::Col(Arc::new(result)))
        }
        _ => Err(format!("log cannot operate on {}", args[0].type_name())),
    }
}

/// (exp x) - Exponential (e^x)
fn builtin_exp(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("exp expects 1 argument, got {}", args.len()));
    }

    match &args[0] {
        Value::Float(f) => Ok(Value::Float(f.exp())),
        Value::Int(n) => Ok(Value::Float((*n as f64).exp())),
        Value::Col(c) => {
            let result = exp_column(c)?;
            Ok(Value::Col(Arc::new(result)))
        }
        _ => Err(format!("exp cannot operate on {}", args[0].type_name())),
    }
}

/// (abs x) - Absolute value
fn builtin_abs(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("abs expects 1 argument, got {}", args.len()));
    }

    match &args[0] {
        Value::Int(n) => Ok(Value::Int(n.abs())),
        Value::Float(f) => Ok(Value::Float(f.abs())),
        Value::Col(c) => {
            let result = abs_column(c)?;
            Ok(Value::Col(Arc::new(result)))
        }
        _ => Err(format!("abs cannot operate on {}", args[0].type_name())),
    }
}

// ============================================================================
// Comparison Operations (GLD_NUM Tier 1)
// ============================================================================

/// (> a b) - Greater than comparison
///
/// Returns:
/// - Column: 1.0 where a > b, 0.0 otherwise, NA where either is NA
/// - Scalar: boolean as 1.0/0.0
fn builtin_gt(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("> expects 2 arguments, got {}", args.len()));
    }

    match (&args[0], &args[1]) {
        // Scalar > Scalar
        (Value::Int(a), Value::Int(b)) => Ok(Value::Float(if a > b { 1.0 } else { 0.0 })),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(if a > b { 1.0 } else { 0.0 })),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(if (*a as f64) > *b { 1.0 } else { 0.0 })),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(if a > &(*b as f64) { 1.0 } else { 0.0 })),

        // Col > Scalar
        (Value::Col(col), Value::Int(n)) => {
            let result = compare_column_scalar(col, *n as f64, |a, b| a > b)?;
            Ok(Value::Col(Arc::new(result)))
        }
        (Value::Col(col), Value::Float(f)) => {
            let result = compare_column_scalar(col, *f, |a, b| a > b)?;
            Ok(Value::Col(Arc::new(result)))
        }

        // Col > Col
        (Value::Col(a), Value::Col(b)) => {
            let result = compare_columns(a, b, |x, y| x > y)?;
            Ok(Value::Col(Arc::new(result)))
        }

        _ => Err(format!("> cannot compare {} and {}", args[0].type_name(), args[1].type_name())),
    }
}

/// (< a b) - Less than comparison
fn builtin_lt(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("< expects 2 arguments, got {}", args.len()));
    }

    match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Float(if a < b { 1.0 } else { 0.0 })),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(if a < b { 1.0 } else { 0.0 })),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(if (*a as f64) < *b { 1.0 } else { 0.0 })),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(if a < &(*b as f64) { 1.0 } else { 0.0 })),

        (Value::Col(col), Value::Int(n)) => {
            let result = compare_column_scalar(col, *n as f64, |a, b| a < b)?;
            Ok(Value::Col(Arc::new(result)))
        }
        (Value::Col(col), Value::Float(f)) => {
            let result = compare_column_scalar(col, *f, |a, b| a < b)?;
            Ok(Value::Col(Arc::new(result)))
        }

        (Value::Col(a), Value::Col(b)) => {
            let result = compare_columns(a, b, |x, y| x < y)?;
            Ok(Value::Col(Arc::new(result)))
        }

        _ => Err(format!("< cannot compare {} and {}", args[0].type_name(), args[1].type_name())),
    }
}

/// (>= a b) - Greater than or equal
fn builtin_gte(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!(">= expects 2 arguments, got {}", args.len()));
    }

    match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Float(if a >= b { 1.0 } else { 0.0 })),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(if a >= b { 1.0 } else { 0.0 })),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(if (*a as f64) >= *b { 1.0 } else { 0.0 })),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(if a >= &(*b as f64) { 1.0 } else { 0.0 })),

        (Value::Col(col), Value::Int(n)) => {
            let result = compare_column_scalar(col, *n as f64, |a, b| a >= b)?;
            Ok(Value::Col(Arc::new(result)))
        }
        (Value::Col(col), Value::Float(f)) => {
            let result = compare_column_scalar(col, *f, |a, b| a >= b)?;
            Ok(Value::Col(Arc::new(result)))
        }

        (Value::Col(a), Value::Col(b)) => {
            let result = compare_columns(a, b, |x, y| x >= y)?;
            Ok(Value::Col(Arc::new(result)))
        }

        _ => Err(format!(">= cannot compare {} and {}", args[0].type_name(), args[1].type_name())),
    }
}

/// (<= a b) - Less than or equal
fn builtin_lte(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("<= expects 2 arguments, got {}", args.len()));
    }

    match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Float(if a <= b { 1.0 } else { 0.0 })),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(if a <= b { 1.0 } else { 0.0 })),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(if (*a as f64) <= *b { 1.0 } else { 0.0 })),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(if a <= &(*b as f64) { 1.0 } else { 0.0 })),

        (Value::Col(col), Value::Int(n)) => {
            let result = compare_column_scalar(col, *n as f64, |a, b| a <= b)?;
            Ok(Value::Col(Arc::new(result)))
        }
        (Value::Col(col), Value::Float(f)) => {
            let result = compare_column_scalar(col, *f, |a, b| a <= b)?;
            Ok(Value::Col(Arc::new(result)))
        }

        (Value::Col(a), Value::Col(b)) => {
            let result = compare_columns(a, b, |x, y| x <= y)?;
            Ok(Value::Col(Arc::new(result)))
        }

        _ => Err(format!("<= cannot compare {} and {}", args[0].type_name(), args[1].type_name())),
    }
}

/// (== a b) - Equal comparison
fn builtin_eq(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("== expects 2 arguments, got {}", args.len()));
    }

    match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Float(if a == b { 1.0 } else { 0.0 })),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(if a == b { 1.0 } else { 0.0 })),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(if (*a as f64) == *b { 1.0 } else { 0.0 })),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(if a == &(*b as f64) { 1.0 } else { 0.0 })),

        (Value::Col(col), Value::Int(n)) => {
            let result = compare_column_scalar(col, *n as f64, |a, b| a == b)?;
            Ok(Value::Col(Arc::new(result)))
        }
        (Value::Col(col), Value::Float(f)) => {
            let result = compare_column_scalar(col, *f, |a, b| a == b)?;
            Ok(Value::Col(Arc::new(result)))
        }

        (Value::Col(a), Value::Col(b)) => {
            let result = compare_columns(a, b, |x, y| x == y)?;
            Ok(Value::Col(Arc::new(result)))
        }

        _ => Err(format!("== cannot compare {} and {}", args[0].type_name(), args[1].type_name())),
    }
}

/// (!= a b) - Not equal comparison
fn builtin_neq(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("!= expects 2 arguments, got {}", args.len()));
    }

    match (&args[0], &args[1]) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Float(if a != b { 1.0 } else { 0.0 })),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(if a != b { 1.0 } else { 0.0 })),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float(if (*a as f64) != *b { 1.0 } else { 0.0 })),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(if a != &(*b as f64) { 1.0 } else { 0.0 })),

        (Value::Col(col), Value::Int(n)) => {
            let result = compare_column_scalar(col, *n as f64, |a, b| a != b)?;
            Ok(Value::Col(Arc::new(result)))
        }
        (Value::Col(col), Value::Float(f)) => {
            let result = compare_column_scalar(col, *f, |a, b| a != b)?;
            Ok(Value::Col(Arc::new(result)))
        }

        (Value::Col(a), Value::Col(b)) => {
            let result = compare_columns(a, b, |x, y| x != y)?;
            Ok(Value::Col(Arc::new(result)))
        }

        _ => Err(format!("!= cannot compare {} and {}", args[0].type_name(), args[1].type_name())),
    }
}

/// (>-cols table scalar) - Apply > comparison to all numeric columns
///
/// Table-level wrapper: TableView -> TableView
/// Returns table with 1.0/0.0 masks for all numeric columns
fn builtin_gt_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!(">-cols expects 2 arguments (table scalar), got {}", args.len()));
    }

    let threshold = args[1].as_float()?;

    match &args[0] {
        Value::TableView(tv) => {
            let result = map_numeric_cols(tv.as_ref(), |col| {
                match col {
                    blawktrust::Column::F64(data) => {
                        let result: Vec<f64> = data.iter()
                            .map(|&x| {
                                if x.is_nan() {
                                    f64::NAN
                                } else if x > threshold {
                                    1.0
                                } else {
                                    0.0
                                }
                            })
                            .collect();
                        Ok(blawktrust::Column::new_f64(result))
                    }
                    _ => unreachable!("map_numeric_cols only passes F64"),
                }
            })?;

            Ok(Value::tableview(result))
        }
        _ => Err(format!(">-cols expects TableView, got {}", args[0].type_name())),
    }
}

// ============================================================================
// GLD_NUM Tier 2: Shape and Null Handling
// ============================================================================

/// (locf col) - Last observation carried forward (forward fill)
///
/// Propagates non-NA values forward to fill NA gaps.
/// First value if NA remains NA.
///
/// Example:
///   [1.0, NA, NA, 2.0, NA] → [1.0, 1.0, 1.0, 2.0, 2.0]
///   [NA, 1.0, NA, 2.0]     → [NA, 1.0, 1.0, 2.0]
fn builtin_locf(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("locf expects 1 argument (column), got {}", args.len()));
    }

    let col = args[0].as_col()?;

    match col.as_ref() {
        blawktrust::Column::F64(data) => {
            let mut result = Vec::with_capacity(data.len());
            let mut last_valid = f64::NAN;

            for &val in data {
                if !val.is_nan() {
                    last_valid = val;
                    result.push(val);
                } else {
                    result.push(last_valid);
                }
            }

            Ok(Value::Col(Arc::new(blawktrust::Column::new_f64(result))))
        }
        _ => Err("locf only supported for F64 columns".to_string()),
    }
}

/// (locf-cols table) - Apply locf to all numeric columns
///
/// Table-level wrapper: TableView -> TableView
/// Applies forward fill to each F64 column, preserves non-numeric columns.
fn builtin_locf_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("locf-cols expects 1 argument (table), got {}", args.len()));
    }

    match &args[0] {
        Value::TableView(tv) => {
            let result = map_numeric_cols(tv.as_ref(), |col| {
                // Apply locf logic
                match col {
                    blawktrust::Column::F64(data) => {
                        let mut result = Vec::with_capacity(data.len());
                        let mut last_valid = f64::NAN;

                        for &val in data {
                            if !val.is_nan() {
                                last_valid = val;
                                result.push(val);
                            } else {
                                result.push(last_valid);
                            }
                        }

                        Ok(blawktrust::Column::new_f64(result))
                    }
                    _ => unreachable!("map_numeric_cols only passes F64"),
                }
            })?;

            Ok(Value::tableview(result))
        }
        _ => Err(format!("locf-cols expects TableView, got {}. Use (file ...) which returns TableView.", args[0].type_name())),
    }
}

/// (keep-shape col k) - Keep every kth value, others become NA
///
/// Shape-preserving downsample: keeps values at indices 0, k, 2k, ...
/// All other positions become NA (ready for locf to propagate).
///
/// Example:
///   (keep-shape [10 11 12 13 14 15] 3) → [10 NA NA 13 NA NA]
///
/// Used in wzs macro: (wzs data window step) = (locf (keep-shape (wz0 data window) step))
fn builtin_keep_shape(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("keep-shape expects 2 arguments (col k), got {}", args.len()));
    }

    let col = args[0].as_col()?;
    let k = args[1].as_int()? as usize;

    if k == 0 {
        return Err("keep-shape: k must be > 0".to_string());
    }

    match col.as_ref() {
        blawktrust::Column::F64(data) => {
            let result: Vec<f64> = data.iter().enumerate()
                .map(|(i, &val)| {
                    if i % k == 0 {
                        val
                    } else {
                        f64::NAN
                    }
                })
                .collect();

            Ok(Value::Col(Arc::new(blawktrust::Column::new_f64(result))))
        }
        _ => Err("keep-shape only supported for F64 columns".to_string()),
    }
}

/// (keep-shape-cols table k) - Apply keep-shape to all numeric columns
///
/// Table-level wrapper: TableView -> TableView
fn builtin_keep_shape_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("keep-shape-cols expects 2 arguments (table k), got {}", args.len()));
    }

    let k = args[1].as_int()? as usize;
    if k == 0 {
        return Err("keep-shape-cols: k must be > 0".to_string());
    }

    match &args[0] {
        Value::TableView(tv) => {
            let result = map_numeric_cols(tv.as_ref(), |col| {
                match col {
                    blawktrust::Column::F64(data) => {
                        let result: Vec<f64> = data.iter().enumerate()
                            .map(|(i, &val)| if i % k == 0 { val } else { f64::NAN })
                            .collect();
                        Ok(blawktrust::Column::new_f64(result))
                    }
                    _ => unreachable!("map_numeric_cols only passes F64"),
                }
            })?;

            Ok(Value::tableview(result))
        }
        _ => Err(format!("keep-shape-cols expects TableView, got {}", args[0].type_name())),
    }
}

// ============================================================================
// GLD_NUM Tier 3: Table Transforms
// ============================================================================

/// (w5 table) - Filter table to weekdays only (keep rows where date column is Mon-Fri)
///
/// Uses kdb idiom: (date + 5) % 7 > 1
/// - Epoch (1970-01-01) is Thursday
/// - Adding 5 shifts Thursday→0, so weekdays (Mon-Fri) map to 2-6
/// - Modulo 7 then > 1 selects weekdays
///
/// Assumes first column is Date type with days since epoch.
fn builtin_w5(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("w5 expects 1 argument (table), got {}", args.len()));
    }

    let tv = ensure_tableview(&args[0], rt)?;

    if tv.table.columns.is_empty() {
        return Err("w5: table has no columns".to_string());
    }

    // Get first column (should be Date)
    let date_col = &tv.table.columns[0];

    let date_data = match date_col {
        blawktrust::Column::Date(data) => data,
        _ => return Err("w5: first column must be Date type".to_string()),
    };

    // Build mask: (date + 5) % 7 > 1 (weekdays)
    let mask: Vec<bool> = date_data.iter()
        .map(|&days| {
            if days == blawktrust::NULL_DATE {
                false // Exclude NULL dates
            } else {
                let dow = (days + 5).rem_euclid(7);
                dow > 1
            }
        })
        .collect();

    // Filter all columns
    let new_names: Vec<String> = tv.table.names.clone();
    let new_columns: Vec<blawktrust::Column> = tv.table.columns.iter()
        .map(|col| filter_column(col, &mask))
        .collect();

    let new_table = blawktrust::Table::new(new_names, new_columns);
    Ok(Value::tableview(blawktrust::TableView::new(new_table)))
}

/// (xminus table half) - Pairwise spreads (A - B for all pairs of numeric columns)
///
/// If half=1, computes all pairs in first half minus second half of NUMERIC columns:
///   Numeric columns [A, B, C, D] with half=1 → [A-C, A-D, B-C, B-D]
///   Non-numeric columns (Date, Timestamp) are ignored
///
/// Column naming: "A\B" means A minus B
fn builtin_xminus(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("xminus expects 2 arguments (table half), got {}", args.len()));
    }

    let tv = ensure_tableview(&args[0], rt)?;
    let half = args[1].as_int()? as usize;

    if half != 1 {
        return Err("xminus: only half=1 currently supported".to_string());
    }

    // Extract only numeric columns
    let mut numeric_names = Vec::new();
    let mut numeric_cols = Vec::new();
    for (i, col) in tv.table.columns.iter().enumerate() {
        if matches!(col, blawktrust::Column::F64(_)) {
            numeric_names.push(tv.table.names[i].clone());
            numeric_cols.push(col);
        }
    }

    let ncols = numeric_cols.len();
    if ncols % 2 != 0 {
        return Err(format!("xminus: expected even number of numeric columns, got {}", ncols));
    }

    let mid = ncols / 2;
    let mut new_names = Vec::new();
    let mut new_columns = Vec::new();

    // Preserve first column if it's a Date column (for mapr join key)
    if !tv.table.columns.is_empty() {
        if matches!(tv.table.columns[0], blawktrust::Column::Date(_)) {
            new_names.push(tv.table.names[0].clone());
            new_columns.push(tv.table.columns[0].clone());
        }
    }

    // Compute all pairs: first_half - second_half
    for i in 0..mid {
        for j in mid..ncols {
            let col_a = numeric_cols[i];
            let col_b = numeric_cols[j];

            // Compute A - B
            let result_col = subtract_columns_pair(col_a, col_b)?;

            // Name: A\B
            let name = format!("{}\\{}", numeric_names[i], numeric_names[j]);
            new_names.push(name);
            new_columns.push(result_col);
        }
    }

    let new_table = blawktrust::Table::new(new_names, new_columns);
    Ok(Value::tableview(blawktrust::TableView::new(new_table)))
}

/// (cs1 col) - Cumulative sum starting from 1.0
///
/// Converts differences/returns to levels starting at 1.0:
///   diffs = [0.01, -0.02, 0.03]
///   cs1   = [1.01, 0.99, 1.02]
///
/// Formula: result[i] = 1.0 + sum(diffs[0..=i])
fn builtin_cs1(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("cs1 expects 1 argument (column), got {}", args.len()));
    }

    let col = args[0].as_col()?;

    match col.as_ref() {
        blawktrust::Column::F64(data) => {
            let mut result = Vec::with_capacity(data.len());
            let mut sum = 1.0;

            for &val in data {
                if val.is_nan() {
                    // Carry forward last cumsum (like locf), don't output NaN
                    result.push(sum);
                } else {
                    sum += val;
                    result.push(sum);
                }
            }

            Ok(Value::Col(Arc::new(blawktrust::Column::new_f64(result))))
        }
        _ => Err("cs1 only supported for F64 columns".to_string()),
    }
}

/// (cs1-cols table) - Cumulative sum starting from 1.0, axis-aware
///
/// axis=:col (default) → cumulative down rows per column
/// axis=:row → cumulative across columns per row
fn builtin_cs1_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("cs1-cols expects 1 argument (table), got {}", args.len()));
    }

    match &args[0] {
        Value::TableView(tv) => {
            use crate::value::Axis;

            match tv.axis {
                Axis::Col => {
                    // Cumulative down rows, per column (fast path)
                    let result = map_numeric_cols(tv.as_ref(), |col| {
                        match col {
                            blawktrust::Column::F64(data) => {
                                let mut result = Vec::with_capacity(data.len());
                                let mut sum = 1.0;

                                for &val in data {
                                    if val.is_nan() {
                                        result.push(sum);  // LOCF
                                    } else {
                                        sum += val;
                                        result.push(sum);
                                    }
                                }

                                Ok(blawktrust::Column::new_f64(result))
                            }
                            _ => unreachable!("map_numeric_cols only passes F64"),
                        }
                    })?;

                    Ok(Value::tableview(result))
                }
                Axis::Row => {
                    // Cumulative across columns, per row (slow path)
                    let nrows = tv.table.columns.get(0).map(|c| c.len()).unwrap_or(0);
                    let ncols = tv.table.columns.len();

                    // Collect numeric column indices
                    let numeric_indices: Vec<usize> = tv.table.columns.iter().enumerate()
                        .filter_map(|(i, col)| if matches!(col, blawktrust::Column::F64(_)) { Some(i) } else { None })
                        .collect();

                    if numeric_indices.is_empty() {
                        return Err("cs1-cols: no numeric columns".to_string());
                    }

                    // Build output: one column per numeric input column
                    let mut result_cols = Vec::new();
                    let mut result_names = Vec::new();

                    for &col_idx in &numeric_indices {
                        let mut col_data = vec![0.0; nrows];

                        // For each row, scan across columns up to this one
                        if let blawktrust::Column::F64(data) = &tv.table.columns[col_idx] {
                            for row in 0..nrows {
                                let mut cumsum = 1.0;
                                for &nc_idx in &numeric_indices {
                                    if nc_idx > col_idx {
                                        break;  // Haven't reached this column yet in the scan
                                    }
                                    if let blawktrust::Column::F64(nc_data) = &tv.table.columns[nc_idx] {
                                        let val = nc_data[row];
                                        if !val.is_nan() {
                                            cumsum += val;
                                        }
                                    }
                                }
                                col_data[row] = cumsum;
                            }
                        }

                        result_cols.push(blawktrust::Column::new_f64(col_data));
                        result_names.push(tv.table.names[col_idx].clone());
                    }

                    let result_table = blawktrust::Table::new(result_names, result_cols);
                    Ok(Value::tableview(blawktrust::TableView::new(result_table)))
                }
            }
        }
        _ => Err(format!("cs1-cols expects TableView, got {}", args[0].type_name())),
    }
}

/// (ecs1 col) - Exponential cumulative sum: exp(cumsum(log_returns))
///
/// Converts log returns back to price index / cumulative product.
/// Formula: exp(sum of log returns) = cumulative product of exp(returns)
fn builtin_ecs1(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("ecs1 expects 1 argument (column), got {}", args.len()));
    }

    let col = args[0].as_col()?;

    match col.as_ref() {
        blawktrust::Column::F64(data) => {
            let mut result = Vec::with_capacity(data.len());
            let mut cumsum: f64 = 0.0;

            for &val in data {
                if val.is_nan() {
                    // Carry forward last value (preserves index level)
                    result.push(cumsum.exp());
                } else {
                    cumsum += val;
                    result.push(cumsum.exp());
                }
            }

            Ok(Value::Col(Arc::new(blawktrust::Column::new_f64(result))))
        }
        _ => Err("ecs1 only supported for F64 columns".to_string()),
    }
}

/// (ecs1-cols table) - Exponential cumulative sum, axis-aware
///
/// axis=:col (default) → cumulative down rows per column
/// axis=:row → cumulative across columns per row
fn builtin_ecs1_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("ecs1-cols expects 1 argument (table), got {}", args.len()));
    }

    match &args[0] {
        Value::TableView(tv) => {
            use crate::value::Axis;

            match tv.axis {
                Axis::Col => {
                    // Cumulative down rows, per column (fast path)
                    let result = map_numeric_cols(tv.as_ref(), |col| {
                        match col {
                            blawktrust::Column::F64(data) => {
                                let mut result = Vec::with_capacity(data.len());
                                let mut cumsum: f64 = 0.0;

                                for &val in data {
                                    if val.is_nan() {
                                        result.push(cumsum.exp());  // LOCF
                                    } else {
                                        cumsum += val;
                                        result.push(cumsum.exp());
                                    }
                                }

                                Ok(blawktrust::Column::new_f64(result))
                            }
                            _ => unreachable!("map_numeric_cols only passes F64"),
                        }
                    })?;

                    Ok(Value::tableview(result))
                }
                Axis::Row => {
                    // Cumulative across columns, per row (slow path)
                    let nrows = tv.table.columns.get(0).map(|c| c.len()).unwrap_or(0);

                    // Collect numeric column indices
                    let numeric_indices: Vec<usize> = tv.table.columns.iter().enumerate()
                        .filter_map(|(i, col)| if matches!(col, blawktrust::Column::F64(_)) { Some(i) } else { None })
                        .collect();

                    if numeric_indices.is_empty() {
                        return Err("ecs1-cols: no numeric columns".to_string());
                    }

                    // Build output: one column per numeric input column
                    let mut result_cols = Vec::new();
                    let mut result_names = Vec::new();

                    for &col_idx in &numeric_indices {
                        let mut col_data = vec![0.0; nrows];

                        if let blawktrust::Column::F64(_data) = &tv.table.columns[col_idx] {
                            for row in 0..nrows {
                                let mut cumsum = 0.0;
                                for &nc_idx in &numeric_indices {
                                    if nc_idx > col_idx {
                                        break;
                                    }
                                    if let blawktrust::Column::F64(nc_data) = &tv.table.columns[nc_idx] {
                                        let val = nc_data[row];
                                        if !val.is_nan() {
                                            cumsum += val;
                                        }
                                    }
                                }
                                col_data[row] = cumsum.exp();
                            }
                        }

                        result_cols.push(blawktrust::Column::new_f64(col_data));
                        result_names.push(tv.table.names[col_idx].clone());
                    }

                    let result_table = blawktrust::Table::new(result_names, result_cols);
                    Ok(Value::tableview(blawktrust::TableView::new(result_table)))
                }
            }
        }
        _ => Err(format!("ecs1-cols expects TableView, got {}", args[0].type_name())),
    }
}

// ============================================================================
// GLD_NUM Tier 4: Advanced Operations (JOIN, Finance)
// ============================================================================

/// (mapr target source) - Map target data to source row structure
///
/// Reshapes target to have the same rows as source (INNER JOIN semantics).
/// - Result has source's row structure (dates/keys)
/// - Target data columns mapped to source rows
/// - O(1) lookup using HashMap
///
/// Semantics: "Give me target's data, but only for rows that exist in source"
///
/// Example:
///   target: [date: 2024-01-01, price: 100]
///           [date: 2024-01-02, price: 101]
///           [date: 2024-01-03, price: 102]
///   source: [date: 2024-01-01, signal: 1.5]
///           [date: 2024-01-03, signal: 2.1]
///   result: [date: 2024-01-01, price: 100]
///           [date: 2024-01-03, price: 102]
fn builtin_mapr(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("mapr expects 2 arguments (target source), got {}", args.len()));
    }

    let target = ensure_tableview(&args[0], rt)?;
    let source = ensure_tableview(&args[1], rt)?;

    if target.table.columns.is_empty() || source.table.columns.is_empty() {
        return Err("mapr: both tables must have at least one column".to_string());
    }

    // Build HashMap from target first column (keys) to row indices
    let target_keys = &target.table.columns[0];
    use std::collections::HashMap;
    let mut key_map: HashMap<String, usize> = HashMap::new();

    match target_keys {
        blawktrust::Column::Date(data) => {
            for (i, &key) in data.iter().enumerate() {
                key_map.insert(key.to_string(), i);
            }
        }
        blawktrust::Column::F64(data) => {
            for (i, &key) in data.iter().enumerate() {
                key_map.insert(key.to_string(), i);
            }
        }
        _ => return Err("mapr: first column must be Date or F64".to_string()),
    }

    // Prepare result columns (use source's first column as row keys)
    let mut result_names = vec![source.table.names[0].clone()];
    let mut result_columns = vec![source.table.columns[0].clone()];

    // For each target data column (skip first, it's the key)
    for j in 1..target.table.columns.len() {
        let target_col = &target.table.columns[j];

        // Map target column to source keys
        let mapped_col = map_column_by_keys(&source.table.columns[0], target_col, &key_map)?;

        result_names.push(target.table.names[j].clone());
        result_columns.push(mapped_col);
    }

    let result_table = blawktrust::Table::new(result_names, result_columns);
    Ok(Value::tableview(blawktrust::TableView::new(result_table)))
}

/// (ur col window [step]) - Unit ratio: value / rolling_volatility (step default=1)
///
/// Formula: value / (100 * sqrt(252) * rolling_stddev)
/// - Uses rolling standard deviation with given window
/// - Excludes zeros from stddev calculation
/// - Returns NA where stddev is zero or NA
///
/// Example:
///   (ur prices 250 5) ; 250-day window, decay=5
fn builtin_ur(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err(format!("ur expects 2-3 arguments (col window [step]), got {}", args.len()));
    }

    let col = args[0].as_col()?;
    let window = args[1].as_int()? as usize;
    let step = if args.len() == 3 {
        args[2].as_int()? as usize
    } else {
        1
    };

    if window == 0 {
        return Err("ur: window must be > 0".to_string());
    }

    if step == 0 {
        return Err("ur: step must be > 0".to_string());
    }

    match col.as_ref() {
        blawktrust::Column::F64(data) => {
            let n = data.len();
            let mut result = vec![f64::NAN; n];
            let scale = 100.0 * (252.0_f64).sqrt();

            // Step parameter: recalculate volatility every 'step' rows, but output every row
            let mut current_inv_vol = f64::NAN;  // 1 / (scale * stddev)

            for i in 0..n {
                // Rolling window philosophy: require full window before computing
                if i < window {
                    result[i] = f64::NAN;
                    continue;
                }

                // Recalculate volatility every 'step' rows
                if i % step == 0 || current_inv_vol.is_nan() {
                    let start = i - window;
                    let end = i;  // EXCLUDE current value (Ft-measurable: use only past)

                    // Calculate rolling stddev
                    let mut sum = 0.0;
                    let mut sum_sq = 0.0;
                    let mut count = 0;

                    for j in start..end {
                        let val = data[j];
                        if !val.is_nan() && val != 0.0 {
                            sum += val;
                            sum_sq += val * val;
                            count += 1;
                        }
                    }

                    if count > 1 {
                        let variance = (sum_sq - sum * sum / count as f64) / (count - 1) as f64;
                        if variance > 0.0 {
                            let stddev = variance.sqrt();
                            current_inv_vol = 1.0 / (scale * stddev);
                        } else {
                            current_inv_vol = f64::NAN;
                        }
                    } else {
                        current_inv_vol = f64::NAN;
                    }
                }

                // Apply current volatility coefficient to every row
                if !data[i].is_nan() && !current_inv_vol.is_nan() {
                    result[i] = data[i] * current_inv_vol;
                } else {
                    result[i] = f64::NAN;
                }
            }

            Ok(Value::Col(Arc::new(blawktrust::Column::new_f64(result))))
        }
        _ => Err("ur only supported for F64 columns".to_string()),
    }
}

/// (ur-cols table window [step]) - Apply ur to all numeric columns (step default=1)
///
/// Table-level wrapper: TableView -> TableView
fn builtin_ur_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err(format!("ur-cols expects 2-3 arguments (table window [step]), got {}", args.len()));
    }

    let window = args[1].as_int()? as usize;
    let step = if args.len() == 3 {
        args[2].as_int()? as usize
    } else {
        1
    };

    if window == 0 {
        return Err("ur-cols: window must be > 0".to_string());
    }

    if step == 0 {
        return Err("ur-cols: step must be > 0".to_string());
    }

    match &args[0] {
        Value::TableView(tv) => {
            let result = map_numeric_cols(tv.as_ref(), |col| {
                match col {
                    blawktrust::Column::F64(data) => {
                        let n = data.len();
                        let mut result = vec![f64::NAN; n];
                        let scale = 100.0 * (252.0_f64).sqrt();

                        // Step parameter: recalculate volatility every 'step' rows, but output every row
                        let mut current_inv_vol = f64::NAN;  // 1 / (scale * stddev)

                        for i in 0..n {
                            // Rolling window philosophy: require full window before computing
                            if i < window {
                                result[i] = f64::NAN;
                                continue;
                            }

                            // Recalculate volatility every 'step' rows
                            if i % step == 0 || current_inv_vol.is_nan() {
                                let start = i - window;
                                let end = i;

                                let mut sum = 0.0;
                                let mut sum_sq = 0.0;
                                let mut count = 0;

                                for j in start..end {
                                    let val = data[j];
                                    if !val.is_nan() && val != 0.0 {
                                        sum += val;
                                        sum_sq += val * val;
                                        count += 1;
                                    }
                                }

                                if count > 1 {
                                    let variance = (sum_sq - sum * sum / count as f64) / (count - 1) as f64;
                                    if variance > 0.0 {
                                        let stddev = variance.sqrt();
                                        current_inv_vol = 1.0 / (scale * stddev);
                                    } else {
                                        current_inv_vol = f64::NAN;
                                    }
                                } else {
                                    current_inv_vol = f64::NAN;
                                }
                            }

                            // Apply current volatility coefficient to every row
                            if !data[i].is_nan() && !current_inv_vol.is_nan() {
                                result[i] = data[i] * current_inv_vol;
                            } else {
                                result[i] = f64::NAN;
                            }
                        }

                        Ok(blawktrust::Column::new_f64(result))
                    }
                    _ => unreachable!("map_numeric_cols only passes F64"),
                }
            })?;

            Ok(Value::tableview(result))
        }
        _ => Err(format!("ur-cols expects TableView, got {}", args[0].type_name())),
    }
}

/// (wz0 col window) - Rolling z-score
///
/// Formula: (value - rolling_mean) / rolling_stddev
/// - Uses rolling window for mean and stddev
/// - Returns NA where stddev is zero or NA
///
/// Example:
///   (wz0 prices 25) ; 25-period rolling z-score
fn builtin_wz0(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("wz0 expects 2 arguments (col window), got {}", args.len()));
    }

    let col = args[0].as_col()?;
    let window = args[1].as_int()? as usize;

    if window == 0 {
        return Err("wz0: window must be > 0".to_string());
    }

    match col.as_ref() {
        blawktrust::Column::F64(data) => {
            let n = data.len();
            let mut result = vec![f64::NAN; n];

            for i in 0..n {
                // Match clispi: use window [i-window, i) excluding current value
                // Then store z-score at position i+1 (one position later)
                // Use sample variance (ddof=1, Bessel's correction)
                if i >= window && i + 1 < n {
                    let start = i - window;
                    let end = i;

                    // Calculate rolling mean and stddev
                    let mut sum = 0.0;
                    let mut sum_sq = 0.0;
                    let mut count = 0;

                    for j in start..end {
                        let val = data[j];
                        if !val.is_nan() {
                            sum += val;
                            sum_sq += val * val;
                            count += 1;
                        }
                    }

                    if count > 1 {
                        let mean = sum / count as f64;
                        // Use sample variance (ddof=1) to match clispi
                        let variance = (sum_sq - sum * sum / count as f64) / (count - 1) as f64;

                        if variance > 0.0 && !data[i].is_nan() {
                            let stddev = variance.sqrt();
                            result[i + 1] = (data[i] - mean) / stddev;
                        }
                    }
                }
            }

            Ok(Value::Col(Arc::new(blawktrust::Column::new_f64(result))))
        }
        _ => Err("wz0 only supported for F64 columns".to_string()),
    }
}

/// (wz0-cols table window) - Apply wz0 to all numeric columns
///
/// Table-level wrapper: TableView -> TableView
fn builtin_wz0_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("wz0-cols expects 2 arguments (table window), got {}", args.len()));
    }

    let window = args[1].as_int()? as usize;
    if window == 0 {
        return Err("wz0-cols: window must be > 0".to_string());
    }

    match &args[0] {
        Value::TableView(tv) => {
            let result = map_numeric_cols(tv.as_ref(), |col| {
                match col {
                    blawktrust::Column::F64(data) => {
                        let n = data.len();
                        let mut result = vec![f64::NAN; n];

                        for i in 0..n {
                            // Match clispi: use window [i-window, i) excluding current value
                            // Then store z-score at position i+1 (one position later)
                            // Use sample variance (ddof=1, Bessel's correction)
                            if i >= window && i + 1 < n {
                                let start = i - window;
                                let end = i;

                                let mut sum = 0.0;
                                let mut sum_sq = 0.0;
                                let mut count = 0;

                                for j in start..end {
                                    let val = data[j];
                                    if !val.is_nan() {
                                        sum += val;
                                        sum_sq += val * val;
                                        count += 1;
                                    }
                                }

                                if count > 1 {
                                    let mean = sum / count as f64;
                                    let variance = (sum_sq - sum * sum / count as f64) / (count - 1) as f64;

                                    if variance > 0.0 && !data[i].is_nan() {
                                        let stddev = variance.sqrt();
                                        result[i] = (data[i] - mean) / stddev;
                                    }
                                }
                            }
                        }

                        Ok(blawktrust::Column::new_f64(result))
                    }
                    _ => unreachable!("map_numeric_cols only passes F64"),
                }
            })?;

            Ok(Value::tableview(result))
        }
        _ => Err(format!("wz0-cols expects TableView, got {}", args[0].type_name())),
    }
}

/// (wzs table window step) - Rolling z-score with keep-shape and locf
///
/// Composition: locf(keep-shape(wz0(table, window), step))
/// This is the standard finance pipeline for z-score signals
fn builtin_wzs(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 3 {
        return Err(format!("wzs expects 3 arguments (table window step), got {}", args.len()));
    }

    // Step 1: wz0-cols
    let wz0_result = builtin_wz0_cols(rt, &[args[0].clone(), args[1].clone()])?;

    // Step 2: keep-shape-cols
    let keep_result = builtin_keep_shape_cols(rt, &[wz0_result, args[2].clone()])?;

    // Step 3: locf-cols
    builtin_locf_cols(rt, &[keep_result])
}

// ============================================================================
// Column Operations (Step 6) - Using blawktrust kernels
// ============================================================================

/// (dlog col lag) - Log returns using optimized blawktrust kernel
///
/// Computes: log(x[i]) - log(x[i-lag])
///
/// This uses blawktrust's optimized dlog kernel which is ~1.89x faster than C++.
/// Performance: 15.51 ms for 1M elements (vs 29.33 ms C++).
///
/// Example:
///   (dlog prices 1)  ; Daily log returns
///   (dlog prices 5)  ; 5-day log returns
fn builtin_dlog(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("dlog expects 2 arguments (frame/col lag), got {}", args.len()));
    }

    let lag = args[1].as_int()? as usize;

    // Dispatch: Frame or Column
    match &args[0] {
        Value::Frame(f) => {
            // BLADE: Use map_numeric_preserve_tags (Blueprint Phase 1)
            // Invariants: I1 (tags.index Arc), I2 (tags.colnames Arc), I3 (nrows)
            let result = frame::map_numeric_preserve_tags(f, |col| {
                dlog_column(col, lag)
            });
            Ok(Value::Frame(Arc::new(result)))
        }
        Value::Col(col) => {
            // Single column version
            let result = dlog_column(col, lag);
            Ok(Value::Col(Arc::new(result)))
        }
        _ => Err(format!("dlog expects frame or col, got {}", args[0].type_name())),
    }
}

/// (shift col lag) - Shift/lag column values
///
/// Returns column shifted by lag positions. First lag elements become NA.
///
/// Example:
///   (shift prices 1)   ; Yesterday's prices
///   (shift prices -1)  ; Tomorrow's prices (lead)
fn builtin_shift(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("shift expects 2 arguments (col lag), got {}", args.len()));
    }

    let col = args[0].as_col()?;
    let lag = args[1].as_int()?;

    if lag < 0 {
        return Err("shift with negative lag not yet implemented".to_string());
    }

    // Use Column's shift method
    let result = shift_column(&col, lag as usize)?;
    Ok(Value::Col(Arc::new(result)))
}

/// (diff col lag) - Difference operator
///
/// Computes: x[i] - x[i-lag]
///
/// Example:
///   (diff prices 1)  ; Daily price changes
///   (diff prices 5)  ; 5-day price changes
fn builtin_diff(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("diff expects 2 arguments (col lag), got {}", args.len()));
    }

    let col = args[0].as_col()?;
    let lag = args[1].as_int()? as usize;

    // Compute: col - shift(col, lag)
    let shifted = shift_column(&col, lag)?;
    let result = subtract_columns(&col, &shifted)?;
    Ok(Value::Col(Arc::new(result)))
}

// ============================================================================
// I/O Operations (Step 8)
// ============================================================================

/// (file "filename.csv") - Load CSV file into Table
///
/// Example:
///   (file "GC1C.csv")         ; Load gold futures data
///   (file "ES1I.csv")         ; Load S&P futures data
fn builtin_file(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("file expects 1 argument (filename), got {}", args.len()));
    }

    let filename = match &args[0] {
        Value::Str(s) => s.as_ref(),
        _ => return Err(format!("file expects string filename, got {}", args[0].type_name())),
    };

    crate::io::load_csv(filename, &mut rt.interner)
}

/// (file-head "filename.csv" n) - Load first n rows from CSV (preview mode)
///
/// Fast path for display/pipelines: only parses header + first n data rows.
/// Much faster than (file) for large CSVs when you only need a preview.
///
/// Example:
///   (file-head "At.csv" 10)    ; Load only first 10 rows
///   (file-head "data.csv" 1)   ; Peek at first row
fn builtin_file_head(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("file-head expects 2 arguments (filename n), got {}", args.len()));
    }

    let filename = match &args[0] {
        Value::Str(s) => s.as_ref(),
        _ => return Err(format!("file-head expects string filename, got {}", args[0].type_name())),
    };

    let n = args[1].as_int()?;
    if n < 0 {
        return Err("file-head expects non-negative row limit".to_string());
    }

    crate::io::load_csv_limit(filename, &mut rt.interner, n as usize)
}

/// (stdin) - Read CSV from stdin into Table
///
/// Example:
///   cat prices.csv | ./blisp -e "(dlog (stdin) 1)"
fn builtin_stdin(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if !args.is_empty() {
        return Err(format!("stdin expects 0 arguments, got {}", args.len()));
    }

    crate::io::load_stdin(&mut rt.interner)
}

/// (save "filename.csv" table) - Save Table to CSV file
///
/// Example:
///   (save "output.csv" results)
fn builtin_save(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("save expects 2 arguments (filename table), got {}", args.len()));
    }

    let filename = match &args[0] {
        Value::Str(s) => s.as_ref(),
        _ => return Err(format!("save expects string filename, got {}", args[0].type_name())),
    };

    match &args[1] {
        Value::TableView(tv) => {
            // Convert TableView to blisp Table for save_csv
            let mut table = crate::value::Table::new();
            for (i, name) in tv.table.names.iter().enumerate() {
                let sym = rt.interner.intern(name);
                table.add_column(sym, tv.table.columns[i].clone());
            }
            crate::io::save_csv(filename, &table, &rt.interner)?;
            Ok(Value::Nil)
        }
        Value::Table(t) => {
            crate::io::save_csv(filename, t, &rt.interner)?;
            Ok(Value::Nil)
        }
        _ => Err(format!("save expects table or tableview, got {}", args[1].type_name())),
    }
}

/// (col table 'colname) - Extract column from table by name
///
/// Example:
///   (col prices 'px)          ; Extract 'px' column
///   (col data 'volume)        ; Extract 'volume' column
fn builtin_col(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("col expects 2 arguments (table colname), got {}", args.len()));
    }

    let tv = ensure_tableview(&args[0], rt)?;

    let col_name = match &args[1] {
        Value::Sym(id) => rt.interner.resolve(*id).to_string(),
        Value::Str(s) => s.to_string(),
        _ => return Err(format!("col expects symbol or string column name, got {}", args[1].type_name())),
    };

    // Find column by name
    let col_idx = tv.table.names.iter().position(|n| n == &col_name)
        .ok_or_else(|| format!("Column '{}' not found in table", col_name))?;

    Ok(Value::Col(Arc::new(tv.table.columns[col_idx].clone())))
}

/// (w table index) - Extract column from table by index (0-based)
///
/// Example:
///   (w prices 0)              ; First column
///   (w prices 1)              ; Second column
///   (w5 prices)               ; Alias for (w prices 5)
fn builtin_w(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("w expects 2 arguments (table index), got {}", args.len()));
    }

    let table = args[0].as_table()?;
    let index = args[1].as_int()? as usize;

    if index >= table.columns.len() {
        return Err(format!(
            "Column index {} out of bounds (table has {} columns)",
            index,
            table.columns.len()
        ));
    }

    let (_, col) = &table.columns[index];
    Ok(Value::Col(Arc::new(col.clone())))
}

/// (setcol table "colname" column) → TableView
///
/// Replace or add a column to a table, returning a new TableView.
/// This is the primary way to update table columns in the TableView-only runtime.
///
/// Example:
///   (setcol prices "log_price" (dlog (col prices "price")))
fn builtin_setcol(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 3 {
        return Err(format!("setcol expects 3 arguments (table colname column), got {}", args.len()));
    }

    // Get table as TableView
    let tv = ensure_tableview(&args[0], rt)?;

    // Get column name
    let col_name = match &args[1] {
        Value::Sym(id) => rt.interner.resolve(*id).to_string(),
        Value::Str(s) => s.to_string(),
        _ => return Err(format!("setcol expects symbol or string column name, got {}", args[1].type_name())),
    };

    // Get new column
    let new_col = args[2].as_col()?;

    // Validate lengths match existing table
    if tv.table.row_count() > 0 && new_col.len() != tv.table.row_count() {
        return Err(format!(
            "Column length mismatch: table has {} rows, column has {}",
            tv.table.row_count(),
            new_col.len()
        ));
    }

    // Build new table with updated column
    let mut names = Vec::new();
    let mut columns = Vec::new();
    let mut replaced = false;

    for (i, name) in tv.table.names.iter().enumerate() {
        if name == &col_name {
            // Replace existing column
            names.push(col_name.clone());
            columns.push(new_col.as_ref().clone());
            replaced = true;
        } else {
            // Keep existing column
            names.push(name.clone());
            columns.push(tv.table.columns[i].clone());
        }
    }

    // If column not found, append it
    if !replaced {
        names.push(col_name.clone());
        columns.push(new_col.as_ref().clone());
    }

    let new_bt = blawktrust::Table::new(names, columns);
    Ok(Value::tableview(blawktrust::TableView::new(new_bt)))
}

/// (withcol table "colname" fn [args...]) → TableView
///
/// Apply a builtin function to a column and replace it in the table.
/// Equivalent to (setcol table colname (fn (col table colname) args...))
///
/// This enforces explicit column boundaries: the function operates on a Column
/// and returns a Column. The threading macro does NOT extract columns implicitly.
///
/// Currently supports only builtin functions (not lambdas).
///
/// Example:
///   (withcol prices "close" dlog)                ; Replace close with dlog(close)
///   (withcol prices "close" shift 2)             ; Replace close with shift(close, 2)
///   (-> prices (withcol "close" dlog))           ; Same with threading
fn builtin_withcol(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() < 3 {
        return Err(format!("withcol expects at least 3 arguments (table colname fn [args...]), got {}", args.len()));
    }

    // Get table as TableView
    let tv = ensure_tableview(&args[0], rt)?;

    // Get column name
    let col_name_sym = match &args[1] {
        Value::Sym(id) => *id,
        Value::Str(s) => rt.interner.intern(s.as_ref()),
        _ => return Err(format!("withcol expects symbol or string column name, got {}", args[1].type_name())),
    };

    // Get function (must be a builtin symbol)
    let func_sym = match &args[2] {
        Value::Sym(id) => *id,
        _ => return Err(format!("withcol currently only supports builtin function symbols, got {}", args[2].type_name())),
    };

    if !rt.is_builtin(func_sym) {
        let func_name = rt.interner.resolve(func_sym);
        return Err(format!("withcol: '{}' is not a builtin function", func_name));
    }

    // Extract column
    let col_name = rt.interner.resolve(col_name_sym).to_string();
    let col_idx = tv.table.names.iter().position(|n| n == &col_name)
        .ok_or_else(|| format!("Column '{}' not found in table", col_name))?;
    let existing_col = &tv.table.columns[col_idx];

    // Build args for function call: [column, extra_args...]
    let mut func_args = vec![Value::Col(Arc::new(existing_col.clone()))];
    func_args.extend_from_slice(&args[3..]);

    // Call builtin function
    let result = rt.call_builtin(func_sym, &func_args)?;

    // Extract resulting column
    let new_col = result.as_col()?;

    // Use setcol to update the table
    let col_name_val = Value::Str(col_name.into());
    builtin_setcol(rt, &[Value::tableview_arc(tv), col_name_val, Value::Col(new_col)])
}

/// (make-col v1 v2 v3 ...) - Create column from values
///
/// Example:
///   (make-col 10.0 12.0 15.0)     ; Create column with 3 values
fn builtin_make_col(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.is_empty() {
        return Err("make-col expects at least 1 argument".to_string());
    }

    let mut values = Vec::new();
    for arg in args {
        let val = arg.as_float()?;
        values.push(val);
    }

    let col = blawktrust::Column::new_f64(values);
    Ok(Value::Col(Arc::new(col)))
}

// ============================================================================
// Utility Builtins
// ============================================================================

/// (print ...) - Print values
fn builtin_print(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            print!(" ");
        }
        print!("{}", arg.display(&rt.interner));
    }
    println!();
    Ok(Value::Nil)
}

/// (type-of x) - Get type name
fn builtin_type_of(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("type-of expects 1 argument, got {}", args.len()));
    }
    let type_name = args[0].type_name();
    Ok(Value::Str(type_name.into()))
}

/// (len x) - Get length
fn builtin_len(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("len expects 1 argument, got {}", args.len()));
    }

    match &args[0] {
        Value::Col(c) => Ok(Value::Int(c.len() as i64)),
        Value::Table(t) => Ok(Value::Int(t.row_count as i64)),
        Value::Str(s) => Ok(Value::Int(s.len() as i64)),
        _ => Err(format!("len cannot get length of {}", args[0].type_name())),
    }
}

// ============================================================================
// Aggregation Builtins (kdb-style)
// ============================================================================

/// (sum table) - Sum values according to axis
///
/// Axis-aware aggregation:
///   axis=:col (default) → sum down rows, per column → 1×N output
///   axis=:row → sum across columns, per row → M×1 output
fn builtin_sum(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("sum expects 1 argument, got {}", args.len()));
    }

    match &args[0] {
        Value::Col(col) => {
            // Single column: compute sum ignoring NAs
            let data = match col.as_ref() {
                blawktrust::Column::F64(data) => data,
                _ => return Err("sum expects F64 column".to_string()),
            };
            let mut sum = 0.0;
            for &val in data {
                if !val.is_nan() {
                    sum += val;
                }
            }
            Ok(Value::Float(sum))
        }
        Value::TableView(tv) => {
            use crate::value::Axis;

            // Axis-aware aggregation
            match tv.axis {
                Axis::Col => {
                    // Sum down rows, per column → 1×N output (ignore NAs)
                    let mut result_cols = Vec::new();
                    let mut result_names = Vec::new();

                    for (i, col) in tv.table.columns.iter().enumerate() {
                        if let blawktrust::Column::F64(data) = col {
                            // Compute sum ignoring NAs
                            let mut sum = 0.0;
                            for &val in data {
                                if !val.is_nan() {
                                    sum += val;
                                }
                            }
                            result_cols.push(blawktrust::Column::new_f64(vec![sum]));
                            result_names.push(tv.table.names[i].clone());
                        }
                    }

                    if result_cols.is_empty() {
                        return Err("sum: no numeric columns".to_string());
                    }

                    let result_table = blawktrust::Table::new(result_names, result_cols);
                    Ok(Value::tableview(blawktrust::TableView::new(result_table)))
                }
                Axis::Row => {
                    // Sum across columns, per row → M×1 output (single column)
                    let nrows = tv.table.columns.get(0).map(|c| c.len()).unwrap_or(0);
                    let mut sums = vec![0.0; nrows];

                    // Sum each row across numeric columns
                    for col in &tv.table.columns {
                        if let blawktrust::Column::F64(data) = col {
                            for (i, &val) in data.iter().enumerate() {
                                if !val.is_nan() {
                                    sums[i] += val;
                                }
                            }
                        }
                    }

                    let result_table = blawktrust::Table::new(
                        vec!["sum".to_string()],
                        vec![blawktrust::Column::new_f64(sums)]
                    );
                    Ok(Value::tableview(blawktrust::TableView::new(result_table)))
                }
            }
        }
        _ => Err(format!("sum expects column or tableview, got {}", args[0].type_name()))
    }
}

/// (sum0 col) - Sum column values (ignores NaN)
fn builtin_sum0(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("sum0 expects 1 argument, got {}", args.len()));
    }

    let col = args[0].as_col()?;
    let result = blawktrust::sum0(&col);
    Ok(Value::Float(result))
}

/// (mean table) - Mean of values according to axis
///
/// Axis-aware aggregation:
///   axis=:col (default) → mean down rows, per column → 1×N output
///   axis=:row → mean across columns, per row → M×1 output
fn builtin_mean(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("mean expects 1 argument, got {}", args.len()));
    }

    match &args[0] {
        Value::Col(col) => {
            // Single column: compute mean ignoring NAs
            let data = match col.as_ref() {
                blawktrust::Column::F64(data) => data,
                _ => return Err("mean expects F64 column".to_string()),
            };
            let mut sum = 0.0;
            let mut count = 0;
            for &val in data {
                if !val.is_nan() {
                    sum += val;
                    count += 1;
                }
            }
            let result = if count > 0 {
                sum / count as f64
            } else {
                f64::NAN
            };
            Ok(Value::Float(result))
        }
        Value::TableView(tv) => {
            use crate::value::Axis;

            match tv.axis {
                Axis::Col => {
                    // Mean down rows, per column → 1×N output (ignore NAs)
                    let mut result_cols = Vec::new();
                    let mut result_names = Vec::new();

                    for (i, col) in tv.table.columns.iter().enumerate() {
                        if let blawktrust::Column::F64(data) = col {
                            // Compute mean ignoring NAs
                            let mut sum = 0.0;
                            let mut count = 0;
                            for &val in data {
                                if !val.is_nan() {
                                    sum += val;
                                    count += 1;
                                }
                            }
                            let mean_val = if count > 0 {
                                sum / count as f64
                            } else {
                                f64::NAN
                            };
                            result_cols.push(blawktrust::Column::new_f64(vec![mean_val]));
                            result_names.push(tv.table.names[i].clone());
                        }
                    }

                    if result_cols.is_empty() {
                        return Err("mean: no numeric columns".to_string());
                    }

                    let result_table = blawktrust::Table::new(result_names, result_cols);
                    Ok(Value::tableview(blawktrust::TableView::new(result_table)))
                }
                Axis::Row => {
                    // Mean across columns, per row → M×1 output
                    let nrows = tv.table.columns.get(0).map(|c| c.len()).unwrap_or(0);
                    let mut means = vec![0.0; nrows];
                    let mut counts = vec![0; nrows];

                    // Sum each row across numeric columns
                    for col in &tv.table.columns {
                        if let blawktrust::Column::F64(data) = col {
                            for (i, &val) in data.iter().enumerate() {
                                if !val.is_nan() {
                                    means[i] += val;
                                    counts[i] += 1;
                                }
                            }
                        }
                    }

                    // Compute means
                    for (i, count) in counts.iter().enumerate() {
                        if *count > 0 {
                            means[i] /= *count as f64;
                        } else {
                            means[i] = f64::NAN;
                        }
                    }

                    let result_table = blawktrust::Table::new(
                        vec!["mean".to_string()],
                        vec![blawktrust::Column::new_f64(means)]
                    );
                    Ok(Value::tableview(blawktrust::TableView::new(result_table)))
                }
            }
        }
        _ => Err(format!("mean expects column or tableview, got {}", args[0].type_name()))
    }
}

/// (mean0 col) - Mean of column values (ignores NaN)
fn builtin_mean0(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("mean0 expects 1 argument, got {}", args.len()));
    }

    let col = args[0].as_col()?;
    let result = blawktrust::mean0(&col);
    Ok(Value::Float(result))
}

/// (std table) - Standard deviation, axis-aware (sample std, ddof=1, ignores NA)
///
/// axis=:col (default) → std down rows per column → 1×N output
/// axis=:row → std across columns per row → M×1 output
/// NA handling: ignore NAs, output NA if valid count < 2
fn builtin_std(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("std expects 1 argument, got {}", args.len()));
    }

    match &args[0] {
        Value::Col(col) => {
            // Single column: compute sample std
            let data = match col.as_ref() {
                blawktrust::Column::F64(data) => data,
                _ => return Err("std expects F64 column".to_string()),
            };

            let mut sum = 0.0;
            let mut sum_sq = 0.0;
            let mut count = 0;

            for &val in data {
                if !val.is_nan() {
                    sum += val;
                    sum_sq += val * val;
                    count += 1;
                }
            }

            if count < 2 {
                return Ok(Value::Float(f64::NAN));
            }

            let mean = sum / count as f64;
            let variance = (sum_sq - sum * sum / count as f64) / (count - 1) as f64;
            Ok(Value::Float(variance.sqrt()))
        }
        Value::TableView(tv) => {
            use crate::value::Axis;

            match tv.axis {
                Axis::Col => {
                    // Std down rows, per column → 1×N output
                    let mut result_cols = Vec::new();
                    let mut result_names = Vec::new();

                    for (i, col) in tv.table.columns.iter().enumerate() {
                        if let blawktrust::Column::F64(data) = col {
                            let mut sum = 0.0;
                            let mut sum_sq = 0.0;
                            let mut count = 0;

                            for &val in data {
                                if !val.is_nan() {
                                    sum += val;
                                    sum_sq += val * val;
                                    count += 1;
                                }
                            }

                            let std_val = if count < 2 {
                                f64::NAN
                            } else {
                                let mean = sum / count as f64;
                                let variance = (sum_sq - sum * sum / count as f64) / (count - 1) as f64;
                                variance.sqrt()
                            };

                            result_cols.push(blawktrust::Column::new_f64(vec![std_val]));
                            result_names.push(tv.table.names[i].clone());
                        }
                    }

                    if result_cols.is_empty() {
                        return Err("std: no numeric columns".to_string());
                    }

                    let result_table = blawktrust::Table::new(result_names, result_cols);
                    Ok(Value::tableview(blawktrust::TableView::new(result_table)))
                }
                Axis::Row => {
                    // Std across columns, per row → M×1 output (Welford algorithm)
                    let nrows = tv.table.columns.get(0).map(|c| c.len()).unwrap_or(0);
                    let mut stds = vec![0.0; nrows];

                    for i in 0..nrows {
                        let mut n = 0;
                        let mut mean = 0.0;
                        let mut m2 = 0.0;

                        // Welford's online algorithm for numerical stability
                        for col in &tv.table.columns {
                            if let blawktrust::Column::F64(data) = col {
                                let val = data[i];
                                if !val.is_nan() {
                                    n += 1;
                                    let delta = val - mean;
                                    mean += delta / n as f64;
                                    let delta2 = val - mean;
                                    m2 += delta * delta2;
                                }
                            }
                        }

                        stds[i] = if n < 2 {
                            f64::NAN
                        } else {
                            (m2 / (n - 1) as f64).sqrt()
                        };
                    }

                    let result_table = blawktrust::Table::new(
                        vec!["std".to_string()],
                        vec![blawktrust::Column::new_f64(stds)]
                    );
                    Ok(Value::tableview(blawktrust::TableView::new(result_table)))
                }
            }
        }
        _ => Err(format!("std expects column or tableview, got {}", args[0].type_name()))
    }
}

/// (std0 col) - Standard deviation of column values (ignores NaN, ddof=1)
fn builtin_std0(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("std0 expects 1 argument, got {}", args.len()));
    }

    let col = args[0].as_col()?;

    let data = match col.as_ref() {
        blawktrust::Column::F64(data) => data,
        _ => return Err("std0 expects F64 column".to_string()),
    };

    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    let mut count = 0;

    for &val in data {
        if !val.is_nan() {
            sum += val;
            sum_sq += val * val;
            count += 1;
        }
    }

    if count < 2 {
        return Ok(Value::Float(f64::NAN));
    }

    let mean = sum / count as f64;
    let variance = (sum_sq - sum * sum / count as f64) / (count - 1) as f64;
    Ok(Value::Float(variance.sqrt()))
}

/// (wstd col window) - Rolling standard deviation (propagates NaN)
fn builtin_wstd(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("wstd expects 2 arguments (col window), got {}", args.len()));
    }

    let col = args[0].as_col()?;
    let window = args[1].as_int()? as usize;

    let result = wstd(&col, window);
    Ok(Value::Col(Arc::new(result)))
}

/// (wstd0 col window) - Rolling standard deviation (ignores NaN)
fn builtin_wstd0(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("wstd0 expects 2 arguments (col window), got {}", args.len()));
    }

    let col = args[0].as_col()?;
    let window = args[1].as_int()? as usize;

    let result = wstd0(&col, window);
    Ok(Value::Col(Arc::new(result)))
}

/// (wstd-cols table window) - Apply wstd to all F64 columns
fn builtin_wstd_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("wstd-cols expects 2 arguments (table window), got {}", args.len()));
    }

    match &args[0] {
        Value::TableView(tv) => {
            let window = args[1].as_int()? as usize;
            let result = map_numeric_cols(tv.as_ref(), |col| Ok(wstd(col, window)))?;
            Ok(Value::tableview(result))
        }
        _ => Err(format!("wstd-cols expects TableView, got {}", args[0].type_name())),
    }
}

/// (wstd0-cols table window) - Apply wstd0 to all F64 columns
fn builtin_wstd0_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("wstd0-cols expects 2 arguments (table window), got {}", args.len()));
    }

    match &args[0] {
        Value::TableView(tv) => {
            let window = args[1].as_int()? as usize;
            let result = map_numeric_cols(tv.as_ref(), |col| Ok(wstd0(col, window)))?;
            Ok(Value::tableview(result))
        }
        _ => Err(format!("wstd0-cols expects TableView, got {}", args[0].type_name())),
    }
}

/// (wv col window) - Rolling volatility: wstd0(col, window) * sqrt(252)
fn builtin_wv(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("wv expects 2 arguments (col window), got {}", args.len()));
    }

    let col = args[0].as_col()?;
    let window = args[1].as_int()? as usize;

    let wstd_col = wstd0(&col, window);

    // Multiply by sqrt(252) for annualized volatility
    let sqrt_252 = 252.0_f64.sqrt();
    let result = match wstd_col {
        blawktrust::Column::F64(data) => {
            let scaled: Vec<f64> = data.iter().map(|&x| x * sqrt_252).collect();
            blawktrust::Column::new_f64(scaled)
        }
        _ => return Err("wv: unexpected column type".to_string()),
    };

    Ok(Value::Col(Arc::new(result)))
}

/// (wv-cols table window) - Apply wv to all F64 columns
fn builtin_wv_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("wv-cols expects 2 arguments (table window), got {}", args.len()));
    }

    match &args[0] {
        Value::TableView(tv) => {
            let window = args[1].as_int()? as usize;
            let sqrt_252 = 252.0_f64.sqrt();

            let result = map_numeric_cols(tv.as_ref(), |col| {
                let wstd_col = wstd0(col, window);
                Ok(match wstd_col {
                    blawktrust::Column::F64(data) => {
                        let scaled: Vec<f64> = data.iter().map(|&x| x * sqrt_252).collect();
                        blawktrust::Column::new_f64(scaled)
                    }
                    _ => wstd_col,
                })
            })?;
            Ok(Value::tableview(result))
        }
        _ => Err(format!("wv-cols expects TableView, got {}", args[0].type_name())),
    }
}

/// (zscore col) - Z-score normalization: (x - mean(x)) / std(x)
fn builtin_zscore(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("zscore expects 1 argument, got {}", args.len()));
    }

    let col = args[0].as_col()?;

    let data = match col.as_ref() {
        blawktrust::Column::F64(data) => data,
        _ => return Err("zscore expects F64 column".to_string()),
    };

    // Compute mean and std
    let n = data.len();
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    let mut count = 0;

    for &val in data {
        if !val.is_nan() {
            sum += val;
            sum_sq += val * val;
            count += 1;
        }
    }

    if count < 2 {
        return Ok(Value::Col(Arc::new(blawktrust::Column::new_f64(vec![f64::NAN; n]))));
    }

    let mean = sum / count as f64;
    let variance = (sum_sq - sum * sum / count as f64) / (count - 1) as f64;
    let std = variance.sqrt();

    if std == 0.0 || std.is_nan() {
        return Ok(Value::Col(Arc::new(blawktrust::Column::new_f64(vec![f64::NAN; n]))));
    }

    // Apply z-score transformation
    let result: Vec<f64> = data.iter().map(|&x| {
        if x.is_nan() {
            f64::NAN
        } else {
            (x - mean) / std
        }
    }).collect();

    Ok(Value::Col(Arc::new(blawktrust::Column::new_f64(result))))
}

/// (chop col min max) - Clip column values to [min, max]
fn builtin_chop(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 3 {
        return Err(format!("chop expects 3 arguments (col min max), got {}", args.len()));
    }

    let col = args[0].as_col()?;
    let min_val = args[1].as_float()?;
    let max_val = args[2].as_float()?;

    if min_val > max_val {
        return Err(format!("chop: min ({}) must be <= max ({})", min_val, max_val));
    }

    let data = match col.as_ref() {
        blawktrust::Column::F64(data) => data,
        _ => return Err("chop expects F64 column".to_string()),
    };

    let result: Vec<f64> = data.iter().map(|&x| {
        if x.is_nan() {
            f64::NAN
        } else if x < min_val {
            min_val
        } else if x > max_val {
            max_val
        } else {
            x
        }
    }).collect();

    Ok(Value::Col(Arc::new(blawktrust::Column::new_f64(result))))
}

// ============================================================================
// Column Operation Helpers
// ============================================================================

fn add_columns(a: &blawktrust::Column, b: &blawktrust::Column) -> Result<blawktrust::Column, String> {
    if a.len() != b.len() {
        return Err(format!("Column length mismatch: {} vs {}", a.len(), b.len()));
    }

    // Simple element-wise addition for F64 columns
    match (a, b) {
        (blawktrust::Column::F64(a_data),
         blawktrust::Column::F64(b_data)) => {
            let result: Vec<f64> = a_data.iter().zip(b_data.iter())
                .map(|(x, y)| x + y)
                .collect();
            Ok(blawktrust::Column::new_f64(result))
        }
        _ => Err("Column addition only supported for F64 columns".to_string()),
    }
}

fn add_column_scalar(col: &blawktrust::Column, scalar: f64) -> Result<blawktrust::Column, String> {
    match col {
        blawktrust::Column::F64(data) => {
            let result: Vec<f64> = data.iter().map(|x| x + scalar).collect();
            Ok(blawktrust::Column::new_f64(result))
        }
        _ => Err("Column scalar addition only supported for F64 columns".to_string()),
    }
}

fn mul_column_scalar(col: &blawktrust::Column, scalar: f64) -> Result<blawktrust::Column, String> {
    match col {
        blawktrust::Column::F64(data) => {
            let result: Vec<f64> = data.iter().map(|x| x * scalar).collect();
            Ok(blawktrust::Column::new_f64(result))
        }
        _ => Err("Column scalar multiplication only supported for F64 columns".to_string()),
    }
}

fn mul_columns(a: &blawktrust::Column, b: &blawktrust::Column) -> Result<blawktrust::Column, String> {
    if a.len() != b.len() {
        return Err(format!("Column length mismatch: {} vs {}", a.len(), b.len()));
    }

    match (a, b) {
        (blawktrust::Column::F64(a_data), blawktrust::Column::F64(b_data)) => {
            let result: Vec<f64> = a_data.iter().zip(b_data.iter())
                .map(|(x, y)| x * y)
                .collect();
            Ok(blawktrust::Column::new_f64(result))
        }
        _ => Err("Column multiplication only supported for F64 columns".to_string()),
    }
}

fn log_column(col: &blawktrust::Column) -> Result<blawktrust::Column, String> {
    match col {
        blawktrust::Column::F64(data) => {
            let result: Vec<f64> = data.iter().map(|x| x.ln()).collect();
            Ok(blawktrust::Column::new_f64(result))
        }
        _ => Err("Column log only supported for F64 columns".to_string()),
    }
}

fn exp_column(col: &blawktrust::Column) -> Result<blawktrust::Column, String> {
    match col {
        blawktrust::Column::F64(data) => {
            let result: Vec<f64> = data.iter().map(|x| x.exp()).collect();
            Ok(blawktrust::Column::new_f64(result))
        }
        _ => Err("Column exp only supported for F64 columns".to_string()),
    }
}

fn abs_column(col: &blawktrust::Column) -> Result<blawktrust::Column, String> {
    match col {
        blawktrust::Column::F64(data) => {
            let result: Vec<f64> = data.iter().map(|x| x.abs()).collect();
            Ok(blawktrust::Column::new_f64(result))
        }
        _ => Err("Column abs only supported for F64 columns".to_string()),
    }
}

/// Compare column elements with scalar using given comparison function
///
/// Returns 1.0 where comparison is true, 0.0 where false, NA where col is NA
fn compare_column_scalar<F>(col: &blawktrust::Column, scalar: f64, cmp: F) -> Result<blawktrust::Column, String>
where
    F: Fn(f64, f64) -> bool,
{
    match col {
        blawktrust::Column::F64(data) => {
            let result: Vec<f64> = data.iter()
                .map(|&x| {
                    if x.is_nan() {
                        f64::NAN
                    } else if cmp(x, scalar) {
                        1.0
                    } else {
                        0.0
                    }
                })
                .collect();
            Ok(blawktrust::Column::new_f64(result))
        }
        _ => Err("Comparison only supported for F64 columns".to_string()),
    }
}

/// Compare two columns element-wise using given comparison function
///
/// Returns 1.0 where comparison is true, 0.0 where false, NA if either element is NA
fn compare_columns<F>(a: &blawktrust::Column, b: &blawktrust::Column, cmp: F) -> Result<blawktrust::Column, String>
where
    F: Fn(f64, f64) -> bool,
{
    if a.len() != b.len() {
        return Err(format!("Column length mismatch: {} vs {}", a.len(), b.len()));
    }

    match (a, b) {
        (blawktrust::Column::F64(a_data), blawktrust::Column::F64(b_data)) => {
            let result: Vec<f64> = a_data.iter().zip(b_data.iter())
                .map(|(&x, &y)| {
                    if x.is_nan() || y.is_nan() {
                        f64::NAN
                    } else if cmp(x, y) {
                        1.0
                    } else {
                        0.0
                    }
                })
                .collect();
            Ok(blawktrust::Column::new_f64(result))
        }
        _ => Err("Comparison only supported for F64 columns".to_string()),
    }
}

/// Filter column by boolean mask (keep only true indices)
fn filter_column(col: &blawktrust::Column, mask: &[bool]) -> blawktrust::Column {
    match col {
        blawktrust::Column::F64(data) => {
            let filtered: Vec<f64> = data.iter().zip(mask.iter())
                .filter_map(|(&val, &keep)| if keep { Some(val) } else { None })
                .collect();
            blawktrust::Column::new_f64(filtered)
        }
        blawktrust::Column::Date(data) => {
            let filtered: Vec<i32> = data.iter().zip(mask.iter())
                .filter_map(|(&val, &keep)| if keep { Some(val) } else { None })
                .collect();
            blawktrust::Column::new_date(filtered)
        }
        blawktrust::Column::Timestamp(data) => {
            let filtered: Vec<i64> = data.iter().zip(mask.iter())
                .filter_map(|(&val, &keep)| if keep { Some(val) } else { None })
                .collect();
            blawktrust::Column::new_timestamp(filtered)
        }
    }
}

/// Subtract two columns element-wise (A - B)
fn subtract_columns_pair(a: &blawktrust::Column, b: &blawktrust::Column) -> Result<blawktrust::Column, String> {
    if a.len() != b.len() {
        return Err(format!("Column length mismatch: {} vs {}", a.len(), b.len()));
    }

    match (a, b) {
        (blawktrust::Column::F64(a_data), blawktrust::Column::F64(b_data)) => {
            let result: Vec<f64> = a_data.iter().zip(b_data.iter())
                .map(|(x, y)| x - y)
                .collect();
            Ok(blawktrust::Column::new_f64(result))
        }
        _ => Err("Column subtraction only supported for F64 columns".to_string()),
    }
}

/// Map source column values to target keys using HashMap lookup
fn map_column_by_keys(
    target_keys: &blawktrust::Column,
    source_col: &blawktrust::Column,
    key_map: &std::collections::HashMap<String, usize>,
) -> Result<blawktrust::Column, String> {
    use std::collections::HashMap;

    // Extract target keys as strings
    let target_key_strs: Vec<String> = match target_keys {
        blawktrust::Column::Date(data) => data.iter().map(|k| k.to_string()).collect(),
        blawktrust::Column::F64(data) => data.iter().map(|k| k.to_string()).collect(),
        _ => return Err("map_column_by_keys: target keys must be Date or F64".to_string()),
    };

    // Map source values
    match source_col {
        blawktrust::Column::F64(source_data) => {
            let result: Vec<f64> = target_key_strs.iter()
                .map(|key| {
                    key_map.get(key)
                        .map(|&idx| source_data[idx])
                        .unwrap_or(f64::NAN)
                })
                .collect();
            Ok(blawktrust::Column::new_f64(result))
        }
        _ => Err("map_column_by_keys: source column must be F64".to_string()),
    }
}

/// Generic helper: Apply function to all F64 columns in a TableView
///
/// Contract: TableView -> (Column -> Column) -> TableView
/// - Preserves column order (from TableView.table.names)
/// - Preserves non-numeric columns unchanged (Date, Timestamp)
/// - Applies function only to F64 columns
/// - Returns new TableView with same schema
fn map_numeric_cols<F>(tv: &blawktrust::TableView, f: F) -> Result<blawktrust::TableView, String>
where
    F: Fn(&blawktrust::Column) -> Result<blawktrust::Column, String>,
{
    let mut new_names = Vec::new();
    let mut new_columns = Vec::new();

    // Iterate in original column order (stable order for CSV output)
    for (i, name) in tv.table.names.iter().enumerate() {
        let col = &tv.table.columns[i];

        match col {
            blawktrust::Column::F64(_) => {
                // Apply transformation to numeric column
                let transformed = f(col)?;
                new_names.push(name.clone());
                new_columns.push(transformed);
            }
            blawktrust::Column::Date(_) | blawktrust::Column::Timestamp(_) => {
                // Preserve non-numeric columns unchanged
                new_names.push(name.clone());
                new_columns.push(col.clone());
            }
        }
    }

    let new_table = blawktrust::Table::new(new_names, new_columns);
    Ok(blawktrust::TableView::new(new_table))
}

/// Shift column by lag positions (first lag elements become NA)
fn shift_column(col: &blawktrust::Column, lag: usize) -> Result<blawktrust::Column, String> {
    match col {
        blawktrust::Column::F64(data) => {
            let n = data.len();
            let mut result = vec![f64::NAN; n];

            // Copy shifted values
            for i in lag..n {
                result[i] = data[i - lag];
            }

            Ok(blawktrust::Column::new_f64(result))
        }
        _ => Err("shift only supported for F64 columns".to_string()),
    }
}

/// Subtract two columns element-wise
fn subtract_columns(a: &blawktrust::Column, b: &blawktrust::Column) -> Result<blawktrust::Column, String> {
    if a.len() != b.len() {
        return Err(format!("Column length mismatch: {} vs {}", a.len(), b.len()));
    }

    match (a, b) {
        (blawktrust::Column::F64(a_data),
         blawktrust::Column::F64(b_data)) => {
            let result: Vec<f64> = a_data.iter().zip(b_data.iter())
                .map(|(x, y)| x - y)
                .collect();
            Ok(blawktrust::Column::new_f64(result))
        }
        _ => Err("Column subtraction only supported for F64 columns".to_string()),
    }
}

// ============================================================================
// Table Operations
// ============================================================================

/// (cols table) → list of column names (as strings)
fn builtin_cols(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("cols expects 1 argument, got {}", args.len()));
    }

    let table = args[0].as_table()?;

    // Return column names as a list of strings
    let names: Vec<Value> = table.columns.iter()
        .map(|(sym_id, _)| {
            let name_str = rt.interner.resolve(*sym_id).to_string();
            Value::Str(name_str.into())
        })
        .collect();

    Ok(Value::List(names))
}

/// (select table "col1" "col2" ...) → Table with selected columns
fn builtin_select(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() < 2 {
        return Err(format!("select expects at least 2 arguments (table + column names), got {}", args.len()));
    }

    let table = args[0].as_table()?;
    let mut new_table = crate::value::Table::new();

    // Select columns by name
    for arg in &args[1..] {
        let col_name = match arg {
            Value::Str(s) => s.as_ref(),
            _ => return Err("select: column names must be strings".to_string()),
        };

        // Find column by name
        let col_sym = rt.interner.intern(col_name);
        let col = table.get_column(col_sym)
            .ok_or_else(|| format!("select: column '{}' not found", col_name))?;

        new_table.add_column(col_sym, col.clone());
    }

    Ok(Value::Table(Arc::new(new_table)))
}

/// (select-num table) → Table with only F64 columns
fn builtin_select_num(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!("select-num expects 1 argument, got {}", args.len()));
    }

    let table = args[0].as_table()?;
    let mut new_table = crate::value::Table::new();

    // Keep only F64 columns
    for (name, col) in &table.columns {
        if let blawktrust::Column::F64(_) = col {
            new_table.add_column(*name, col.clone());
        }
    }

    Ok(Value::Table(Arc::new(new_table)))
}

/// (map-cols table fn) → Table
/// Apply unary Col→Col function to each F64 column, preserve Ts columns unchanged
fn builtin_map_cols(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("map-cols expects 2 arguments (table fn), got {}", args.len()));
    }

    let table = args[0].as_table()?;

    // For now, fn must be a symbol referring to a builtin
    // Clone the function name string to avoid borrow conflicts
    let fn_name = match &args[1] {
        Value::Sym(sym_id) => rt.interner.resolve(*sym_id).to_string(),
        _ => return Err("map-cols: function must be a symbol (builtin name)".to_string()),
    };

    let mut new_table = crate::value::Table::new();

    // Apply function to each column
    for (name, col) in &table.columns {
        match col {
            blawktrust::Column::F64(_) => {
                // Apply function to F64 columns
                let col_val = Value::Col(Arc::new(col.clone()));
                let fn_args = vec![col_val];

                // Dispatch to builtin (only unary math functions for now)
                let result = match fn_name.as_str() {
                    "log" => builtin_log(rt, &fn_args)?,
                    "exp" => builtin_exp(rt, &fn_args)?,
                    "abs" => builtin_abs(rt, &fn_args)?,
                    _ => return Err(format!("map-cols: unsupported function '{}' (try: log, exp, abs)", fn_name)),
                };

                let result_col = result.as_col()?;
                new_table.add_column(*name, (*result_col).clone());
            }
            blawktrust::Column::Date(_) | blawktrust::Column::Timestamp(_) => {
                // Keep Date/Timestamp columns unchanged
                new_table.add_column(*name, col.clone());
            }
        }
    }

    Ok(Value::Table(Arc::new(new_table)))
}

/// (apply-cols table fn) → 1-row Table
/// Apply Col→scalar aggregation to each F64 column, skip Ts columns
fn builtin_apply_cols(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    if args.len() != 2 {
        return Err(format!("apply-cols expects 2 arguments (table fn), got {}", args.len()));
    }

    let table = args[0].as_table()?;

    // For now, fn must be a symbol referring to a builtin
    // Clone the function name string to avoid borrow conflicts
    let fn_name = match &args[1] {
        Value::Sym(sym_id) => rt.interner.resolve(*sym_id).to_string(),
        _ => return Err("apply-cols: function must be a symbol (builtin name)".to_string()),
    };

    let mut new_table = crate::value::Table::new();

    // Apply aggregation function to each numeric column
    for (name, col) in &table.columns {
        if let blawktrust::Column::F64(_) = col {
            let col_val = Value::Col(Arc::new(col.clone()));
            let fn_args = vec![col_val];

            // Dispatch to aggregation builtin
            let scalar_result = match fn_name.as_str() {
                "sum" => builtin_sum(rt, &fn_args)?,
                "sum0" => builtin_sum0(rt, &fn_args)?,
                "mean" => builtin_mean(rt, &fn_args)?,
                "mean0" => builtin_mean0(rt, &fn_args)?,
                _ => return Err(format!("apply-cols: unknown aggregation function '{}'", fn_name)),
            };

            // Convert scalar to 1-element column
            let scalar_val = scalar_result.as_float()?;
            let result_col = blawktrust::Column::new_f64(vec![scalar_val]);
            new_table.add_column(*name, result_col);
        }
        // Skip Ts columns
    }

    Ok(Value::Table(Arc::new(new_table)))
}

/// (dlog-cols table [lag]) → Frame/TableView
/// Apply dlog with lag to each F64 column (default lag=1)
/// BLADE: Frame version uses map_numeric_preserve_tags (Blueprint I1-I3)
fn builtin_dlog_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    let lag = match args.len() {
        1 => 1,
        2 => args[1].as_int()? as usize,
        n => return Err(format!("dlog expects 1-2 arguments, got {}", n)),
    };

    match &args[0] {
        Value::Frame(f) => {
            // BLADE: Use map_numeric_preserve_tags (preserves I1-I3)
            let result = frame::map_numeric_preserve_tags(f, |col| {
                dlog_column(col, lag)
            });
            Ok(Value::Frame(Arc::new(result)))
        }
        Value::TableView(tv) => {
            let result = map_numeric_cols(tv.as_ref(), |col| {
                let col_arc = Arc::new(col.clone());
                Ok(dlog_column(&col_arc, lag))
            })?;
            Ok(Value::tableview(result))
        }
        _ => Err(format!("dlog expects Frame or TableView, got {}", args[0].type_name())),
    }
}

/// (shift-cols table [lag]) → Table
/// Apply shift with lag to each F64 column (default lag=1)
fn builtin_shift_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    let lag = match args.len() {
        1 => 1,
        2 => args[1].as_int()?,
        n => return Err(format!("shift-cols expects 1-2 arguments, got {}", n)),
    };

    if lag < 0 {
        return Err("shift-cols with negative lag not yet implemented".to_string());
    }

    match &args[0] {
        Value::TableView(tv) => {
            let result = map_numeric_cols(tv.as_ref(), |col| {
                shift_column(col, lag as usize)
            })?;

            Ok(Value::tableview(result))
        }
        _ => Err(format!("shift-cols expects TableView, got {}", args[0].type_name())),
    }
}

/// (diff-cols table [lag]) → Table
/// Apply diff with lag to each F64 column (default lag=1)
fn builtin_diff_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    let lag = match args.len() {
        1 => 1,
        2 => args[1].as_int()? as usize,
        n => return Err(format!("diff-cols expects 1-2 arguments, got {}", n)),
    };

    let table = args[0].as_table()?;

    let mut new_table = crate::value::Table::new();

    // Apply diff to each F64 column
    for (name, col) in &table.columns {
        match col {
            blawktrust::Column::F64(_) => {
                // Compute: col - shift(col, lag)
                let shifted = shift_column(col, lag)?;
                let result_col = subtract_columns(col, &shifted)?;
                new_table.add_column(*name, result_col);
            }
            blawktrust::Column::Date(_) | blawktrust::Column::Timestamp(_) => {
                // Keep Date/Timestamp columns unchanged
                new_table.add_column(*name, col.clone());
            }
        }
    }

    Ok(Value::Table(Arc::new(new_table)))
}

// ============================================================================
// Orientation Builtin
// ============================================================================

/// (o table orientation) - Change table orientation
/// Example: (o data WENS) rotates to row-wise view
/// Example: (o data H) or (o data NSWE) for horizontal (default) view
fn builtin_o(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    use crate::value::{Axis, Layout, TableViewWithMetadata};

    match args.len() {
        1 => {
            // Unary form: (o MODE) returns a function
            // This is a special case - we return a lambda-like value
            // For now, just return an error - proper lambda support needs more work
            Err("o: unary form (o MODE) not yet implemented - use (o MODE table) instead".to_string())
        }
        2 => {
            // Binary form: (o MODE table) returns modified table
            let mode_arg = &args[0];
            let table_arg = &args[1];

            // Parse the mode argument (keyword or symbol)
            let mode_name = match mode_arg {
                Value::Sym(s) => rt.interner.resolve(*s),
                Value::Str(s) => s.as_ref(),
                _ => return Err(format!("o expects keyword (:col/:row/:reset) or symbol (NSWE/WENS/...), got {}", mode_arg.type_name())),
            };

            // Check if it's a keyword (starts with :) for axis semantics
            if mode_name.starts_with(':') {
                // Keyword form: axis semantics
                let keyword = &mode_name[1..]; // Strip leading ':'
                let axis = match keyword {
                    "col" => Axis::Col,
                    "row" => Axis::Row,
                    "reset" => Axis::Col, // :reset = back to column-wise default
                    _ => return Err(format!("o: unknown axis keyword :{}. Valid: :col, :row, :reset", keyword)),
                };

                // Get table and set axis
                match table_arg {
                    Value::TableView(tv) => {
                        let new_tv = tv.with_axis(axis);
                        Ok(Value::TableView(Arc::new(new_tv)))
                    }
                    _ => {
                        // Try to convert to TableView first
                        let tv = ensure_tableview(table_arg, rt)?;
                        let tv_meta = TableViewWithMetadata::from_view(tv).with_axis(axis);
                        Ok(Value::TableView(Arc::new(tv_meta)))
                    }
                }
            } else {
                // Symbol form: layout (physical orientation)
                let layout = Layout::from_str(mode_name)
                    .ok_or_else(|| format!("o: unknown layout symbol '{}'. Valid: NSWE, WENS, SNWE, EWNS, H, Z, N, S, etc.", mode_name))?;

                // Get table and set layout
                match table_arg {
                    Value::TableView(tv) => {
                        let new_tv = tv.with_layout(layout);
                        Ok(Value::TableView(Arc::new(new_tv)))
                    }
                    _ => {
                        // Try to convert to TableView first
                        let tv = ensure_tableview(table_arg, rt)?;
                        let tv_meta = TableViewWithMetadata::from_view(tv).with_layout(layout);
                        Ok(Value::TableView(Arc::new(tv_meta)))
                    }
                }
            }
        }
        _ => Err(format!("o expects 1 or 2 arguments, got {}", args.len())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_scalars() {
        let mut rt = Runtime::new();
        let args = vec![Value::Int(1), Value::Int(2)];
        let result = builtin_add(&mut rt, &args).unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_add_float_int() {
        let mut rt = Runtime::new();
        let args = vec![Value::Float(3.14), Value::Int(2)];
        let result = builtin_add(&mut rt, &args).unwrap();
        if let Value::Float(f) = result {
            assert!((f - 5.14).abs() < 0.0001);
        } else {
            panic!("Expected float result");
        }
    }

    #[test]
    fn test_mul_scalars() {
        let mut rt = Runtime::new();
        let args = vec![Value::Int(3), Value::Int(4)];
        let result = builtin_mul(&mut rt, &args).unwrap();
        assert_eq!(result, Value::Int(12));
    }

    #[test]
    fn test_div_scalars() {
        let mut rt = Runtime::new();
        let args = vec![Value::Int(10), Value::Int(2)];
        let result = builtin_div(&mut rt, &args).unwrap();
        assert_eq!(result, Value::Float(5.0));
    }

    #[test]
    fn test_div_by_zero() {
        let mut rt = Runtime::new();
        let args = vec![Value::Int(10), Value::Int(0)];
        let result = builtin_div(&mut rt, &args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Division by zero"));
    }

    #[test]
    fn test_abs() {
        let mut rt = Runtime::new();
        assert_eq!(builtin_abs(&mut rt, &[Value::Int(-5)]).unwrap(), Value::Int(5));
        assert_eq!(builtin_abs(&mut rt, &[Value::Float(-3.14)]).unwrap(), Value::Float(3.14));
    }

    #[test]
    fn test_type_of() {
        let mut rt = Runtime::new();
        let result = builtin_type_of(&mut rt, &[Value::Int(42)]).unwrap();
        assert_eq!(result, Value::Str("int".into()));
    }

    #[test]
    fn test_len_col() {
        let mut rt = Runtime::new();
        let col = blawktrust::Column::new_f64(vec![1.0, 2.0, 3.0]);
        let val = Value::Col(Arc::new(col));
        let result = builtin_len(&mut rt, &[val]).unwrap();
        assert_eq!(result, Value::Int(3));
    }

    #[test]
    fn test_add_column_scalar() {
        let mut rt = Runtime::new();
        let col = blawktrust::Column::new_f64(vec![1.0, 2.0, 3.0]);
        let args = vec![Value::Col(Arc::new(col)), Value::Float(10.0)];
        let result = builtin_add(&mut rt, &args).unwrap();

        if let Value::Col(result_col) = result {
            if let blawktrust::Column::F64(data) = &*result_col {
                assert_eq!(data[0], 11.0);
                assert_eq!(data[1], 12.0);
                assert_eq!(data[2], 13.0);
            }
        } else {
            panic!("Expected Col result");
        }
    }

    #[test]
    fn test_mul_column_scalar() {
        let mut rt = Runtime::new();
        let col = blawktrust::Column::new_f64(vec![1.0, 2.0, 3.0]);
        let args = vec![Value::Col(Arc::new(col)), Value::Float(2.0)];
        let result = builtin_mul(&mut rt, &args).unwrap();

        if let Value::Col(result_col) = result {
            if let blawktrust::Column::F64(data) = &*result_col {
                assert_eq!(data[0], 2.0);
                assert_eq!(data[1], 4.0);
                assert_eq!(data[2], 6.0);
            }
        } else {
            panic!("Expected Col result");
        }
    }

    #[test]
    fn test_dlog() {
        let mut rt = Runtime::new();
        let col = blawktrust::Column::new_f64(vec![100.0, 101.0, 102.0, 103.0]);
        let args = vec![Value::Col(Arc::new(col)), Value::Int(1)];
        let result = builtin_dlog(&mut rt, &args).unwrap();

        if let Value::Col(result_col) = result {
            if let blawktrust::Column::F64(data) = &*result_col {
                // First element should be NaN (no previous value)
                assert!(data[0].is_nan());
                // Second element: log(101) - log(100)
                let expected = (101.0_f64).ln() - (100.0_f64).ln();
                assert!((data[1] - expected).abs() < 1e-10);
            }
        } else {
            panic!("Expected Col result");
        }
    }

    #[test]
    fn test_shift() {
        let mut rt = Runtime::new();
        let col = blawktrust::Column::new_f64(vec![10.0, 20.0, 30.0, 40.0]);
        let args = vec![Value::Col(Arc::new(col)), Value::Int(1)];
        let result = builtin_shift(&mut rt, &args).unwrap();

        if let Value::Col(result_col) = result {
            if let blawktrust::Column::F64(data) = &*result_col {
                assert!(data[0].is_nan()); // First element is NA
                assert_eq!(data[1], 10.0);
                assert_eq!(data[2], 20.0);
                assert_eq!(data[3], 30.0);
            }
        } else {
            panic!("Expected Col result");
        }
    }

    #[test]
    fn test_diff() {
        let mut rt = Runtime::new();
        let col = blawktrust::Column::new_f64(vec![100.0, 102.0, 105.0, 103.0]);
        let args = vec![Value::Col(Arc::new(col)), Value::Int(1)];
        let result = builtin_diff(&mut rt, &args).unwrap();

        if let Value::Col(result_col) = result {
            if let blawktrust::Column::F64(data) = &*result_col {
                assert!(data[0].is_nan()); // First element is NA
                assert_eq!(data[1], 2.0);  // 102 - 100
                assert_eq!(data[2], 3.0);  // 105 - 102
                assert_eq!(data[3], -2.0); // 103 - 105
            }
        } else {
            panic!("Expected Col result");
        }
    }

    #[test]
    fn test_col_extraction() {
        use crate::value::Table;
        let mut rt = Runtime::new();

        // Create a table
        let mut table = Table::new();
        let px_sym = rt.interner.intern("px");
        let vol_sym = rt.interner.intern("vol");

        let px_col = blawktrust::Column::new_f64(vec![100.0, 102.0]);
        let vol_col = blawktrust::Column::new_f64(vec![1000.0, 1200.0]);

        table.add_column(px_sym, px_col);
        table.add_column(vol_sym, vol_col);

        let table_val = Value::Table(Arc::new(table));

        // Test (col table 'px)
        let px_sym_val = Value::Sym(px_sym);
        let args = vec![table_val.clone(), px_sym_val];
        let result = builtin_col(&mut rt, &args).unwrap();

        if let Value::Col(col) = result {
            assert_eq!(col.len(), 2);
        } else {
            panic!("Expected Col result");
        }
    }

    // TASK D: Test string-based column lookup for Bloomberg-style headers
    #[test]
    fn test_col_extraction_with_string() {
        use crate::value::Table;
        let mut rt = Runtime::new();

        // Create a table with space in column name (Bloomberg style)
        let mut table = Table::new();
        let spy_sym = rt.interner.intern("SPY US Equity");
        let es_sym = rt.interner.intern("ES1 Index");

        let spy_col = blawktrust::Column::new_f64(vec![145.0, 146.0]);
        let es_col = blawktrust::Column::new_f64(vec![1534.0, 1542.0]);

        table.add_column(spy_sym, spy_col);
        table.add_column(es_sym, es_col);

        let table_val = Value::Table(Arc::new(table));

        // Test (col table "SPY US Equity") - string lookup
        let spy_str = Value::Str("SPY US Equity".into());
        let args = vec![table_val.clone(), spy_str];
        let result = builtin_col(&mut rt, &args).unwrap();

        if let Value::Col(col) = result {
            assert_eq!(col.len(), 2);
            if let blawktrust::Column::F64(data) = &*col {
                assert_eq!(data[0], 145.0);
                assert_eq!(data[1], 146.0);
            }
        } else {
            panic!("Expected Col result");
        }

        // Test (col table "ES1 Index") - another string lookup
        let es_str = Value::Str("ES1 Index".into());
        let args = vec![table_val, es_str];
        let result = builtin_col(&mut rt, &args).unwrap();

        if let Value::Col(col) = result {
            assert_eq!(col.len(), 2);
            if let blawktrust::Column::F64(data) = &*col {
                assert_eq!(data[0], 1534.0);
                assert_eq!(data[1], 1542.0);
            }
        } else {
            panic!("Expected Col result");
        }
    }

    #[test]
    fn test_w_extraction() {
        use crate::value::Table;
        let mut rt = Runtime::new();

        // Create a table
        let mut table = Table::new();
        let px_sym = rt.interner.intern("px");
        let vol_sym = rt.interner.intern("vol");

        let px_col = blawktrust::Column::new_f64(vec![100.0, 102.0]);
        let vol_col = blawktrust::Column::new_f64(vec![1000.0, 1200.0]);

        table.add_column(px_sym, px_col);
        table.add_column(vol_sym, vol_col);

        let table_val = Value::Table(Arc::new(table));

        // Test (w table 0) - first column
        let args = vec![table_val.clone(), Value::Int(0)];
        let result = builtin_w(&mut rt, &args).unwrap();

        if let Value::Col(col) = result {
            assert_eq!(col.len(), 2);
        } else {
            panic!("Expected Col result");
        }

        // Test (w table 1) - second column
        let args = vec![table_val, Value::Int(1)];
        let result = builtin_w(&mut rt, &args).unwrap();

        if let Value::Col(col) = result {
            assert_eq!(col.len(), 2);
        } else {
            panic!("Expected Col result");
        }
    }
}

    #[test]
    fn test_sum_aggregation() {
        let mut rt = Runtime::new();
        
        // Test sum without NaN
        let col = blawktrust::Column::new_f64(vec![1.0, 2.0, 3.0, 4.0]);
        let args = vec![Value::Col(Arc::new(col))];
        let result = builtin_sum(&mut rt, &args).unwrap();
        assert_eq!(result.as_float().unwrap(), 10.0);
        
        // Test sum with NaN (propagates)
        let col_na = blawktrust::Column::new_f64(vec![1.0, f64::NAN, 3.0]);
        let args_na = vec![Value::Col(Arc::new(col_na))];
        let result_na = builtin_sum(&mut rt, &args_na).unwrap();
        assert!(result_na.as_float().unwrap().is_nan());
    }

    #[test]
    fn test_sum0_aggregation() {
        let mut rt = Runtime::new();
        
        // Test sum0 with NaN (ignores)
        let col = blawktrust::Column::new_f64(vec![1.0, f64::NAN, 3.0, 4.0]);
        let args = vec![Value::Col(Arc::new(col))];
        let result = builtin_sum0(&mut rt, &args).unwrap();
        assert_eq!(result.as_float().unwrap(), 8.0);
    }

    #[test]
    fn test_mean_aggregation() {
        let mut rt = Runtime::new();
        
        // Test mean without NaN
        let col = blawktrust::Column::new_f64(vec![1.0, 2.0, 3.0, 4.0]);
        let args = vec![Value::Col(Arc::new(col))];
        let result = builtin_mean(&mut rt, &args).unwrap();
        assert_eq!(result.as_float().unwrap(), 2.5);
        
        // Test mean with NaN (propagates)
        let col_na = blawktrust::Column::new_f64(vec![1.0, f64::NAN, 3.0]);
        let args_na = vec![Value::Col(Arc::new(col_na))];
        let result_na = builtin_mean(&mut rt, &args_na).unwrap();
        assert!(result_na.as_float().unwrap().is_nan());
    }

    #[test]
    fn test_mean0_aggregation() {
        let mut rt = Runtime::new();
        
        // Test mean0 with NaN (ignores)
        let col = blawktrust::Column::new_f64(vec![2.0, f64::NAN, 4.0, 6.0]);
        let args = vec![Value::Col(Arc::new(col))];
        let result = builtin_mean0(&mut rt, &args).unwrap();
        assert_eq!(result.as_float().unwrap(), 4.0);
    }
