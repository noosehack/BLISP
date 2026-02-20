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
