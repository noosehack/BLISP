//! Metamorphic Properties - Semantic Tripwires
//!
//! These properties must hold for ANY correct implementation, regardless of
//! representation (AST vs IR vs optimized IR).
//!
//! Unlike equivalence tests, these catch:
//! - Scoping bugs (let* sequential vs parallel evaluation)
//! - Shape invariants (join output dimensions)
//! - NA mask bugs (unary ops changing missingness pattern)
//! - Alignment bugs (cross-column bleeding)
//!
//! Each failure is a CONTRACT VIOLATION per contracts.md.

mod common;

use blisp::normalize::normalize;
use blisp::planner::plan;
use blisp::exec::execute;
use blisp::runtime::Runtime;
use blisp::value::Value;
use blisp::ast::{Expr, Interner};
use blisp::frame::IndexColumn;
use common::{assert_frame_equiv, build_date_frame, Env};
use std::sync::Arc;

// ============================================================================
// Let* Scoping Laws (Sequential Semantics)
// ============================================================================

#[test]
fn meta_let_shadowing_law() {
    // Property: (let ((x e1) (x e2)) body) == (let ((x e2)) body)
    // Validates: Inner binding shadows outer, no accidental parallel eval

    let mut rt = Runtime::new();
    let mut env = Env::new();

    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
    env.bind("x", Arc::clone(&x));
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(x));

    let y = build_date_frame(43, "DATE", 15, 1, false, 0.05);
    env.bind("y", Arc::clone(&y));
    let y_sym = rt.interner.intern("y");
    rt.define(y_sym, Value::Frame(y));

    // Construct: (let ((a (dlog x)) (a (dlog y))) a)
    let let_sym = rt.interner.intern("let");
    let a_sym = rt.interner.intern("a");
    let dlog_sym = rt.interner.intern("dlog");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");

    let shadowing_expr = Expr::List(vec![
        Expr::Sym(let_sym),
        Expr::List(vec![
            Expr::List(vec![
                Expr::Sym(a_sym),
                Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(x_sym)]),
            ]),
            Expr::List(vec![
                Expr::Sym(a_sym),
                Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(y_sym)]),
            ]),
        ]),
        Expr::Sym(a_sym),
    ]);

    // Construct: (let ((a (dlog y))) a)
    let expected_expr = Expr::List(vec![
        Expr::Sym(let_sym),
        Expr::List(vec![
            Expr::List(vec![
                Expr::Sym(a_sym),
                Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(y_sym)]),
            ]),
        ]),
        Expr::Sym(a_sym),
    ]);

    // Execute both via IR
    let normalized1 = normalize(shadowing_expr, &mut rt.interner);
    let plan1 = plan(&normalized1, &rt.interner).expect("plan1 failed");
    let result1 = execute(&plan1, &mut rt).expect("exec1 failed");

    let normalized2 = normalize(expected_expr, &mut rt.interner);
    let plan2 = plan(&normalized2, &rt.interner).expect("plan2 failed");
    let result2 = execute(&plan2, &mut rt).expect("exec2 failed");

    // Assert: shadowing == just using the second binding
    match (result1, result2) {
        (Value::Frame(f1), Value::Frame(f2)) => {
            assert_frame_equiv(&f1, &f2);
        }
        _ => panic!("Expected Frame results"),
    }
}

#[test]
fn meta_let_sequential_dependency() {
    // Property: (let ((x e1) (y f(x))) body) must see x's value when evaluating y
    // Validates: Sequential evaluation, not parallel

    let mut rt = Runtime::new();
    let mut env = Env::new();

    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
    env.bind("x", Arc::clone(&x));
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(x));

    let y = build_date_frame(43, "DATE", 15, 1, false, 0.05);
    env.bind("y", Arc::clone(&y));
    let y_sym = rt.interner.intern("y");
    rt.define(y_sym, Value::Frame(y));

    // (let ((a (dlog x)) (b (dlog a))) b)
    // This REQUIRES sequential evaluation: b depends on a
    let let_sym = rt.interner.intern("let");
    let a_sym = rt.interner.intern("a");
    let b_sym = rt.interner.intern("b");
    let dlog_sym = rt.interner.intern("dlog");
    let x_sym = rt.interner.intern("x");

    let expr = Expr::List(vec![
        Expr::Sym(let_sym),
        Expr::List(vec![
            Expr::List(vec![
                Expr::Sym(a_sym),
                Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(x_sym)]),
            ]),
            Expr::List(vec![
                Expr::Sym(b_sym),
                Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(a_sym)]),
            ]),
        ]),
        Expr::Sym(b_sym),
    ]);

    // This should NOT crash (would crash if parallel evaluation)
    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let result = execute(&ir_plan, &mut rt);

    assert!(result.is_ok(), "Sequential dependency must work");
}

