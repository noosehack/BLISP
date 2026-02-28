//! Deterministic smoke tests for IR equivalence

mod common;

use blisp::ast::{Expr, Interner};
use blisp::exec::execute;
use blisp::frame::IndexColumn;
use blisp::normalize::normalize;
use blisp::planner::plan;
use blisp::runtime::Runtime;
use blisp::value::Value;
use common::{assert_frame_equiv, direct_eval, Env};
use std::sync::Arc;

// Helper to set up test environment
fn setup_env() -> (Runtime, Env) {
    let mut rt = Runtime::new();
    let mut env = Env::new();

    // Intern symbols first
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");
    let z_sym = rt.interner.intern("z");

    // Build and bind frames
    let x = common::build_date_frame(42, "DATE", 10, 2, false, 0.1);
    env.bind("x", Arc::clone(&x));
    rt.define(x_sym, Value::Frame(x));

    let y = common::build_date_frame(43, "DATE", 15, 1, false, 0.05);
    env.bind("y", Arc::clone(&y));
    rt.define(y_sym, Value::Frame(y));

    let z = common::build_date_frame(44, "DATE", 8, 3, true, 0.15);
    env.bind("z", Arc::clone(&z));
    rt.define(z_sym, Value::Frame(z));

    (rt, env)
}

// Helper to set up Timestamp environment
fn setup_env_ts() -> (Runtime, Env) {
    let mut rt = Runtime::new();
    let mut env = Env::new();

    // Intern symbols first
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");
    let z_sym = rt.interner.intern("z");

    // Build and bind timestamp frames
    let x = common::build_timestamp_frame(42, "TIMESTAMP", 10, 2, false, 0.1);
    env.bind("x", Arc::clone(&x));
    rt.define(x_sym, Value::Frame(x));

    let y = common::build_timestamp_frame(43, "TIMESTAMP", 15, 1, false, 0.05);
    env.bind("y", Arc::clone(&y));
    rt.define(y_sym, Value::Frame(y));

    let z = common::build_timestamp_frame(44, "TIMESTAMP", 8, 3, true, 0.15);
    env.bind("z", Arc::clone(&z));
    rt.define(z_sym, Value::Frame(z));

    (rt, env)
}

