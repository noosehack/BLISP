/// BLADE Phase 3: IR Executor
///
/// Purpose: Execute validated IR plans using ONLY frozen primitives
///
/// Contract enforcement:
/// - Uses ONLY: map_numeric_preserve_tags, reindex_by, mapr, asofr
/// - NO ad-hoc kernel calls
/// - NO schema manipulation outside primitives
/// - Arc preservation verified at runtime
///
/// This is where Phase 2's frozen API earns its keep.

use crate::ir::{Plan, Node, NodeId, Operation, Source, UnaryOp, JoinOp, NumericFunc};
use crate::frame::{Frame, map_numeric_preserve_tags, asofr};
use crate::value::Value;
use crate::runtime::Runtime;
use crate::io;
use std::sync::Arc;
use std::collections::HashMap;
use blawktrust::builtins::ops::{dlog_column};

/// Execution context - holds intermediate values during execution
pub struct ExecContext {
    /// Map from NodeId to computed Frame
    values: HashMap<NodeId, Arc<Frame>>,
}

impl ExecContext {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn store(&mut self, id: NodeId, frame: Arc<Frame>) {
        self.values.insert(id, frame);
    }

    pub fn load(&self, id: NodeId) -> Option<Arc<Frame>> {
        self.values.get(&id).cloned()
    }
}

/// Execute a plan and return the final result
///
/// The plan MUST be validated before execution (call plan.validate())
pub fn execute(plan: &Plan, rt: &mut Runtime) -> Result<Value, String> {
    let mut ctx = ExecContext::new();

    // Execute nodes in order (they're already topologically sorted)
    for node in &plan.nodes {
        let frame = execute_node(node, &ctx, rt)?;
        ctx.store(node.id, frame);
    }

    // Return the last node's result
    let last_id = NodeId(plan.nodes.len() - 1);
    ctx.load(last_id)
        .map(|f| Value::Frame(f))
        .ok_or_else(|| "No result from execution".to_string())
}

/// Execute a single node
fn execute_node(
    node: &Node,
    ctx: &ExecContext,
    rt: &mut Runtime,
) -> Result<Arc<Frame>, String> {
    match &node.op {
        Operation::Source(source) => execute_source(source, rt),
        Operation::Unary(unary) => execute_unary(unary, ctx),
        Operation::Join(join) => execute_join(join, ctx),
    }
}

/// Execute a source operation
fn execute_source(source: &Source, rt: &mut Runtime) -> Result<Arc<Frame>, String> {
    match source {
        Source::File { path } => {
            // Use the frozen CSV loader from io module
            let value = io::load_csv(path, &mut rt.interner)?;
            match value {
                Value::Frame(f) => Ok(f),
                _ => Err(format!("CSV loader returned non-Frame: {}", value.type_name())),
            }
        }
        Source::Variable { name } => {
            // Load from runtime environment
            let value = rt.resolve(*name)?;
            match value {
                Value::Frame(f) => Ok(f),
                _ => Err(format!("Variable is not a Frame: {}", value.type_name())),
            }
        }
    }
}

/// Execute a unary operation using ONLY map_numeric_preserve_tags
fn execute_unary(unary: &UnaryOp, ctx: &ExecContext) -> Result<Arc<Frame>, String> {
    match unary {
        UnaryOp::MapNumeric { input, func } => {
            let input_frame = ctx.load(*input)
                .ok_or_else(|| format!("Input node {:?} not found", input))?;

            // Execute using ONLY the frozen primitive
            let result = map_numeric_preserve_tags(&input_frame, |col| {
                match func {
                    NumericFunc::Dlog => dlog_column(col, 1),
                    NumericFunc::Ret => ret_column(col, 1),
                    NumericFunc::Log => log_column(col),
                    NumericFunc::Exp => exp_column(col),
                    NumericFunc::Sqrt => sqrt_column(col),
                    NumericFunc::Abs => abs_column(col),
                    NumericFunc::Inv => inv_column(col),
                }
            });

            // Verify Arc preservation (I1-I2)
            debug_assert!(
                Arc::ptr_eq(&result.tags.index, &input_frame.tags.index),
                "I1 violation: index Arc not preserved"
            );
            debug_assert!(
                Arc::ptr_eq(&result.tags.colnames, &input_frame.tags.colnames),
                "I2 violation: colnames Arc not preserved"
            );
            debug_assert_eq!(
                result.nrows, input_frame.nrows,
                "I3 violation: nrows not preserved"
            );

            Ok(Arc::new(result))
        }
    }
}

/// Execute a join operation using ONLY frozen join primitives
fn execute_join(join: &JoinOp, ctx: &ExecContext) -> Result<Arc<Frame>, String> {
    match join {
        JoinOp::MapR { x, y } => {
            let x_frame = ctx.load(*x)
                .ok_or_else(|| format!("X node {:?} not found", x))?;
            let y_frame = ctx.load(*y)
                .ok_or_else(|| format!("Y node {:?} not found", y))?;

            // Use frozen mapr primitive (RIGHT OUTER JOIN)
            let result = crate::frame::reindex_by(&x_frame, Arc::clone(&y_frame.tags.index));

            // Verify join contracts
            debug_assert!(
                Arc::ptr_eq(&result.tags.index, &y_frame.tags.index),
                "mapr contract violation: output index != y's index"
            );
            debug_assert!(
                Arc::ptr_eq(&result.tags.colnames, &x_frame.tags.colnames),
                "mapr contract violation: output colnames != x's colnames"
            );
            debug_assert_eq!(
                result.nrows, y_frame.nrows,
                "mapr contract violation: output nrows != y's nrows"
            );

            Ok(Arc::new(result))
        }

        JoinOp::AsofR { x, y } => {
            let x_frame = ctx.load(*x)
                .ok_or_else(|| format!("X node {:?} not found", x))?;
            let y_frame = ctx.load(*y)
                .ok_or_else(|| format!("Y node {:?} not found", y))?;

            // Use frozen asofr primitive (RIGHT OUTER ASOF JOIN)
            let result = asofr(&x_frame, &y_frame);

            // Verify asof contracts
            debug_assert!(
                Arc::ptr_eq(&result.tags.index, &y_frame.tags.index),
                "asofr contract violation: output index != y's index"
            );
            debug_assert!(
                Arc::ptr_eq(&result.tags.colnames, &x_frame.tags.colnames),
                "asofr contract violation: output colnames != x's colnames"
            );
            debug_assert_eq!(
                result.nrows, y_frame.nrows,
                "asofr contract violation: output nrows != y's nrows"
            );

            Ok(Arc::new(result))
        }
    }
}