// ============================================================================
// Join Semantics Metamorphics (mapr / asofr)
// ============================================================================

#[test]
fn meta_mapr_output_shape_law() {
    // Property: rows(mapr(x, y)) == rows(y) ALWAYS
    // Validates: RIGHT OUTER JOIN contract

    let mut rt = Runtime::new();

    // Create frames with DIFFERENT row counts
    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);  // 10 rows
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::clone(&x)));

    let y = build_date_frame(43, "DATE", 15, 1, false, 0.05); // 15 rows
    let y_sym = rt.interner.intern("y");
    rt.define(y_sym, Value::Frame(Arc::clone(&y)));

    // (mapr x y)
    let mapr_sym = rt.interner.intern("mapr");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");

    let expr = Expr::List(vec![
        Expr::Sym(mapr_sym),
        Expr::Sym(x_sym),
        Expr::Sym(y_sym),
    ]);

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let result = execute(&ir_plan, &mut rt).expect("exec failed");

    match result {
        Value::Frame(f) => {
            assert_eq!(f.nrows, 15, "mapr output must have y's row count (RIGHT OUTER JOIN)");
        }
        _ => panic!("Expected Frame"),
    }
}

#[test]
fn meta_asofr_output_shape_law() {
    // Property: rows(asofr(x, y)) == rows(y) ALWAYS
    // Validates: RIGHT OUTER ASOF JOIN contract

    let mut rt = Runtime::new();

    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);  // 10 rows
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::clone(&x)));

    let y = build_date_frame(43, "DATE", 15, 1, false, 0.05); // 15 rows
    let y_sym = rt.interner.intern("y");
    rt.define(y_sym, Value::Frame(Arc::clone(&y)));

    // (asofr x y)
    let asofr_sym = rt.interner.intern("asofr");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");

    let expr = Expr::List(vec![
        Expr::Sym(asofr_sym),
        Expr::Sym(x_sym),
        Expr::Sym(y_sym),
    ]);

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let result = execute(&ir_plan, &mut rt).expect("exec failed");

    match result {
        Value::Frame(f) => {
            assert_eq!(f.nrows, 15, "asofr output must have y's row count (RIGHT OUTER ASOF JOIN)");
        }
        _ => panic!("Expected Frame"),
    }
}

#[test]
fn meta_mapr_column_projection_law() {
    // Property: cols(mapr(x, y)) == cols(x)
    // Validates: mapr projects x's columns onto y's index

    let mut rt = Runtime::new();

    let x = build_date_frame(42, "DATE", 10, 3, false, 0.1);  // 3 columns
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::clone(&x)));

    let y = build_date_frame(43, "DATE", 15, 1, false, 0.05); // 1 column
    let y_sym = rt.interner.intern("y");
    rt.define(y_sym, Value::Frame(Arc::clone(&y)));

    let mapr_sym = rt.interner.intern("mapr");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");

    let expr = Expr::List(vec![
        Expr::Sym(mapr_sym),
        Expr::Sym(x_sym),
        Expr::Sym(y_sym),
    ]);

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let result = execute(&ir_plan, &mut rt).expect("exec failed");

    match result {
        Value::Frame(f) => {
            assert_eq!(f.ncols(), 3, "mapr output must have x's column count");

            // Verify column names match x's columns
            for (i, colname) in f.tags.colnames.iter().enumerate() {
                assert_eq!(colname, &format!("col{}", i), "mapr must preserve x's column names");
            }
        }
        _ => panic!("Expected Frame"),
    }
}