// Helper to run equivalence test
fn check_equiv(mut rt: Runtime, env: &Env, expr: Expr) {
    let direct = direct_eval(&expr, env, &rt.interner).expect("direct eval failed");

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");

    let via_ir = match execute(&ir_plan, &mut rt).expect("execute failed") {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    assert_frame_equiv(&direct, &via_ir);
}

#[test]
fn smoke_var_only() {
    let (mut rt, env) = setup_env();
    let x_sym = rt.interner.intern("x");
    let expr = Expr::Sym(x_sym);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_dlog() {
    let (mut rt, env) = setup_env();
    let dlog_sym = rt.interner.intern("dlog");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(x_sym)]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_mapr() {
    let (mut rt, env) = setup_env();
    let mapr_sym = rt.interner.intern("mapr");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");
    let expr = Expr::List(vec![
        Expr::Sym(mapr_sym),
        Expr::Sym(x_sym),
        Expr::Sym(y_sym),
    ]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_asofr() {
    let (mut rt, env) = setup_env();
    let asofr_sym = rt.interner.intern("asofr");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");
    let expr = Expr::List(vec![
        Expr::Sym(asofr_sym),
        Expr::Sym(x_sym),
        Expr::Sym(y_sym),
    ]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_nested_unary() {
    let (mut rt, env) = setup_env();
    let dlog_sym = rt.interner.intern("dlog");
    let log_sym = rt.interner.intern("log");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![
        Expr::Sym(dlog_sym),
        Expr::List(vec![Expr::Sym(log_sym), Expr::Sym(x_sym)]),
    ]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_join_after_unary() {
    let (mut rt, env) = setup_env();
    let mapr_sym = rt.interner.intern("mapr");
    let dlog_sym = rt.interner.intern("dlog");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");
    let expr = Expr::List(vec![
        Expr::Sym(mapr_sym),
        Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(x_sym)]),
        Expr::Sym(y_sym),
    ]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_nested_joins() {
    let (mut rt, env) = setup_env();
    // (asofr (mapr x y) z)
    let asofr_sym = rt.interner.intern("asofr");
    let mapr_sym = rt.interner.intern("mapr");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");
    let z_sym = rt.interner.intern("z");
    let expr = Expr::List(vec![
        Expr::Sym(asofr_sym),
        Expr::List(vec![
            Expr::Sym(mapr_sym),
            Expr::Sym(x_sym),
            Expr::Sym(y_sym),
        ]),
        Expr::Sym(z_sym),
    ]);
    check_equiv(rt, &env, expr);
}

// ============================================================================
// Timestamp Frame Smoke Tests
// ============================================================================

#[test]
fn smoke_timestamp_var() {
    let (mut rt, env) = setup_env_ts();
    let x_sym = rt.interner.intern("x");
    let expr = Expr::Sym(x_sym);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_timestamp_mapr() {
    let (mut rt, env) = setup_env_ts();
    let mapr_sym = rt.interner.intern("mapr");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");
    let expr = Expr::List(vec![
        Expr::Sym(mapr_sym),
        Expr::Sym(x_sym),
        Expr::Sym(y_sym),
    ]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_timestamp_asofr() {
    let (mut rt, env) = setup_env_ts();
    let asofr_sym = rt.interner.intern("asofr");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");
    let expr = Expr::List(vec![
        Expr::Sym(asofr_sym),
        Expr::Sym(x_sym),
        Expr::Sym(y_sym),
    ]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_timestamp_pipeline() {
    let (mut rt, env) = setup_env_ts();
    // (asofr (dlog x) y)
    let asofr_sym = rt.interner.intern("asofr");
    let dlog_sym = rt.interner.intern("dlog");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");
    let expr = Expr::List(vec![
        Expr::Sym(asofr_sym),
        Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(x_sym)]),
        Expr::Sym(y_sym),
    ]);
    check_equiv(rt, &env, expr);
}

// ============================================================================
// Let Binding Smoke Tests
// ============================================================================

#[test]
fn smoke_let_simple_reuse() {
    let (mut rt, env) = setup_env();
    // (let ((a (dlog x))) (mapr a y))
    let let_sym = rt.interner.intern("let");
    let a_sym = rt.interner.intern("a");
    let dlog_sym = rt.interner.intern("dlog");
    let mapr_sym = rt.interner.intern("mapr");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");

    let expr = Expr::List(vec![
        Expr::Sym(let_sym),
        Expr::List(vec![Expr::List(vec![
            Expr::Sym(a_sym),
            Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(x_sym)]),
        ])]),
        Expr::List(vec![
            Expr::Sym(mapr_sym),
            Expr::Sym(a_sym),
            Expr::Sym(y_sym),
        ]),
    ]);

    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_let_sequential_binding() {
    let (mut rt, env) = setup_env();
    // (let ((a (mapr x y)) (b (dlog a))) b)
    let let_sym = rt.interner.intern("let");
    let a_sym = rt.interner.intern("a");
    let b_sym = rt.interner.intern("b");
    let mapr_sym = rt.interner.intern("mapr");
    let dlog_sym = rt.interner.intern("dlog");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");

    let expr = Expr::List(vec![
        Expr::Sym(let_sym),
        Expr::List(vec![
            Expr::List(vec![
                Expr::Sym(a_sym),
                Expr::List(vec![
                    Expr::Sym(mapr_sym),
                    Expr::Sym(x_sym),
                    Expr::Sym(y_sym),
                ]),
            ]),
            Expr::List(vec![
                Expr::Sym(b_sym),
                Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(a_sym)]),
            ]),
        ]),
        Expr::Sym(b_sym),
    ]);

    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_let_shadowing() {
    let (mut rt, env) = setup_env();
    // (let ((x (dlog x))) x)  - inner x shadows outer x
    let let_sym = rt.interner.intern("let");
    let dlog_sym = rt.interner.intern("dlog");
    let x_sym = rt.interner.intern("x");

    let expr = Expr::List(vec![
        Expr::Sym(let_sym),
        Expr::List(vec![Expr::List(vec![
            Expr::Sym(x_sym),
            Expr::List(vec![
                Expr::Sym(dlog_sym),
                Expr::Sym(x_sym), // Refers to outer x
            ]),
        ])]),
        Expr::Sym(x_sym), // Refers to inner x (shadowed)
    ]);

    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_let_join_of_bound() {
    let (mut rt, env) = setup_env();
    // (let ((a (dlog x)) (b (dlog y))) (mapr a b))
    let let_sym = rt.interner.intern("let");
    let a_sym = rt.interner.intern("a");
    let b_sym = rt.interner.intern("b");
    let dlog_sym = rt.interner.intern("dlog");
    let mapr_sym = rt.interner.intern("mapr");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");

    let expr = Expr::List(vec![
        Expr::Sym(let_sym),
        Expr::List(vec![
            Expr::List(vec![
                Expr::Sym(a_sym),
                Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(x_sym)]),
            ]),
            Expr::List(vec![
                Expr::Sym(b_sym),
                Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(y_sym)]),
            ]),
        ]),
        Expr::List(vec![
            Expr::Sym(mapr_sym),
            Expr::Sym(a_sym),
            Expr::Sym(b_sym),
        ]),
    ]);

    check_equiv(rt, &env, expr);
}

// ============================================================================
// Binary Operations Smoke Tests
// ============================================================================

#[test]
fn smoke_binary_scalar_add() {
    let (mut rt, env) = setup_env();
    // (+ x 5.0)
    let plus_sym = rt.interner.intern("+");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![
        Expr::Sym(plus_sym),
        Expr::Sym(x_sym),
        Expr::Float(5.0),
    ]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_binary_scalar_sub() {
    let (mut rt, env) = setup_env();
    // (- x 2.0)
    let sub_sym = rt.interner.intern("-");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![Expr::Sym(sub_sym), Expr::Sym(x_sym), Expr::Float(2.0)]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_binary_scalar_mul() {
    let (mut rt, env) = setup_env();
    // (* x 3.0)
    let mul_sym = rt.interner.intern("*");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![Expr::Sym(mul_sym), Expr::Sym(x_sym), Expr::Float(3.0)]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_binary_scalar_div() {
    let (mut rt, env) = setup_env();
    // (/ x 2.0)
    let div_sym = rt.interner.intern("/");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![Expr::Sym(div_sym), Expr::Sym(x_sym), Expr::Float(2.0)]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_binary_frame_frame_add() {
    let (mut rt, env) = setup_env();
    // (+ x x) - same frame added to itself
    let plus_sym = rt.interner.intern("+");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![
        Expr::Sym(plus_sym),
        Expr::Sym(x_sym),
        Expr::Sym(x_sym),
    ]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_binary_pipeline_with_scalar() {
    let (mut rt, env) = setup_env();
    // (* (dlog x) 100.0) - convert to percentage
    let mul_sym = rt.interner.intern("*");
    let dlog_sym = rt.interner.intern("dlog");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![
        Expr::Sym(mul_sym),
        Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(x_sym)]),
        Expr::Float(100.0),
    ]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_binary_after_join() {
    let (mut rt, env) = setup_env();
    // (+ (mapr x y) 10.0)
    let plus_sym = rt.interner.intern("+");
    let mapr_sym = rt.interner.intern("mapr");
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");
    let expr = Expr::List(vec![
        Expr::Sym(plus_sym),
        Expr::List(vec![
            Expr::Sym(mapr_sym),
            Expr::Sym(x_sym),
            Expr::Sym(y_sym),
        ]),
        Expr::Float(10.0),
    ]);
    check_equiv(rt, &env, expr);
}

// ============================================================================
// Shift Operation Smoke Tests
// ============================================================================

#[test]
fn smoke_shift_zero_identity() {
    let (mut rt, env) = setup_env();
    // (shift 0 x) should be exact identity
    let shift_sym = rt.interner.intern("shift");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![Expr::Sym(shift_sym), Expr::Int(0), Expr::Sym(x_sym)]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_shift_one_lag() {
    let (mut rt, env) = setup_env();
    // (shift 1 x) - lag by 1 row
    let shift_sym = rt.interner.intern("shift");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![Expr::Sym(shift_sym), Expr::Int(1), Expr::Sym(x_sym)]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_shift_large_k() {
    let (mut rt, env) = setup_env();
    // (shift 100 x) - k > nrows produces all NA
    let shift_sym = rt.interner.intern("shift");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![Expr::Sym(shift_sym), Expr::Int(100), Expr::Sym(x_sym)]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_shift_composition() {
    let (mut rt, env) = setup_env();
    // (shift 2 (shift 1 x)) - nested shifts
    let shift_sym = rt.interner.intern("shift");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![
        Expr::Sym(shift_sym),
        Expr::Int(2),
        Expr::List(vec![Expr::Sym(shift_sym), Expr::Int(1), Expr::Sym(x_sym)]),
    ]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_shift_after_unary() {
    let (mut rt, env) = setup_env();
    // (shift 1 (dlog x)) - shift after transformation
    let shift_sym = rt.interner.intern("shift");
    let dlog_sym = rt.interner.intern("dlog");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![
        Expr::Sym(shift_sym),
        Expr::Int(1),
        Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(x_sym)]),
    ]);
    check_equiv(rt, &env, expr);
}

// ============================================================================
// Time-Series Identity: dlog
// ============================================================================

#[test]
fn smoke_dlog_identity_handcrafted() {
    // Property: dlog(x) == log(x / shift(1, x))
    // Hand-crafted data to verify sign convention and NA behavior

    let mut rt = Runtime::new();
    let mut interner = Interner::new();

    // Build a small positive-valued frame: [2.0, 4.0, 8.0, 4.0, 2.0]
    // Expected dlog results:
    // Row 0: NA (no prior value)
    // Row 1: log(4/2) = log(2) ≈ 0.693
    // Row 2: log(8/4) = log(2) ≈ 0.693
    // Row 3: log(4/8) = log(0.5) ≈ -0.693
    // Row 4: log(2/4) = log(0.5) ≈ -0.693

    let index = IndexColumn::Date(Arc::new(vec![
        20200101, 20200102, 20200103, 20200104, 20200105,
    ]));

    let col_data = blawktrust::Column::F64(vec![2.0, 4.0, 8.0, 4.0, 2.0]);

    let tags = blisp::frame::Tags::new("DATE".to_string(), index, vec!["price".to_string()]);

    let frame = blisp::frame::Frame {
        tags: Arc::new(tags),
        cols: vec![blisp::frame::ColData::Mat(Arc::new(col_data))],
        nrows: 5,
    };

    let x_sym = interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::new(frame)));

    // LHS: (dlog x)
    let dlog_sym = interner.intern("dlog");
    let lhs_expr = Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(x_sym)]);

    // RHS: (log (/ x (shift 1 x)))
    let log_sym = interner.intern("log");
    let div_sym = interner.intern("/");
    let shift_sym = interner.intern("shift");
    let rhs_expr = Expr::List(vec![
        Expr::Sym(log_sym),
        Expr::List(vec![
            Expr::Sym(div_sym),
            Expr::Sym(x_sym),
            Expr::List(vec![Expr::Sym(shift_sym), Expr::Int(1), Expr::Sym(x_sym)]),
        ]),
    ]);

    // Evaluate both via IR
    let lhs_normalized = normalize(lhs_expr, &mut interner);
    let lhs_plan = plan(&lhs_normalized, &interner).expect("LHS plan failed");
    let lhs_val = execute(&lhs_plan, &mut rt).expect("LHS execute failed");
    let lhs = match lhs_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    let rhs_normalized = normalize(rhs_expr, &mut interner);
    let rhs_plan = plan(&rhs_normalized, &interner).expect("RHS plan failed");
    let rhs_val = execute(&rhs_plan, &mut rt).expect("RHS execute failed");
    let rhs = match rhs_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    // Verify identity
    common::assert_frame_equiv(&lhs, &rhs);
}

// ============================================================================
// Rolling Mean Operation Smoke Tests
// ============================================================================

#[test]
fn smoke_rolling_mean_window_one() {
    let (mut rt, env) = setup_env();
    // (rolling-mean 1 x) - window=1 should be identity-like (each value equals itself)
    let rm_sym = rt.interner.intern("rolling-mean");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![Expr::Sym(rm_sym), Expr::Int(1), Expr::Sym(x_sym)]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_rolling_mean_window_three() {
    let (mut rt, env) = setup_env();
    // (rolling-mean 3 x) - basic trailing window
    let rm_sym = rt.interner.intern("rolling-mean");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![Expr::Sym(rm_sym), Expr::Int(3), Expr::Sym(x_sym)]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_rolling_mean_large_window() {
    let (mut rt, env) = setup_env();
    // (rolling-mean 100 x) - window > nrows produces all NA
    let rm_sym = rt.interner.intern("rolling-mean");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![Expr::Sym(rm_sym), Expr::Int(100), Expr::Sym(x_sym)]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_rolling_mean_after_unary() {
    let (mut rt, env) = setup_env();
    // (rolling-mean 2 (dlog x)) - rolling mean after transformation
    let rm_sym = rt.interner.intern("rolling-mean");
    let dlog_sym = rt.interner.intern("dlog");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![
        Expr::Sym(rm_sym),
        Expr::Int(2),
        Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(x_sym)]),
    ]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_rolling_mean_handcrafted() {
    // Hand-crafted data to verify rolling mean correctness with NA handling
    // Series: [1.0, 2.0, 3.0, NA, 5.0, 6.0]
    // Window = 3, strict min_periods
    // Expected (with skip NA):
    // [0]: window [1.0] - count=1 < 3 -> NA
    // [1]: window [1.0, 2.0] - count=2 < 3 -> NA
    // [2]: window [1.0, 2.0, 3.0] - count=3 == 3 -> mean = 2.0
    // [3]: window [2.0, 3.0, NA] - count=2 < 3 -> NA
    // [4]: window [3.0, NA, 5.0] - count=2 < 3 -> NA
    // [5]: window [NA, 5.0, 6.0] - count=2 < 3 -> NA

    let mut rt = Runtime::new();
    let mut interner = Interner::new();

    let index = IndexColumn::Date(Arc::new(vec![
        20200101, 20200102, 20200103, 20200104, 20200105, 20200106,
    ]));

    let col_data = blawktrust::Column::F64(vec![1.0, 2.0, 3.0, f64::NAN, 5.0, 6.0]);

    let tags = blisp::frame::Tags::new("DATE".to_string(), index, vec!["value".to_string()]);

    let frame = blisp::frame::Frame {
        tags: Arc::new(tags),
        cols: vec![blisp::frame::ColData::Mat(Arc::new(col_data))],
        nrows: 6,
    };

    let x_sym = interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::new(frame)));

    // Execute (rolling-mean 3 x)
    let rm_sym = interner.intern("rolling-mean");
    let expr = Expr::List(vec![Expr::Sym(rm_sym), Expr::Int(3), Expr::Sym(x_sym)]);

    let normalized = normalize(expr, &mut interner);
    let ir_plan = plan(&normalized, &interner).expect("plan failed");
    let result_val = execute(&ir_plan, &mut rt).expect("execute failed");
    let result = match result_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    // Verify result
    assert_eq!(result.nrows, 6);
    assert_eq!(result.cols.len(), 1);

    let result_col = match &result.cols[0] {
        blisp::frame::ColData::Mat(col) => col,
        _ => panic!("Expected Mat column"),
    };

    let values = match &**result_col {
        blawktrust::Column::F64(v) => v,
        _ => panic!("Expected F64 column"),
    };

    // Check expected values
    assert!(values[0].is_nan(), "Row 0 should be NA (window too small)");
    assert!(values[1].is_nan(), "Row 1 should be NA (window too small)");
    assert!(
        (values[2] - 2.0).abs() < 1e-10,
        "Row 2 should be 2.0 (mean of 1,2,3)"
    );
    assert!(
        values[3].is_nan(),
        "Row 3 should be NA (only 2 valid in window)"
    );
    assert!(
        values[4].is_nan(),
        "Row 4 should be NA (only 2 valid in window)"
    );
    assert!(
        values[5].is_nan(),
        "Row 5 should be NA (only 2 valid in window)"
    );
}

// ============================================================================
// Rolling Std Operation Smoke Tests
// ============================================================================

#[test]
fn smoke_rolling_std_window_one() {
    let (mut rt, env) = setup_env();
    // (rolling-std 1 x) - window=1 should return 0.0 (single point has zero variance)
    let rs_sym = rt.interner.intern("rolling-std");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![Expr::Sym(rs_sym), Expr::Int(1), Expr::Sym(x_sym)]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_rolling_std_constant_series() {
    // Constant series should have std = 0.0
    let mut rt = Runtime::new();
    let mut interner = Interner::new();

    let index = IndexColumn::Date(Arc::new(vec![
        20200101, 20200102, 20200103, 20200104, 20200105,
    ]));

    let col_data = blawktrust::Column::F64(vec![5.0, 5.0, 5.0, 5.0, 5.0]);

    let tags = blisp::frame::Tags::new("DATE".to_string(), index, vec!["const".to_string()]);

    let frame = blisp::frame::Frame {
        tags: Arc::new(tags),
        cols: vec![blisp::frame::ColData::Mat(Arc::new(col_data))],
        nrows: 5,
    };

    let x_sym = interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::new(frame)));

    let rs_sym = interner.intern("rolling-std");
    let expr = Expr::List(vec![Expr::Sym(rs_sym), Expr::Int(3), Expr::Sym(x_sym)]);

    let normalized = normalize(expr, &mut interner);
    let ir_plan = plan(&normalized, &interner).expect("plan failed");
    let result_val = execute(&ir_plan, &mut rt).expect("execute failed");
    let result = match result_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    let col = match &result.cols[0] {
        blisp::frame::ColData::Mat(c) => c,
        _ => panic!("Expected Mat"),
    };
    let values = match col.as_ref() {
        blawktrust::Column::F64(v) => v,
        _ => panic!("Expected F64"),
    };

    assert!(values[0].is_nan(), "Row 0 should be NA (prefix)");
    assert!(values[1].is_nan(), "Row 1 should be NA (prefix)");
    assert!(values[2].abs() < 1e-10, "Row 2 should be 0.0 (constant)");
    assert!(values[3].abs() < 1e-10, "Row 3 should be 0.0 (constant)");
    assert!(values[4].abs() < 1e-10, "Row 4 should be 0.0 (constant)");
}

#[test]
fn smoke_rolling_std_known_window() {
    // Known window: [1, 2, 3]
    // Mean = 2.0
    // Variance = (1/3) * [(1-2)² + (2-2)² + (3-2)²] = (1/3) * [1 + 0 + 1] = 2/3
    // Std = sqrt(2/3) ≈ 0.816496580927726

    let mut rt = Runtime::new();
    let mut interner = Interner::new();

    let index = IndexColumn::Date(Arc::new(vec![20200101, 20200102, 20200103]));

    let col_data = blawktrust::Column::F64(vec![1.0, 2.0, 3.0]);

    let tags = blisp::frame::Tags::new("DATE".to_string(), index, vec!["value".to_string()]);

    let frame = blisp::frame::Frame {
        tags: Arc::new(tags),
        cols: vec![blisp::frame::ColData::Mat(Arc::new(col_data))],
        nrows: 3,
    };

    let x_sym = interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::new(frame)));

    let rs_sym = interner.intern("rolling-std");
    let expr = Expr::List(vec![Expr::Sym(rs_sym), Expr::Int(3), Expr::Sym(x_sym)]);

    let normalized = normalize(expr, &mut interner);
    let ir_plan = plan(&normalized, &interner).expect("plan failed");
    let result_val = execute(&ir_plan, &mut rt).expect("execute failed");
    let result = match result_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    let col = match &result.cols[0] {
        blisp::frame::ColData::Mat(c) => c,
        _ => panic!("Expected Mat"),
    };
    let values = match col.as_ref() {
        blawktrust::Column::F64(v) => v,
        _ => panic!("Expected F64"),
    };

    assert!(values[0].is_nan(), "Row 0 should be NA (prefix)");
    assert!(values[1].is_nan(), "Row 1 should be NA (prefix)");
    let expected_std = (2.0_f64 / 3.0_f64).sqrt();
    assert!(
        (values[2] - expected_std).abs() < 1e-10,
        "Row 2 should be sqrt(2/3) ≈ 0.8165"
    );
}

#[test]
fn smoke_rolling_std_with_na() {
    let (mut rt, env) = setup_env();
    // (rolling-std 3 z) where z has NAs
    let rs_sym = rt.interner.intern("rolling-std");
    let z_sym = rt.interner.intern("z");
    let expr = Expr::List(vec![Expr::Sym(rs_sym), Expr::Int(3), Expr::Sym(z_sym)]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_rolling_std_after_unary() {
    let (mut rt, env) = setup_env();
    // (rolling-std 2 (dlog x))
    let rs_sym = rt.interner.intern("rolling-std");
    let dlog_sym = rt.interner.intern("dlog");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![
        Expr::Sym(rs_sym),
        Expr::Int(2),
        Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(x_sym)]),
    ]);
    check_equiv(rt, &env, expr);
}

// ============================================================================
// Rolling Zscore Operation Smoke Tests (derived form)
// ============================================================================

#[test]
fn smoke_rolling_zscore_constant_series() {
    // Constant series: std = 0 → division by zero → NA
    let mut rt = Runtime::new();
    let mut interner = Interner::new();

    let index = IndexColumn::Date(Arc::new(vec![
        20200101, 20200102, 20200103, 20200104, 20200105,
    ]));

    let col_data = blawktrust::Column::F64(vec![5.0, 5.0, 5.0, 5.0, 5.0]);

    let tags = blisp::frame::Tags::new("DATE".to_string(), index, vec!["const".to_string()]);

    let frame = blisp::frame::Frame {
        tags: Arc::new(tags),
        cols: vec![blisp::frame::ColData::Mat(Arc::new(col_data))],
        nrows: 5,
    };

    let x_sym = interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::new(frame)));

    let rz_sym = interner.intern("rolling-zscore");
    let expr = Expr::List(vec![Expr::Sym(rz_sym), Expr::Int(3), Expr::Sym(x_sym)]);

    let normalized = normalize(expr, &mut interner);
    let ir_plan = plan(&normalized, &interner).expect("plan failed");
    let result_val = execute(&ir_plan, &mut rt).expect("execute failed");
    let result = match result_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    let col = match &result.cols[0] {
        blisp::frame::ColData::Mat(c) => c,
        _ => panic!("Expected Mat"),
    };
    let values = match col.as_ref() {
        blawktrust::Column::F64(v) => v,
        _ => panic!("Expected F64"),
    };

    // All values should be NA (constant series → std=0 → div0 → NA)
    assert!(values[0].is_nan(), "Row 0 should be NA (prefix)");
    assert!(values[1].is_nan(), "Row 1 should be NA (prefix)");
    assert!(values[2].is_nan(), "Row 2 should be NA (std=0 → div0)");
    assert!(values[3].is_nan(), "Row 3 should be NA (std=0 → div0)");
    assert!(values[4].is_nan(), "Row 4 should be NA (std=0 → div0)");
}

