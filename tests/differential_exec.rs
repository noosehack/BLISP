//! Differential Execution Testing
//!
//! Two-way oracle comparison on small frames (5-80 rows):
//! 1. AST direct_eval (reference interpreter - simple, obviously correct)
//! 2. IR execute (optimized executor with fusion potential)
//!
//! These must agree on all small test cases.
//! Catches fusion bugs, buffering errors, and semantic drift.

mod common;

use blisp::ast::Expr;
use blisp::exec::execute;
use blisp::normalize::normalize;
use blisp::planner::plan;
use blisp::runtime::Runtime;
use blisp::value::Value;
use common::{assert_frame_equiv, build_date_frame, build_timestamp_frame, direct_eval, Env};
use proptest::prelude::*;
use std::sync::Arc;

// ============================================================================
// Deterministic Differential Tests (Small Shapes)
// ============================================================================

#[test]
fn diff_small_unary_dlog() {
    let mut rt = Runtime::new();
    let x_sym = rt.interner.intern("x");

    let mut env = Env::new();
    let x = build_date_frame(42, "DATE", 8, 2, false, 0.1);
    env.bind("x", Arc::clone(&x));
    rt.define(x_sym, Value::Frame(x));

    let dlog_sym = rt.interner.intern("dlog");
    let expr = Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(x_sym)]);

    let ast_result = direct_eval(&expr, &env, &rt.interner).expect("AST eval failed");

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let ir_result = match execute(&ir_plan, &mut rt).expect("IR execute failed") {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    assert_frame_equiv(&ast_result, &ir_result);
}