#[test]
fn meta_join_index_arc_identity() {
    // Property: Arc::ptr_eq(&mapr(x,y).tags.index, &y.tags.index)
    // Validates: Zero-copy index preservation (I1 contract)

    let mut rt = Runtime::new();

    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::clone(&x)));

    let y = build_date_frame(43, "DATE", 15, 1, false, 0.05);
    let y_index_ptr = Arc::as_ptr(&y.tags.index);
    let y_sym = rt.interner.intern("y");
    rt.define(y_sym, Value::Frame(Arc::clone(&y)));

    let mapr_sym = rt.interner.intern("mapr");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");

    let expr = Expr::List(vec![
        Expr::Sym(mapr_sym),
        Expr::Sym(x_sym),
        Expr::Sym(y_sym),
    ]);

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let result = execute(&ir_plan, &mut rt).expect("exec failed");

    match result {
        Value::Frame(f) => {
            let result_index_ptr = Arc::as_ptr(&f.tags.index);
            assert_eq!(
                result_index_ptr, y_index_ptr,
                "mapr MUST preserve y's index Arc (zero-copy, I1 contract)"
            );
        }
        _ => panic!("Expected Frame"),
    }
}

// ============================================================================
// Numeric Op Metamorphics (Mask Preservation, Column Independence)
// ============================================================================

#[test]
fn meta_unary_preserves_shape() {
    // Property: shape(dlog(x)) == shape(x)
    // Validates: I3 contract (nrows preserved)

    let mut rt = Runtime::new();

    let x = build_date_frame(42, "DATE", 10, 3, false, 0.1);
    let x_nrows = x.nrows;
    let x_ncols = x.ncols();
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::clone(&x)));

    let dlog_sym = rt.interner.intern("dlog");
    let x_sym = rt.interner.intern("x");

    let expr = Expr::List(vec![
        Expr::Sym(dlog_sym),
        Expr::Sym(x_sym),
    ]);

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let result = execute(&ir_plan, &mut rt).expect("exec failed");

    match result {
        Value::Frame(f) => {
            assert_eq!(f.nrows, x_nrows, "Unary op must preserve row count (I3)");
            assert_eq!(f.ncols(), x_ncols, "Unary op must preserve column count");
        }
        _ => panic!("Expected Frame"),
    }
}

#[test]
fn meta_unary_preserves_tags_arc() {
    // Property: Arc::ptr_eq(&dlog(x).tags, &x.tags) for index and colnames
    // Validates: I1-I2 contracts (Arc preservation)

    let mut rt = Runtime::new();

    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
    let x_index_ptr = Arc::as_ptr(&x.tags.index);
    let x_colnames_ptr = Arc::as_ptr(&x.tags.colnames);
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::clone(&x)));

    let dlog_sym = rt.interner.intern("dlog");
    let x_sym = rt.interner.intern("x");

    let expr = Expr::List(vec![
        Expr::Sym(dlog_sym),
        Expr::Sym(x_sym),
    ]);

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let result = execute(&ir_plan, &mut rt).expect("exec failed");

    match result {
        Value::Frame(f) => {
            let result_index_ptr = Arc::as_ptr(&f.tags.index);
            let result_colnames_ptr = Arc::as_ptr(&f.tags.colnames);

            assert_eq!(
                result_index_ptr, x_index_ptr,
                "Unary op MUST preserve index Arc (I1 contract)"
            );
            assert_eq!(
                result_colnames_ptr, x_colnames_ptr,
                "Unary op MUST preserve colnames Arc (I2 contract)"
            );
        }
        _ => panic!("Expected Frame"),
    }
}

#[test]
fn meta_unary_na_positions_monotonic() {
    // Property: NA positions in dlog(x) are superset of NA positions in x
    // (dlog adds NA at first row due to lag)
    // Validates: NA propagation is conservative (no invented values)

    let mut rt = Runtime::new();

    // Build frame with known NA pattern
    let x = build_date_frame(42, "DATE", 10, 1, false, 0.2); // 20% NA
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::clone(&x)));

    let dlog_sym = rt.interner.intern("dlog");
    let x_sym = rt.interner.intern("x");

    let expr = Expr::List(vec![
        Expr::Sym(dlog_sym),
        Expr::Sym(x_sym),
    ]);

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let result = execute(&ir_plan, &mut rt).expect("exec failed");

    match result {
        Value::Frame(f) => {
            // Get input and output columns
            let x_col = x.get_col(0).expect("x col0");
            let result_col = f.get_col(0).expect("result col0");

            use blawktrust::Column;
            if let (Column::F64(x_data), Column::F64(result_data)) = (&**x_col, &**result_col) {
                // Check: if x[i] is NA, then result[i] must be NA
                // (allowing result to have MORE NAs due to lag)
                for i in 0..x_data.len() {
                    if x_data[i].is_nan() {
                        assert!(
                            result_data[i].is_nan(),
                            "Row {}: Input NA must propagate to output (conservative NA policy)",
                            i
                        );
                    }
                }
            }
        }
        _ => panic!("Expected Frame"),
    }
}

