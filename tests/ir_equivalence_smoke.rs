//! Deterministic smoke tests for IR equivalence

mod common;

use blisp::normalize::normalize;
use blisp::planner::plan;
use blisp::exec::execute;
use blisp::runtime::Runtime;
use blisp::value::Value;
use blisp::ast::Expr;
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