#[test]
fn diff_small_join_mapr() {
    let mut rt = Runtime::new();
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");

    let mut env = Env::new();
    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
    env.bind("x", Arc::clone(&x));
    rt.define(x_sym, Value::Frame(x));

    let y = build_date_frame(43, "DATE", 12, 1, false, 0.05);
    env.bind("y", Arc::clone(&y));
    rt.define(y_sym, Value::Frame(y));

    let mapr_sym = rt.interner.intern("mapr");
    let expr = Expr::List(vec![
        Expr::Sym(mapr_sym),
        Expr::Sym(x_sym),
        Expr::Sym(y_sym),
    ]);

    let ast_result = direct_eval(&expr, &env, &rt.interner).expect("AST eval failed");

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let ir_result = match execute(&ir_plan, &mut rt).expect("IR execute failed") {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    assert_frame_equiv(&ast_result, &ir_result);
}

#[test]
fn diff_small_join_asofr() {
    let mut rt = Runtime::new();
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");

    let mut env = Env::new();
    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
    env.bind("x", Arc::clone(&x));
    rt.define(x_sym, Value::Frame(x));

    let y = build_date_frame(43, "DATE", 12, 1, false, 0.05);
    env.bind("y", Arc::clone(&y));
    rt.define(y_sym, Value::Frame(y));

    let asofr_sym = rt.interner.intern("asofr");
    let expr = Expr::List(vec![
        Expr::Sym(asofr_sym),
        Expr::Sym(x_sym),
        Expr::Sym(y_sym),
    ]);

    let ast_result = direct_eval(&expr, &env, &rt.interner).expect("AST eval failed");

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let ir_result = match execute(&ir_plan, &mut rt).expect("IR execute failed") {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    assert_frame_equiv(&ast_result, &ir_result);
}

#[test]
fn diff_small_pipeline() {
    let mut rt = Runtime::new();
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");

    let mut env = Env::new();
    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
    env.bind("x", Arc::clone(&x));
    rt.define(x_sym, Value::Frame(x));

    let y = build_date_frame(43, "DATE", 12, 1, false, 0.05);
    env.bind("y", Arc::clone(&y));
    rt.define(y_sym, Value::Frame(y));

    // (mapr (dlog x) y)
    let mapr_sym = rt.interner.intern("mapr");
    let dlog_sym = rt.interner.intern("dlog");
    let expr = Expr::List(vec![
        Expr::Sym(mapr_sym),
        Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(x_sym)]),
        Expr::Sym(y_sym),
    ]);

    let ast_result = direct_eval(&expr, &env, &rt.interner).expect("AST eval failed");

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let ir_result = match execute(&ir_plan, &mut rt).expect("IR execute failed") {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    assert_frame_equiv(&ast_result, &ir_result);
}

#[test]
fn diff_small_nested_pipeline() {
    let mut rt = Runtime::new();
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");
    let z_sym = rt.interner.intern("z");

    let mut env = Env::new();
    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
    env.bind("x", Arc::clone(&x));
    rt.define(x_sym, Value::Frame(x));

    let y = build_date_frame(43, "DATE", 12, 1, false, 0.05);
    env.bind("y", Arc::clone(&y));
    rt.define(y_sym, Value::Frame(y));

    let z = build_date_frame(44, "DATE", 8, 3, true, 0.15);
    env.bind("z", Arc::clone(&z));
    rt.define(z_sym, Value::Frame(z));

    // (asofr (mapr (dlog x) y) z)
    let asofr_sym = rt.interner.intern("asofr");
    let mapr_sym = rt.interner.intern("mapr");
    let dlog_sym = rt.interner.intern("dlog");
    let expr = Expr::List(vec![
        Expr::Sym(asofr_sym),
        Expr::List(vec![
            Expr::Sym(mapr_sym),
            Expr::List(vec![Expr::Sym(dlog_sym), Expr::Sym(x_sym)]),
            Expr::Sym(y_sym),
        ]),
        Expr::Sym(z_sym),
    ]);

    let ast_result = direct_eval(&expr, &env, &rt.interner).expect("AST eval failed");

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let ir_result = match execute(&ir_plan, &mut rt).expect("IR execute failed") {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    assert_frame_equiv(&ast_result, &ir_result);
}

#[test]
fn diff_small_let_binding() {
    let mut rt = Runtime::new();
    let x_sym = rt.interner.intern("x");
    let y_sym = rt.interner.intern("y");

    let mut env = Env::new();
    let x = build_date_frame(42, "DATE", 10, 2, false, 0.1);
    env.bind("x", Arc::clone(&x));
    rt.define(x_sym, Value::Frame(x));

    let y = build_date_frame(43, "DATE", 12, 1, false, 0.05);
    env.bind("y", Arc::clone(&y));
    rt.define(y_sym, Value::Frame(y));

    // (let ((a (dlog x))) (mapr a y))
    let let_sym = rt.interner.intern("let");
    let a_sym = rt.interner.intern("a");
    let mapr_sym = rt.interner.intern("mapr");
    let dlog_sym = rt.interner.intern("dlog");

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

    let ast_result = direct_eval(&expr, &env, &rt.interner).expect("AST eval failed");

    let normalized = normalize(expr, &mut rt.interner);
    let ir_plan = plan(&normalized, &rt.interner).expect("plan failed");
    let ir_result = match execute(&ir_plan, &mut rt).expect("IR execute failed") {
        Value::Frame(f) => f,
        _ => panic!("Expected Frame"),
    };

    assert_frame_equiv(&ast_result, &ir_result);
}

// ============================================================================
// Property-Based Differential Tests (Small Shapes)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 100,  // Focus on small shapes with many variations
        .. ProptestConfig::default()
    })]

    #[test]
    #[ignore = "Random expr generator hits unimplemented AST ops. Will be replaced by proper differential fuzz (step 3) with approved safe-ops subset."]
    fn diff_prop_small_date_frames(
        seed in any::<u64>(),
        depth in 0usize..4,  // Shallower than main equivalence tests
        nrows_x in 5usize..30,
        nrows_y in 5usize..30,
        ncols_x in 1usize..4,
        with_dups in any::<bool>(),
        na_rate in prop::num::f64::POSITIVE | prop::num::f64::ZERO,
    ) {
        let mut rt = Runtime::new();

        let x_sym = rt.interner.intern("x");
        let y_sym = rt.interner.intern("y");
        let z_sym = rt.interner.intern("z");

        let mut env = Env::new();

        let x = build_date_frame(seed, "DATE", nrows_x, ncols_x, with_dups, na_rate.min(0.3));
        env.bind("x", Arc::clone(&x));
        rt.define(x_sym, Value::Frame(x));

        let y = build_date_frame(seed.wrapping_add(1), "DATE", nrows_y, 1, false, na_rate.min(0.2));
        env.bind("y", Arc::clone(&y));
        rt.define(y_sym, Value::Frame(y));

        let z = build_date_frame(seed.wrapping_add(2), "DATE", 8, 2, with_dups, na_rate.min(0.25));
        env.bind("z", Arc::clone(&z));
        rt.define(z_sym, Value::Frame(z));

        let expr = common::gen_expr_date(seed.wrapping_add(3), depth, &mut rt.interner);

        let ast_result = direct_eval(&expr, &env, &rt.interner)
            .expect("AST eval failed");

        let normalized = normalize(expr, &mut rt.interner);
        let ir_plan = plan(&normalized, &rt.interner)
            .expect("plan failed");

        let ir_result = match execute(&ir_plan, &mut rt).expect("IR execute failed") {
            Value::Frame(f) => f,
            _ => panic!("Expected Frame"),
        };

        assert_frame_equiv(&ast_result, &ir_result);
    }

    #[test]
    #[ignore = "Random expr generator hits unimplemented AST ops. Will be replaced by proper differential fuzz (step 3) with approved safe-ops subset."]
    fn diff_prop_small_timestamp_frames(
        seed in any::<u64>(),
        depth in 0usize..4,
        nrows_x in 5usize..30,
        nrows_y in 5usize..30,
        ncols_x in 1usize..4,
        with_dups in any::<bool>(),
        na_rate in prop::num::f64::POSITIVE | prop::num::f64::ZERO,
    ) {
        let mut rt = Runtime::new();

        let x_sym = rt.interner.intern("x");
        let y_sym = rt.interner.intern("y");
        let z_sym = rt.interner.intern("z");

        let mut env = Env::new();

        let x = build_timestamp_frame(seed, "TIMESTAMP", nrows_x, ncols_x, with_dups, na_rate.min(0.3));
        env.bind("x", Arc::clone(&x));
        rt.define(x_sym, Value::Frame(x));

        let y = build_timestamp_frame(seed.wrapping_add(1), "TIMESTAMP", nrows_y, 1, false, na_rate.min(0.2));
        env.bind("y", Arc::clone(&y));
        rt.define(y_sym, Value::Frame(y));

        let z = build_timestamp_frame(seed.wrapping_add(2), "TIMESTAMP", 8, 2, with_dups, na_rate.min(0.25));
        env.bind("z", Arc::clone(&z));
        rt.define(z_sym, Value::Frame(z));

        let expr = common::gen_expr_date(seed.wrapping_add(3), depth, &mut rt.interner);

        let ast_result = direct_eval(&expr, &env, &rt.interner)
            .expect("AST eval failed");

        let normalized = normalize(expr, &mut rt.interner);
        let ir_plan = plan(&normalized, &rt.interner)
            .expect("plan failed");

        let ir_result = match execute(&ir_plan, &mut rt).expect("IR execute failed") {
            Value::Frame(f) => f,
            _ => panic!("Expected Frame"),
        };

        assert_frame_equiv(&ast_result, &ir_result);
    }
}