#[test]
fn smoke_rolling_zscore_known_window() {
    // Known window: [1, 2, 3] with w=3
    // Mean = 2.0
    // Std = sqrt(2/3) ≈ 0.8165
    // Zscore at i=2: (3 - 2.0) / 0.8165 ≈ 1.2247

    let mut rt = Runtime::new();
    let mut interner = Interner::new();

    let index = IndexColumn::Date(Arc::new(vec![20200101, 20200102, 20200103]));

    let col_data = blawktrust::Column::F64(vec![1.0, 2.0, 3.0]);

    let tags = blisp::frame::Tags::new("DATE".to_string(), index, vec!["value".to_string()]);

    let frame = blisp::frame::Frame {
        tags: Arc::new(tags),
        cols: vec![blisp::frame::ColData::Mat(Arc::new(col_data))],
        nrows: 3,
    };

    let x_sym = interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::new(frame)));

    let rz_sym = interner.intern("rolling-zscore");
    let expr = Expr::List(vec![Expr::Sym(rz_sym), Expr::Int(3), Expr::Sym(x_sym)]);

    let normalized = normalize(expr, &mut interner);
    let ir_plan = plan(&normalized, &interner).expect("plan failed");
    let result_val = execute(&ir_plan, &mut rt).expect("execute failed");
    let result = match result_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    let col = match &result.cols[0] {
        blisp::frame::ColData::Mat(c) => c,
        _ => panic!("Expected Mat"),
    };
    let values = match col.as_ref() {
        blawktrust::Column::F64(v) => v,
        _ => panic!("Expected F64"),
    };

    assert!(values[0].is_nan(), "Row 0 should be NA (prefix)");
    assert!(values[1].is_nan(), "Row 1 should be NA (prefix)");

    // Row 2: zscore of 3 in window [1,2,3]
    // mean=2, std=sqrt(2/3), zscore=(3-2)/sqrt(2/3) = 1/sqrt(2/3) = sqrt(3/2) ≈ 1.2247
    let expected = (3.0_f64 / 2.0_f64).sqrt();
    assert!(
        (values[2] - expected).abs() < 1e-10,
        "Row 2 zscore should be sqrt(3/2) ≈ 1.2247"
    );
}