// ============================================================================
// Kernel functions (will eventually come from blawktrust)
// ============================================================================

use blawktrust::Column;

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

fn exp_column(col: &Column) -> Column {
    match col {
        Column::F64(data) => {
            let result = data.iter().map(|&x| {
                if !x.is_nan() {
                    x.exp()
                } else {
                    f64::NAN
                }
            }).collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

fn sqrt_column(col: &Column) -> Column {
    match col {
        Column::F64(data) => {
            let result = data.iter().map(|&x| {
                if x >= 0.0 && !x.is_nan() {
                    x.sqrt()
                } else {
                    f64::NAN
                }
            }).collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

fn abs_column(col: &Column) -> Column {
    match col {
        Column::F64(data) => {
            let result = data.iter().map(|&x| {
                if !x.is_nan() {
                    x.abs()
                } else {
                    f64::NAN
                }
            }).collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

fn inv_column(col: &Column) -> Column {
    match col {
        Column::F64(data) => {
            let result = data.iter().map(|&x| {
                if !x.is_nan() && x != 0.0 {
                    1.0 / x
                } else {
                    f64::NAN
                }
            }).collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::plan;
    use crate::normalize::normalize;
    use crate::ast::{Expr, Interner};
    use std::io::Write;

    fn setup_test_csv(path: &str, content: &str) {
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_exec_file_source() {
        let test_file = "/tmp/test_exec_source.csv";
        setup_test_csv(test_file, "DATE;price\n2020-01-01;100\n2020-01-02;102\n");

        let mut interner = Interner::new();
        let mut rt = Runtime::new();

        let expr = Expr::List(vec![
            Expr::Sym(interner.intern("read-csv")),
            Expr::Str(test_file.to_string()),
        ]);

        let normalized = normalize(expr, &mut interner);
        let plan_obj = plan(&normalized, &interner).unwrap();
        let result = execute(&plan_obj, &mut rt);

        assert!(result.is_ok());
        match result.unwrap() {
            Value::Frame(f) => {
                assert_eq!(f.nrows, 2);
                assert_eq!(f.tags.colnames.len(), 1);
            }
            _ => panic!("Expected Frame"),
        }

        std::fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_exec_dlog() {
        let test_file = "/tmp/test_exec_dlog.csv";
        setup_test_csv(test_file, "DATE;price\n2020-01-01;100\n2020-01-02;102\n2020-01-03;105\n");

        let mut interner = Interner::new();
        let mut rt = Runtime::new();

        // (dlog (read-csv "..."))
        let expr = Expr::List(vec![
            Expr::Sym(interner.intern("dlog")),
            Expr::List(vec![
                Expr::Sym(interner.intern("read-csv")),
                Expr::Str(test_file.to_string()),
            ]),
        ]);

        let normalized = normalize(expr, &mut interner);
        let plan_obj = plan(&normalized, &interner).unwrap();
        let result = execute(&plan_obj, &mut rt);

        assert!(result.is_ok());
        match result.unwrap() {
            Value::Frame(f) => {
                assert_eq!(f.nrows, 3);
                // First row should be NA, rest should be dlog values
            }
            _ => panic!("Expected Frame"),
        }

        std::fs::remove_file(test_file).ok();
    }

    #[test]
    fn test_exec_thread_first() {
        let test_x = "/tmp/test_exec_thread_x.csv";
        let test_y = "/tmp/test_exec_thread_y.csv";

        setup_test_csv(test_x, "DATE;price\n2020-01-01;100\n2020-01-03;103\n");
        setup_test_csv(test_y, "DATE;dummy\n2020-01-01;1\n2020-01-02;2\n2020-01-03;3\n");

        let mut interner = Interner::new();
        let mut rt = Runtime::new();

        // (-> (read-csv x) dlog (mapr (read-csv y)))
        let expr = Expr::List(vec![
            Expr::Sym(interner.intern("->")),
            Expr::List(vec![
                Expr::Sym(interner.intern("read-csv")),
                Expr::Str(test_x.to_string()),
            ]),
            Expr::Sym(interner.intern("dlog")),
            Expr::List(vec![
                Expr::Sym(interner.intern("mapr")),
                Expr::List(vec![
                    Expr::Sym(interner.intern("read-csv")),
                    Expr::Str(test_y.to_string()),
                ]),
            ]),
        ]);

        let normalized = normalize(expr, &mut interner);
        let plan_obj = plan(&normalized, &interner).unwrap();
        let result = execute(&plan_obj, &mut rt);

        assert!(result.is_ok());
        match result.unwrap() {
            Value::Frame(f) => {
                // Output should have y's nrows (3)
                assert_eq!(f.nrows, 3);
                // Output should have x's colnames (price)
                assert_eq!(f.tags.colnames[0], "price");
            }
            _ => panic!("Expected Frame"),
        }

        std::fs::remove_file(test_x).ok();
        std::fs::remove_file(test_y).ok();
    }
}