// ============================================================================
// Normalization Idempotence & Stability
// ============================================================================

#[test]
fn meta_normalize_idempotent() {
    // Property: normalize(normalize(ast)) == normalize(ast)
    // Validates: Normalization converges to fixed point

    let mut interner = Interner::new();

    // (-> (dlog x) (mapr y))
    let expr = Expr::List(vec![
        Expr::Sym(interner.intern("->")),
        Expr::List(vec![
            Expr::Sym(interner.intern("dlog")),
            Expr::Sym(interner.intern("x")),
        ]),
        Expr::List(vec![
            Expr::Sym(interner.intern("mapr")),
            Expr::Sym(interner.intern("y")),
        ]),
    ]);

    let once = normalize(expr.clone(), &mut interner);
    let twice = normalize(once.inner().clone(), &mut interner);

    assert_eq!(
        once.inner(), twice.inner(),
        "Normalization must be idempotent"
    );
}

#[test]
fn meta_plan_deterministic() {
    // Property: plan(ast, seed1) node count == plan(ast, seed2) node count
    // Validates: No hash-map nondeterminism in planner

    let mut interner = Interner::new();

    let expr = Expr::List(vec![
        Expr::Sym(interner.intern("mapr")),
        Expr::List(vec![
            Expr::Sym(interner.intern("dlog")),
            Expr::Sym(interner.intern("x")),
        ]),
        Expr::Sym(interner.intern("y")),
    ]);

    let normalized = normalize(expr, &mut interner);

    // Plan twice
    let plan1 = plan(&normalized, &interner).expect("plan1 failed");
    let plan2 = plan(&normalized, &interner).expect("plan2 failed");

    assert_eq!(
        plan1.nodes.len(), plan2.nodes.len(),
        "Planner must be deterministic (same AST → same plan)"
    );
}

// ============================================================================
// Binary Operation Identity Laws (Algebraic Properties)
// ============================================================================

#[test]
fn meta_binary_additive_identity() {
    // Property: x + 0 == x (shape, tags, values)
    // Validates: Scalar zero doesn't change frame

    let mut rt = Runtime::new();
    let mut env = Env::new();

    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
    env.bind("x", Arc::clone(&x));
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::clone(&x)));

    // Build (+ x 0)
    let plus_sym = rt.interner.intern("+");
    let expr = Expr::List(vec![
        Expr::Sym(plus_sym),
        Expr::Sym(x_sym),
        Expr::Float(0.0),
    ]);

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let result_val = execute(&ir_plan, &mut rt).expect("execute failed");

    let result = match result_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    // Shape identity
    assert_eq!(result.nrows, x.nrows, "x + 0 must preserve nrows");
    assert_eq!(result.cols.len(), x.cols.len(), "x + 0 must preserve ncols");

    // Arc identity (I1-I3)
    assert!(Arc::ptr_eq(&result.tags.index, &x.tags.index), "x + 0: I1 violation");
    assert!(Arc::ptr_eq(&result.tags.colnames, &x.tags.colnames), "x + 0: I2 violation");

    // Value identity (within floating point tolerance)
    assert_frame_equiv(&x, &result);
}

#[test]
fn meta_binary_multiplicative_identity() {
    // Property: x * 1 == x (shape, tags, values)
    // Validates: Scalar one doesn't change frame

    let mut rt = Runtime::new();
    let mut env = Env::new();

    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
    env.bind("x", Arc::clone(&x));
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::clone(&x)));

    // Build (* x 1)
    let mul_sym = rt.interner.intern("*");
    let expr = Expr::List(vec![
        Expr::Sym(mul_sym),
        Expr::Sym(x_sym),
        Expr::Float(1.0),
    ]);

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let result_val = execute(&ir_plan, &mut rt).expect("execute failed");

    let result = match result_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    // Arc identity (I1-I3)
    assert!(Arc::ptr_eq(&result.tags.index, &x.tags.index), "x * 1: I1 violation");
    assert!(Arc::ptr_eq(&result.tags.colnames, &x.tags.colnames), "x * 1: I2 violation");

    assert_frame_equiv(&x, &result);
}