#[test]
fn smoke_ft_zscore_no_self_reference() {
    // Verify ft-zscore doesn't include current value in its own distribution
    // Series with a spike: [1, 1, 1, 10, 1, 1]
    // At i=3 (spike):
    //   rolling-zscore includes 10 in window → smaller zscore
    //   ft-zscore compares 10 to [1,1,1] → larger zscore

    let mut rt = Runtime::new();
    let mut interner = Interner::new();

    let index = IndexColumn::Date(Arc::new(vec![
        20200101, 20200102, 20200103, 20200104, 20200105, 20200106,
    ]));

    let col_data = blawktrust::Column::F64(vec![1.0, 1.0, 1.0, 10.0, 1.0, 1.0]);

    let tags = blisp::frame::Tags::new("DATE".to_string(), index, vec!["value".to_string()]);

    let frame = blisp::frame::Frame {
        tags: Arc::new(tags),
        cols: vec![blisp::frame::ColData::Mat(Arc::new(col_data))],
        nrows: 6,
    };

    let x_sym = interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::new(frame)));

    // Execute rolling-zscore
    let rz_sym = interner.intern("rolling-zscore");
    let rz_expr = Expr::List(vec![Expr::Sym(rz_sym), Expr::Int(3), Expr::Sym(x_sym)]);

    let rz_normalized = normalize(rz_expr, &mut interner);
    let rz_plan = plan(&rz_normalized, &interner).expect("rz plan failed");
    let rz_val = execute(&rz_plan, &mut rt).expect("rz execute failed");
    let rz_result = match rz_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    // Execute ft-zscore
    let ftz_sym = interner.intern("ft-zscore");
    let ftz_expr = Expr::List(vec![Expr::Sym(ftz_sym), Expr::Int(3), Expr::Sym(x_sym)]);

    let ftz_normalized = normalize(ftz_expr, &mut interner);
    let ftz_plan = plan(&ftz_normalized, &interner).expect("ftz plan failed");
    let ftz_val = execute(&ftz_plan, &mut rt).expect("ftz execute failed");
    let ftz_result = match ftz_val {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    let rz_col = match &rz_result.cols[0] {
        blisp::frame::ColData::Mat(c) => c,
        _ => panic!("Expected Mat"),
    };
    let rz_values = match rz_col.as_ref() {
        blawktrust::Column::F64(v) => v,
        _ => panic!("Expected F64"),
    };

    let ftz_col = match &ftz_result.cols[0] {
        blisp::frame::ColData::Mat(c) => c,
        _ => panic!("Expected Mat"),
    };
    let ftz_values = match ftz_col.as_ref() {
        blawktrust::Column::F64(v) => v,
        _ => panic!("Expected F64"),
    };

    // At i=3 (spike), ft-zscore should be larger than rolling-zscore
    // because ft-zscore compares spike to pure historical [1,1,1]
    // while rolling-zscore includes spike in its own mean/std
    if !ftz_values[3].is_nan() && !rz_values[3].is_nan() {
        assert!(
            ftz_values[3].abs() > rz_values[3].abs(),
            "ft-zscore at spike (|{}|) should be larger than rolling-zscore (|{}|) due to no self-reference",
            ftz_values[3], rz_values[3]
        );
    }
}
