//! IR Equivalence Property Tests
//!
//! Verifies that:
//!   direct_eval(expr) == execute(plan(normalize(expr)))
//!
//! Where "==" is defined by assert_frame_equiv (contracts.md semantics).
//!
//! This is the CRITICAL test that validates the IR layer preserves semantics.

mod common;

use blisp::normalize::normalize;
use blisp::planner::plan;
use blisp::exec::execute;
use blisp::runtime::Runtime;
use blisp::value::Value;
use common::{
    assert_frame_equiv, gen_expr_date, direct_eval, Env,
};
use proptest::prelude::*;
use std::sync::Arc;

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 300,
        .. ProptestConfig::default()
    })]

    #[test]
    fn ir_equiv_date_frames(
        seed in any::<u64>(),
        depth in 0usize..5
    ) {
        // Create runtime FIRST (owns the interner)
        let mut rt = Runtime::new();

        // Intern symbols first to avoid borrowing issues
        let x_sym = rt.interner.intern("x");
        let y_sym = rt.interner.intern("y");
        let z_sym = rt.interner.intern("z");

        // Generate environment using runtime's interner
        let mut env = Env::new();

        // Build frames and bind in both env and runtime
        let x = common::build_date_frame(seed, "DATE", 10, 2, false, 0.1);
        env.bind("x", Arc::clone(&x));
        rt.define(x_sym, Value::Frame(x));

        let y = common::build_date_frame(seed.wrapping_add(1), "DATE", 15, 1, false, 0.05);
        env.bind("y", Arc::clone(&y));
        rt.define(y_sym, Value::Frame(y));

        let z = common::build_date_frame(seed.wrapping_add(2), "DATE", 8, 3, true, 0.15);
        env.bind("z", Arc::clone(&z));
        rt.define(z_sym, Value::Frame(z));

        // Generate expression using runtime's interner
        let expr = gen_expr_date(seed.wrapping_add(3), depth, &mut rt.interner);

        // Direct evaluation (uses runtime's interner)
        let direct = direct_eval(&expr, &env, &rt.interner)
            .expect("direct eval failed");

        // IR path
        let normalized = normalize(expr, &mut rt.interner);
        let ir_plan = plan(&normalized, &rt.interner)
            .expect("plan failed");

        let via_ir_value = execute(&ir_plan, &mut rt)
            .expect("execute failed");

        let via_ir = match via_ir_value {
            Value::Frame(f) => f,
            _ => panic!("IR execution returned non-Frame"),
        };

        // Assert equivalence
        assert_frame_equiv(&direct, &via_ir);
    }
}

// Smoke tests moved to ir_equivalence_smoke.rs for clarity