#[test]
fn meta_binary_absorption_law() {
    // Property: x * 0 yields 0 where x valid, NA where x NA (mask monotonic)
    // Validates: Conservative NA policy, no invented values

    let mut rt = Runtime::new();

    let x = build_date_frame(42, "DATE", 10, 2, false, 0.2); // 20% NA rate
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::clone(&x)));

    // Build (* x 0)
    let mul_sym = rt.interner.intern("*");
    let expr = Expr::List(vec![
        Expr::Sym(mul_sym),
        Expr::Sym(x_sym),
        Expr::Float(0.0),
    ]);

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let result_val = execute(&ir_plan, &mut rt).expect("execute failed");

    let result = match result_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    // Check each column
    for (i, result_col) in result.cols.iter().enumerate() {
        use blisp::frame::ColData;
        let result_data = match result_col {
            ColData::Mat(col) => col,
        };

        let x_data = match &x.cols[i] {
            ColData::Mat(col) => col,
        };

        match (x_data.as_ref(), result_data.as_ref()) {
            (blawktrust::Column::F64(x_vals), blawktrust::Column::F64(result_vals)) => {
                for j in 0..x_vals.len() {
                    if x_vals[j].is_nan() {
                        assert!(result_vals[j].is_nan(),
                            "x * 0: Input NA must propagate (row {}, col {})", j, i);
                    } else {
                        assert_eq!(result_vals[j], 0.0,
                            "x * 0: Valid input must become 0 (row {}, col {})", j, i);
                    }
                }
            }
            _ => panic!("Expected F64 columns"),
        }
    }
}

#[test]
fn meta_binary_preserves_lhs_tags() {
    // Property: (op x scalar).tags == x.tags (pointer equality)
    // Validates: Binary ops preserve LHS tags Arc (I1-I3)

    let mut rt = Runtime::new();

    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
    let x_sym = rt.interner.intern("x");
    let x_index_ptr = Arc::as_ptr(&x.tags.index);
    let x_colnames_ptr = Arc::as_ptr(&x.tags.colnames);
    rt.define(x_sym, Value::Frame(Arc::clone(&x)));

    // Test all four ops
    for (op_name, op_val) in &[("+", 5.0), ("-", 2.0), ("*", 3.0), ("/", 2.0)] {
        let op_sym = rt.interner.intern(op_name);
        let expr = Expr::List(vec![
            Expr::Sym(op_sym),
            Expr::Sym(x_sym),
            Expr::Float(*op_val),
        ]);

        let normalized = normalize(expr, &mut rt.interner);
        let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
        let result_val = execute(&ir_plan, &mut rt).expect("execute failed");

        let result = match result_val {
            Value::Frame(f) => f,
            _ => panic!("Expected Frame"),
        };

        // Arc pointer equality
        let result_index_ptr = Arc::as_ptr(&result.tags.index);
        let result_colnames_ptr = Arc::as_ptr(&result.tags.colnames);

        assert_eq!(result_index_ptr, x_index_ptr,
            "{} must preserve index Arc (I1)", op_name);
        assert_eq!(result_colnames_ptr, x_colnames_ptr,
            "{} must preserve colnames Arc (I2)", op_name);
    }
}

#[test]
fn meta_binary_na_propagation() {
    // Property: mask(x op y) == mask(x) ∧ mask(y)
    // Validates: Conservative NA policy for frame-frame ops

    let mut rt = Runtime::new();

    // Create two frames with different NA patterns
    let x = build_date_frame(42, "DATE", 10, 2, false, 0.15);
    let y = build_date_frame(42, "DATE", 10, 2, false, 0.20); // Same shape, same index

    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");
    rt.define(x_sym, Value::Frame(Arc::clone(&x)));
    rt.define(y_sym, Value::Frame(Arc::clone(&y)));

    // Build (+ x y)
    let plus_sym = rt.interner.intern("+");
    let expr = Expr::List(vec![
        Expr::Sym(plus_sym),
        Expr::Sym(x_sym),
        Expr::Sym(y_sym),
    ]);

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let result_val = execute(&ir_plan, &mut rt).expect("execute failed");

    let result = match result_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    // Check NA propagation
    for (i, ((x_col, y_col), result_col)) in x.cols.iter()
        .zip(y.cols.iter())
        .zip(result.cols.iter())
        .enumerate() {

        use blisp::frame::ColData;
        let x_data = match x_col { ColData::Mat(col) => col };
        let y_data = match y_col { ColData::Mat(col) => col };
        let result_data = match result_col { ColData::Mat(col) => col };

        match (x_data.as_ref(), y_data.as_ref(), result_data.as_ref()) {
            (blawktrust::Column::F64(x_vals),
             blawktrust::Column::F64(y_vals),
             blawktrust::Column::F64(result_vals)) => {
                for j in 0..x_vals.len() {
                    if x_vals[j].is_nan() || y_vals[j].is_nan() {
                        assert!(result_vals[j].is_nan(),
                            "x + y: Either input NA must propagate (row {}, col {})", j, i);
                    }
                    // Note: We don't assert !NA → !NA because operations can create NA (e.g., div by 0)
                }
            }
            _ => panic!("Expected F64 columns"),
        }
    }
}

