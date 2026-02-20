//! Deterministic smoke tests for IR equivalence

mod common;

use blisp::normalize::normalize;
use blisp::planner::plan;
use blisp::exec::execute;
use blisp::runtime::Runtime;
use blisp::value::Value;
use blisp::ast::{Expr, Interner};
use blisp::frame::IndexColumn;
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
    let expr = Expr::List(vec![
        Expr::Sym(dlog_sym),
        Expr::Sym(x_sym),
    ]);
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
        Expr::List(vec![
            Expr::Sym(log_sym),
            Expr::Sym(x_sym),
        ]),
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
        Expr::List(vec![
            Expr::Sym(dlog_sym),
            Expr::Sym(x_sym),
        ]),
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
        Expr::List(vec![
            Expr::Sym(dlog_sym),
            Expr::Sym(x_sym),
        ]),
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
        Expr::List(vec![
            Expr::List(vec![
                Expr::Sym(a_sym),
                Expr::List(vec![
                    Expr::Sym(dlog_sym),
                    Expr::Sym(x_sym),
                ]),
            ]),
        ]),
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
                Expr::List(vec![
                    Expr::Sym(dlog_sym),
                    Expr::Sym(a_sym),
                ]),
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
        Expr::List(vec![
            Expr::List(vec![
                Expr::Sym(x_sym),
                Expr::List(vec![
                    Expr::Sym(dlog_sym),
                    Expr::Sym(x_sym),  // Refers to outer x
                ]),
            ]),
        ]),
        Expr::Sym(x_sym),  // Refers to inner x (shadowed)
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
                Expr::List(vec![
                    Expr::Sym(dlog_sym),
                    Expr::Sym(x_sym),
                ]),
            ]),
            Expr::List(vec![
                Expr::Sym(b_sym),
                Expr::List(vec![
                    Expr::Sym(dlog_sym),
                    Expr::Sym(y_sym),
                ]),
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
    let expr = Expr::List(vec![
        Expr::Sym(sub_sym),
        Expr::Sym(x_sym),
        Expr::Float(2.0),
    ]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_binary_scalar_mul() {
    let (mut rt, env) = setup_env();
    // (* x 3.0)
    let mul_sym = rt.interner.intern("*");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![
        Expr::Sym(mul_sym),
        Expr::Sym(x_sym),
        Expr::Float(3.0),
    ]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_binary_scalar_div() {
    let (mut rt, env) = setup_env();
    // (/ x 2.0)
    let div_sym = rt.interner.intern("/");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![
        Expr::Sym(div_sym),
        Expr::Sym(x_sym),
        Expr::Float(2.0),
    ]);
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
        Expr::List(vec![
            Expr::Sym(dlog_sym),
            Expr::Sym(x_sym),
        ]),
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
    let expr = Expr::List(vec![
        Expr::Sym(shift_sym),
        Expr::Int(0),
        Expr::Sym(x_sym),
    ]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_shift_one_lag() {
    let (mut rt, env) = setup_env();
    // (shift 1 x) - lag by 1 row
    let shift_sym = rt.interner.intern("shift");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![
        Expr::Sym(shift_sym),
        Expr::Int(1),
        Expr::Sym(x_sym),
    ]);
    check_equiv(rt, &env, expr);
}

#[test]
fn smoke_shift_large_k() {
    let (mut rt, env) = setup_env();
    // (shift 100 x) - k > nrows produces all NA
    let shift_sym = rt.interner.intern("shift");
    let x_sym = rt.interner.intern("x");
    let expr = Expr::List(vec![
        Expr::Sym(shift_sym),
        Expr::Int(100),
        Expr::Sym(x_sym),
    ]);
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
        Expr::List(vec![
            Expr::Sym(shift_sym),
            Expr::Int(1),
            Expr::Sym(x_sym),
        ]),
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
        Expr::List(vec![
            Expr::Sym(dlog_sym),
            Expr::Sym(x_sym),
        ]),
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
    
    let tags = blisp::frame::Tags::new(
        "DATE".to_string(),
        index,
        vec!["price".to_string()],
    );
    
    let frame = blisp::frame::Frame {
        tags: Arc::new(tags),
        cols: vec![blisp::frame::ColData::Mat(Arc::new(col_data))],
        nrows: 5,
    };
    
    let x_sym = interner.intern("x");
    rt.define(x_sym, Value::Frame(Arc::new(frame)));
    
    // LHS: (dlog x)
    let dlog_sym = interner.intern("dlog");
    let lhs_expr = Expr::List(vec![
        Expr::Sym(dlog_sym),
        Expr::Sym(x_sym),
    ]);
    
    // RHS: (log (/ x (shift 1 x)))
    let log_sym = interner.intern("log");
    let div_sym = interner.intern("/");
    let shift_sym = interner.intern("shift");
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