// ============================================================================
// Shift Operation Laws (Time Series Foundation)
// ============================================================================

#[test]
fn meta_shift_zero_identity() {
    // Property: shift(0, x) == x (exact, including Arc ptr_eq)
    // Validates: k=0 special case, no spurious copies

    let mut rt = Runtime::new();

    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
    let x_sym = rt.interner.intern("x");
    let x_index_ptr = Arc::as_ptr(&x.tags.index);
    let x_colnames_ptr = Arc::as_ptr(&x.tags.colnames);
    rt.define(x_sym, Value::Frame(Arc::clone(&x)));

    // Build (shift 0 x)
    let shift_sym = rt.interner.intern("shift");
    let expr = Expr::List(vec![
        Expr::Sym(shift_sym),
        Expr::Int(0),
        Expr::Sym(x_sym),
    ]);

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let result_val = execute(&ir_plan, &mut rt).expect("execute failed");

    let result = match result_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    // Arc pointer equality
    let result_index_ptr = Arc::as_ptr(&result.tags.index);
    let result_colnames_ptr = Arc::as_ptr(&result.tags.colnames);

    assert_eq!(result_index_ptr, x_index_ptr, "shift 0: I1 violation");
    assert_eq!(result_colnames_ptr, x_colnames_ptr, "shift 0: I2 violation");

    // Value equality
    assert_frame_equiv(&x, &result);
}

#[test]
fn meta_shift_composition_law() {
    // Property: shift(a, shift(b, x)) == shift(a+b, x)
    // Validates: Compositionality, associativity

    let mut rt = Runtime::new();
    let mut env = Env::new();

    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
    env.bind("x", Arc::clone(&x));
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(x));

    let shift_sym = rt.interner.intern("shift");

    // LHS: (shift 2 (shift 3 x))
    let lhs_expr = Expr::List(vec![
        Expr::Sym(shift_sym),
        Expr::Int(2),
        Expr::List(vec![
            Expr::Sym(shift_sym),
            Expr::Int(3),
            Expr::Sym(x_sym),
        ]),
    ]);

    // RHS: (shift 5 x)
    let rhs_expr = Expr::List(vec![
        Expr::Sym(shift_sym),
        Expr::Int(5),
        Expr::Sym(x_sym),
    ]);

    // Evaluate both
    let lhs = common::direct_eval(&lhs_expr, &env, &rt.interner).expect("LHS eval failed");

    let normalized_rhs = normalize(rhs_expr, &mut rt.interner);
    let rhs_plan = plan(&normalized_rhs, &rt.interner).expect("RHS plan failed");
    let rhs_val = execute(&rhs_plan, &mut rt).expect("RHS execute failed");
    let rhs = match rhs_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    assert_frame_equiv(&lhs, &rhs);
}

#[test]
fn meta_shift_mask_monotonic() {
    // Property: NA positions only grow with k > 0
    // Validates: No invented values, conservative NA policy

    let mut rt = Runtime::new();

    let x = build_date_frame(42, "DATE", 10, 2, false, 0.15); // 15% NA
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::clone(&x)));

    // Build (shift 2 x)
    let shift_sym = rt.interner.intern("shift");
    let expr = Expr::List(vec![
        Expr::Sym(shift_sym),
        Expr::Int(2),
        Expr::Sym(x_sym),
    ]);

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let result_val = execute(&ir_plan, &mut rt).expect("execute failed");

    let result = match result_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    // Check each column: input NA → output NA (monotone)
    for (i, (x_col, result_col)) in x.cols.iter().zip(result.cols.iter()).enumerate() {
        use blisp::frame::ColData;
        let x_data = match x_col { ColData::Mat(col) => col };
        let result_data = match result_col { ColData::Mat(col) => col };

        match (x_data.as_ref(), result_data.as_ref()) {
            (blawktrust::Column::F64(x_vals), blawktrust::Column::F64(result_vals)) => {
                // First 2 rows must be NA (shift introduces)
                for j in 0..2 {
                    assert!(result_vals[j].is_nan(),
                        "shift 2: First {} rows must be NA (col {})", 2, i);
                }

                // For rows j >= 2: if input[j-2] is NA, output[j] must be NA
                for j in 2..result_vals.len() {
                    if x_vals[j - 2].is_nan() {
                        assert!(result_vals[j].is_nan(),
                            "shift 2: Input NA at {} must propagate to output at {} (col {})",
                            j - 2, j, i);
                    }
                }
            }
            _ => panic!("Expected F64 columns"),
        }
    }
}

#[test]
fn meta_shift_preserves_tags_arc() {
    // Property: shift preserves LHS tags Arc (I1-I3)
    // Validates: Zero-copy semantics

    let mut rt = Runtime::new();

    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
    let x_sym = rt.interner.intern("x");
    let x_index_ptr = Arc::as_ptr(&x.tags.index);
    let x_colnames_ptr = Arc::as_ptr(&x.tags.colnames);
    rt.define(x_sym, Value::Frame(x));

    // Test various k values
    for k in [1, 2, 5] {
        let shift_sym = rt.interner.intern("shift");
        let expr = Expr::List(vec![
            Expr::Sym(shift_sym),
            Expr::Int(k),
            Expr::Sym(x_sym),
        ]);

        let normalized = normalize(expr, &mut rt.interner);
        let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
        let result_val = execute(&ir_plan, &mut rt).expect("execute failed");

        let result = match result_val {
            Value::Frame(f) => f,
            _ => panic!("Expected Frame"),
        };

        let result_index_ptr = Arc::as_ptr(&result.tags.index);
        let result_colnames_ptr = Arc::as_ptr(&result.tags.colnames);

        assert_eq!(result_index_ptr, x_index_ptr,
            "shift {}: I1 violation - index Arc not preserved", k);
        assert_eq!(result_colnames_ptr, x_colnames_ptr,
            "shift {}: I2 violation - colnames Arc not preserved", k);
    }
}

#[test]
fn meta_shift_all_na_when_k_exceeds_nrows() {
    // Property: shift(k, x) yields all NA when k >= nrows
    // Validates: Edge case handling

    let mut rt = Runtime::new();

    let x = build_date_frame(42, "DATE", 8, 2, false, 0.0); // 8 rows, no NA
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::clone(&x)));

    // Build (shift 10 x) where 10 > 8
    let shift_sym = rt.interner.intern("shift");
    let expr = Expr::List(vec![
        Expr::Sym(shift_sym),
        Expr::Int(10),
        Expr::Sym(x_sym),
    ]);

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let result_val = execute(&ir_plan, &mut rt).expect("execute failed");

    let result = match result_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    // All cells must be NA
    for (i, result_col) in result.cols.iter().enumerate() {
        use blisp::frame::ColData;
        let result_data = match result_col { ColData::Mat(col) => col };

        match result_data.as_ref() {
            blawktrust::Column::F64(vals) => {
                for (j, &val) in vals.iter().enumerate() {
                    assert!(val.is_nan(),
                        "shift(k >= nrows) must yield all NA (row {}, col {})", j, i);
                }
            }
            _ => panic!("Expected F64 column"),
        }
    }
}

// ============================================================================
// Time-Series Identities (Foundation for Rolling Ops)
// ============================================================================

#[test]
fn meta_dlog_identity_positive_domain() {
    // Property: dlog(x) == log(x / shift(1, x))
    // Validates: Shift sign convention, div-by-zero, NA propagation
    //
    // This is the STRONGEST semantic tripwire for time-series correctness.
    // Catches: off-by-one, sign errors, NA/div0 edge cases
    
    let mut rt = Runtime::new();
    
    // Build positive-valued frame to stay in log domain
    // Values in (0.1, 100.0) with controlled NA rate
    let seed = 12345_u64;
    let nrows = 20;
    let ncols = 2;
    
    // Generate positive values
    let mut rng = seed;
    let mut dates = Vec::with_capacity(nrows);
    for i in 0..nrows {
        dates.push(20200101 + i as i32);
    }
    
    let index = IndexColumn::Date(Arc::new(dates));
    
    let mut cols_data = Vec::new();
    for _col in 0..ncols {
        let mut col_vals = Vec::with_capacity(nrows);
        for _row in 0..nrows {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            
            // 10% NA rate
            if (rng % 100) < 10 {
                col_vals.push(f64::NAN);
            } else {
                // Positive values in (0.1, 100.0)
                let val = 0.1 + ((rng % 10000) as f64) / 100.0;
                col_vals.push(val);
            }
        }
        
        
        cols_data.push(blisp::frame::ColData::Mat(Arc::new(blawktrust::Column::F64(col_vals))));
    }
    
    let colnames = (0..ncols).map(|i| format!("col{}", i)).collect();
    
    let tags = blisp::frame::Tags::new(
        "DATE".to_string(),
        index,
        colnames,
    );
    
    let frame = blisp::frame::Frame {
        tags: Arc::new(tags),
        cols: cols_data,
        nrows,
    };
    
    let x_sym = rt.interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::new(frame)));
    
    // LHS: (dlog x)
    let dlog_sym = rt.interner.intern("dlog");
    let lhs_expr = Expr::List(vec![
        Expr::Sym(dlog_sym),
        Expr::Sym(x_sym),
    ]);
    
    // RHS: (log (/ x (shift 1 x)))
    let log_sym = rt.interner.intern("log");
    let div_sym = rt.interner.intern("/");
    let shift_sym = rt.interner.intern("shift");
    let rhs_expr = Expr::List(vec![
        Expr::Sym(log_sym),
        Expr::List(vec![
            Expr::Sym(div_sym),
            Expr::Sym(x_sym),
            Expr::List(vec![
                Expr::Sym(shift_sym),
                Expr::Int(1),
                Expr::Sym(x_sym),
            ]),
        ]),
    ]);
    
    // Evaluate both via IR
    let lhs_normalized = normalize(lhs_expr, &mut rt.interner);
    let lhs_plan = plan(&lhs_normalized, &rt.interner).expect("LHS plan failed");
    let lhs_val = execute(&lhs_plan, &mut rt).expect("LHS execute failed");
    let lhs = match lhs_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };
    
    let rhs_normalized = normalize(rhs_expr, &mut rt.interner);
    let rhs_plan = plan(&rhs_normalized, &rt.interner).expect("RHS plan failed");
    let rhs_val = execute(&rhs_plan, &mut rt).expect("RHS execute failed");
    let rhs = match rhs_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };
    
    // Layer 1: Tags/shape equivalence
    assert_eq!(lhs.nrows, rhs.nrows, "dlog identity: nrows mismatch");
    assert_eq!(lhs.cols.len(), rhs.cols.len(), "dlog identity: ncols mismatch");
    
    // Layer 2: Mask equivalence (NA patterns must match exactly)
    for (col_idx, (lhs_col, rhs_col)) in lhs.cols.iter().zip(rhs.cols.iter()).enumerate() {
        use blisp::frame::ColData;
        let lhs_data = match lhs_col { ColData::Mat(col) => col };
        let rhs_data = match rhs_col { ColData::Mat(col) => col };
        
        match (lhs_data.as_ref(), rhs_data.as_ref()) {
            (blawktrust::Column::F64(lhs_vals), blawktrust::Column::F64(rhs_vals)) => {
                for (row_idx, (&l, &r)) in lhs_vals.iter().zip(rhs_vals.iter()).enumerate() {
                    // Mask equivalence: is_na(lhs) == is_na(rhs)
                    assert_eq!(
                        l.is_nan(), r.is_nan(),
                        "dlog identity: NA mask mismatch at row {}, col {}",
                        row_idx, col_idx
                    );
                    
                    // Layer 3: Value equivalence for non-NA cells
                    if !l.is_nan() && !r.is_nan() {
                        let diff = (l - r).abs();
                        assert!(
                            diff < 1e-10,
                            "dlog identity: value mismatch at row {}, col {}: {} vs {} (diff: {})",
                            row_idx, col_idx, l, r, diff
                        );
                    }
                }
            }
            _ => panic!("Expected F64 columns"),
        }
    }
}
