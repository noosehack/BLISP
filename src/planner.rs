/// BLADE Phase 3: IR Planner
///
/// Purpose: Convert normalized expressions into validated IR plans
///
/// This is where contracts.md is enforced at compile time:
/// - No index coercion
/// - Join semantics validation
/// - Schema consistency checks
///
/// The planner does NOT execute - it only builds the IR DAG and validates it.

use crate::ir::{Plan, Node, NodeId, Operation, Source, UnaryOp, BinaryOp, BinaryFunc, ValueRef, JoinOp, NumericFunc, SchemaInfo, SchemaOp};
use crate::normalize::CanonExpr;
use crate::ast::{Expr, Interner, SymbolId};
use std::collections::HashMap;

/// Planning context - tracks variable bindings and node references
pub struct PlanContext {
    /// Map from variable names to their IR nodes
    bindings: HashMap<SymbolId, NodeId>,
}

impl PlanContext {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    pub fn bind(&mut self, name: SymbolId, node: NodeId) {
        self.bindings.insert(name, node);
    }

    pub fn lookup(&self, name: SymbolId) -> Option<NodeId> {
        self.bindings.get(&name).copied()
    }
}

/// Plan a normalized expression into an IR plan
///
/// This is the entry point for IR compilation. The input MUST be normalized.
pub fn plan(expr: &CanonExpr, interner: &Interner) -> Result<Plan, String> {
    let mut plan = Plan::new();
    let mut ctx = PlanContext::new();

    plan_expr(expr.inner(), &mut plan, &mut ctx, interner)?;

    // Validate the entire plan against contracts.md
    plan.validate()?;

    Ok(plan)
}

/// Recursively plan an expression
///
/// Returns the NodeId of the planned operation
fn plan_expr(
    expr: &Expr,
    plan: &mut Plan,
    ctx: &mut PlanContext,
    interner: &Interner,
) -> Result<NodeId, String> {
    match expr {
        Expr::Sym(sym) => {
            // Variable reference
            // First check if it's bound in the plan context (from let)
            if let Some(node_id) = ctx.lookup(*sym) {
                return Ok(node_id);
            }

            // Otherwise, create a Variable source (runtime lookup)
            let node_id = NodeId(plan.nodes.len());
            let node = Node {
                id: node_id,
                op: Operation::Source(Source::Variable { name: *sym }),
                schema: SchemaInfo::unknown(),
            };
            Ok(plan.add_node(node))
        }

        Expr::List(elements) if !elements.is_empty() => {
            // Function call
            if let Expr::Sym(func_sym) = &elements[0] {
                let func_name = interner.resolve(*func_sym);

                match func_name {
                    // File loading
                    "load" | "read-csv" | "file" => {
                        if elements.len() != 2 {
                            return Err(format!("{} expects 1 argument", func_name));
                        }

                        let path = match &elements[1] {
                            Expr::Str(s) => s.clone(),
                            _ => return Err(format!("{} expects string path", func_name)),
                        };

                        let node_id = NodeId(plan.nodes.len());
                        let node = Node {
                            id: node_id,
                            op: Operation::Source(Source::File { path }),
                            schema: SchemaInfo::unknown(),
                        };
                        Ok(plan.add_node(node))
                    }

                    // Read from stdin
                    "stdin" => {
                        if elements.len() != 1 {
                            return Err("stdin expects no arguments".to_string());
                        }

                        let node_id = NodeId(plan.nodes.len());
                        let node = Node {
                            id: node_id,
                            op: Operation::Source(Source::Stdin),
                            schema: SchemaInfo::unknown(),
                        };
                        Ok(plan.add_node(node))
                    }

                    // Unary numeric operations
                    "dlog" => plan_unary(NumericFunc::SHF_PTW_OBS_NLN_DLOG, &elements[1..], plan, ctx, interner),  // default: OBS (NA-skipping)
                    "dlog-ofs" => plan_unary(NumericFunc::SHF_PTW_OFS_NLN_DLOG, &elements[1..], plan, ctx, interner),  // explicit OFS (positional)

                    // DEPRECATED: Legacy alias for dlog
                    "dlog-col" => {
                        eprintln!("Warning: 'dlog-col' is deprecated, use 'dlog' instead");
                        plan_unary(NumericFunc::SHF_PTW_OBS_NLN_DLOG, &elements[1..], plan, ctx, interner)
                    }

                    "ret" => plan_unary(NumericFunc::RET, &elements[1..], plan, ctx, interner),
                    "log" => plan_unary(NumericFunc::LOG, &elements[1..], plan, ctx, interner),
                    "exp" => plan_unary(NumericFunc::EXP, &elements[1..], plan, ctx, interner),
                    "sqrt" => plan_unary(NumericFunc::SQRT, &elements[1..], plan, ctx, interner),
                    "abs" => plan_unary(NumericFunc::ABS, &elements[1..], plan, ctx, interner),
                    "inv" => plan_unary(NumericFunc::INV, &elements[1..], plan, ctx, interner),
                    "locf" => plan_unary(NumericFunc::SHF_REC_NLN_LOCF, &elements[1..], plan, ctx, interner),
                    "wkd" => plan_unary(NumericFunc::MSK_WKE, &elements[1..], plan, ctx, interner),

                    // DEPRECATED: Legacy alias for wkd
                    "w5" => {
                        eprintln!("Warning: 'w5' is deprecated, use 'wkd' instead");
                        plan_unary(NumericFunc::MSK_WKE, &elements[1..], plan, ctx, interner)
                    }

                    "cs1" => plan_unary(NumericFunc::SHF_PFX_LIN_SUM, &elements[1..], plan, ctx, interner),

                    // DEPRECATED: Legacy alias for cs1
                    "cs1-col" => {
                        eprintln!("Warning: 'cs1-col' is deprecated, use 'cs1' instead");
                        plan_unary(NumericFunc::SHF_PFX_LIN_SUM, &elements[1..], plan, ctx, interner)
                    }

                    // Shift operation: (shift k x) where k is non-negative integer
                    "shift" => {
                        if elements.len() != 3 {
                            return Err("shift expects 2 arguments: (shift k x)".to_string());
                        }

                        // Parse k as non-negative integer
                        let k = match &elements[1] {
                            Expr::Int(i) if *i >= 0 => *i as usize,
                            Expr::Int(i) => return Err(format!("shift k must be non-negative, got {}", i)),
                            Expr::Float(_) => return Err("shift k must be integer, not float".to_string()),
                            _ => return Err("shift k must be integer literal".to_string()),
                        };

                        plan_unary(NumericFunc::SHF_PTW_LIN_SHF { k }, &elements[2..], plan, ctx, interner)
                    }

                    // DEPRECATED: Legacy alias for shift
                    "shift-col" => {
                        eprintln!("Warning: 'shift-col' is deprecated, use 'shift' instead");
                        if elements.len() != 3 {
                            return Err("shift-col expects 2 arguments: (shift-col k x)".to_string());
                        }

                        // Parse k as non-negative integer
                        let k = match &elements[1] {
                            Expr::Int(i) if *i >= 0 => *i as usize,
                            Expr::Int(i) => return Err(format!("shift-col k must be non-negative, got {}", i)),
                            Expr::Float(_) => return Err("shift-col k must be integer, not float".to_string()),
                            _ => return Err("shift-col k must be integer literal".to_string()),
                        };

                        plan_unary(NumericFunc::SHF_PTW_LIN_SHF { k }, &elements[2..], plan, ctx, interner)
                    }

                    // Mask-aware shift (observation-based lag): (lag-obs k x) or (shift-obs k x)
                    // Skips masked rows when computing lag - business-day lag when weekend mask active
                    "lag-obs" | "shift-obs" => {
                        if elements.len() != 3 {
                            return Err(format!("lag-obs expects 2 arguments: (lag-obs k x)"));
                        }

                        // Parse k as non-negative integer
                        let k = match &elements[1] {
                            Expr::Int(i) if *i >= 0 => *i as usize,
                            Expr::Int(i) => return Err(format!("lag-obs k must be non-negative, got {}", i)),
                            Expr::Float(_) => return Err("lag-obs k must be integer, not float".to_string()),
                            _ => return Err("lag-obs k must be integer literal".to_string()),
                        };

                        plan_unary(NumericFunc::LAG_OBS { k }, &elements[2..], plan, ctx, interner)
                    }

                    // Keep every k-th row: (keep k x) where k is positive integer
                    "keep" => {
                        if elements.len() != 3 {
                            return Err("keep expects 2 arguments: (keep k x)".to_string());
                        }

                        // Parse k as positive integer
                        let k = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => return Err(format!("keep k must be positive, got {}", i)),
                            Expr::Float(_) => return Err("keep k must be integer, not float".to_string()),
                            _ => return Err("keep k must be integer literal".to_string()),
                        };

                        plan_unary(NumericFunc::KEEP { k }, &elements[2..], plan, ctx, interner)
                    }

                    // Rolling mean: (rolling-mean w x) where w is positive integer
                    "rolling-mean" => {
                        if elements.len() != 3 {
                            return Err("rolling-mean expects 2 arguments: (rolling-mean w x)".to_string());
                        }

                        // Parse w as positive integer
                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => return Err(format!("rolling-mean w must be positive, got {}", i)),
                            Expr::Float(_) => return Err("rolling-mean w must be integer, not float".to_string()),
                            _ => return Err("rolling-mean w must be integer literal".to_string()),
                        };

                        plan_unary(NumericFunc::SHF_WIN_LIN_AVG { w }, &elements[2..], plan, ctx, interner)
                    }

                    // Rolling mean (min 2 observations): (rolling-mean-min2 w x) where w is positive integer
                    "rolling-mean-min2" => {
                        if elements.len() != 3 {
                            return Err("rolling-mean-min2 expects 2 arguments: (rolling-mean-min2 w x)".to_string());
                        }

                        // Parse w as positive integer
                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => return Err(format!("rolling-mean-min2 w must be positive, got {}", i)),
                            Expr::Float(_) => return Err("rolling-mean-min2 w must be integer, not float".to_string()),
                            _ => return Err("rolling-mean-min2 w must be integer literal".to_string()),
                        };

                        plan_unary(NumericFunc::SHF_WIN_MIN2_LIN_AVG { w }, &elements[2..], plan, ctx, interner)
                    }

                    // Feature engineering: ft-mean as planner rewrite
                    // (ft-mean w x) → (shift 1 (rolling-mean w x))
                    // Semantics: "yesterday's distribution" (no self-reference)
                    "ft-mean" => {
                        if elements.len() != 3 {
                            return Err("ft-mean expects 2 arguments: (ft-mean w x)".to_string());
                        }

                        // Parse w as positive integer
                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => return Err(format!("ft-mean w must be positive, got {}", i)),
                            Expr::Float(_) => return Err("ft-mean w must be integer, not float".to_string()),
                            _ => return Err("ft-mean w must be integer literal".to_string()),
                        };

                        // Plan inner rolling-mean
                        let rolling_node = plan_unary(NumericFunc::SHF_WIN_LIN_AVG { w }, &elements[2..], plan, ctx, interner)?;

                        // Plan outer shift(1, ...)
                        // Create a temporary node reference for the rolling-mean result
                        let input_node = plan.get_node(rolling_node).ok_or("Invalid rolling-mean node")?;
                        let shift_node_id = NodeId(plan.nodes.len());
                        let shift_node = Node {
                            id: shift_node_id,
                            op: Operation::Unary(UnaryOp::MapNumeric {
                                input: rolling_node,
                                func: NumericFunc::SHF_PTW_LIN_SHF { k: 1 },
                            }),
                            schema: input_node.schema.clone(), // Shift preserves schema (I1-I3)
                        };
                        Ok(plan.add_node(shift_node))
                    }

                    // Rolling std: (rolling-std w x) where w is positive integer
                    "rolling-std" => {
                        if elements.len() != 3 {
                            return Err("rolling-std expects 2 arguments: (rolling-std w x)".to_string());
                        }

                        // Parse w as positive integer
                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => return Err(format!("rolling-std w must be positive, got {}", i)),
                            Expr::Float(_) => return Err("rolling-std w must be integer, not float".to_string()),
                            _ => return Err("rolling-std w must be integer literal".to_string()),
                        };

                        plan_unary(NumericFunc::SHF_WIN_NLN_SDV { w }, &elements[2..], plan, ctx, interner)
                    }


                    // Rolling std (min 2 observations): relaxed min_periods for masked calendars
                    "rolling-std-min2" => {
                        if elements.len() != 3 {
                            return Err("rolling-std-min2 expects 2 arguments: (rolling-std-min2 w x)".to_string());
                        }

                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => return Err(format!("rolling-std-min2 w must be positive, got {}", i)),
                            Expr::Float(_) => return Err("rolling-std-min2 w must be integer, not float".to_string()),
                            _ => return Err("rolling-std-min2 w must be integer literal".to_string()),
                        };

                        plan_unary(NumericFunc::SHF_WIN_MIN2_NLN_SDV { w }, &elements[2..], plan, ctx, interner)
                    }

                    // Feature engineering: ft-std as planner rewrite
                    // (ft-std w x) → (shift 1 (rolling-std w x))
                    "ft-std" => {
                        if elements.len() != 3 {
                            return Err("ft-std expects 2 arguments: (ft-std w x)".to_string());
                        }

                        // Parse w as positive integer
                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => return Err(format!("ft-std w must be positive, got {}", i)),
                            Expr::Float(_) => return Err("ft-std w must be integer, not float".to_string()),
                            _ => return Err("ft-std w must be integer literal".to_string()),
                        };

                        // Plan inner rolling-std
                        let rolling_node = plan_unary(NumericFunc::SHF_WIN_NLN_SDV { w }, &elements[2..], plan, ctx, interner)?;

                        // Plan outer shift(1, ...)
                        let input_node = plan.get_node(rolling_node).ok_or("Invalid rolling-std node")?;
                        let shift_node_id = NodeId(plan.nodes.len());
                        let shift_node = Node {
                            id: shift_node_id,
                            op: Operation::Unary(UnaryOp::MapNumeric {
                                input: rolling_node,
                                func: NumericFunc::SHF_PTW_LIN_SHF { k: 1 },
                            }),
                            schema: input_node.schema.clone(), // Shift preserves schema (I1-I3)
                        };
                        Ok(plan.add_node(shift_node))
                    }

                    // Rolling zscore: (rolling-zscore w x) → (/ (- x (rolling-mean w x)) (rolling-std w x))
                    // Derived form: no IR primitive, rewrite into existing ops
                    "rolling-zscore" | "wzs" => {
                        // wzs is CLISPI compat: (wzs w step x) ignores step param
                        let expected_args = if func_name == "wzs" { 4 } else { 3 };
                        let x_index = if func_name == "wzs" { 3 } else { 2 };

                        if elements.len() != expected_args {
                            let signature = if func_name == "wzs" {
                                "wzs expects 3 arguments: (wzs w step x)"
                            } else {
                                "rolling-zscore expects 2 arguments: (rolling-zscore w x)"
                            };
                            return Err(signature.to_string());
                        }

                        // Parse w as positive integer
                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => return Err(format!("{} w must be positive, got {}", func_name, i)),
                            Expr::Float(_) => return Err(format!("{} w must be integer, not float", func_name)),
                            _ => return Err(format!("{} w must be integer literal", func_name)),
                        };

                        // Plan input x ONCE (critical: don't re-plan stdin multiple times!)
                        let x_node = plan_expr(&elements[x_index], plan, ctx, interner)?;
                        let x_schema = plan.get_node(x_node).ok_or("Invalid x node")?.schema.clone();

                        // Create rolling-mean (min2) node using already-planned x_node
                        // Use min2 for CLISPI compatibility with masked calendars
                        let mean_node_id = NodeId(plan.nodes.len());
                        let mean_node = Node {
                            id: mean_node_id,
                            op: Operation::Unary(UnaryOp::MapNumeric {
                                input: x_node,
                                func: NumericFunc::SHF_WIN_MIN2_LIN_AVG { w },
                            }),
                            schema: x_schema.clone(), // Unary preserves schema (I1-I3)
                        };
                        let mean_node_id = plan.add_node(mean_node);

                        // Create rolling-std (min2) node using already-planned x_node
                        // Use min2 for CLISPI compatibility with masked calendars
                        let std_node_id = NodeId(plan.nodes.len());
                        let std_node = Node {
                            id: std_node_id,
                            op: Operation::Unary(UnaryOp::MapNumeric {
                                input: x_node,
                                func: NumericFunc::SHF_WIN_MIN2_NLN_SDV { w },
                            }),
                            schema: x_schema.clone(), // Unary preserves schema (I1-I3)
                        };
                        let std_node_id = plan.add_node(std_node);

                        // Plan (- x mean)
                        let sub_node_id = NodeId(plan.nodes.len());
                        let sub_node = Node {
                            id: sub_node_id,
                            op: Operation::Binary(BinaryOp::MapNumeric2 {
                                lhs: x_node,
                                rhs: ValueRef::Frame(mean_node_id),
                                func: BinaryFunc::SUB,
                            }),
                            schema: x_schema.clone(), // Binary preserves LHS schema
                        };
                        let sub_node_id = plan.add_node(sub_node);

                        // Plan (/ (- x mean) std)
                        let div_node_id = NodeId(plan.nodes.len());
                        let div_node = Node {
                            id: div_node_id,
                            op: Operation::Binary(BinaryOp::MapNumeric2 {
                                lhs: sub_node_id,
                                rhs: ValueRef::Frame(std_node_id),
                                func: BinaryFunc::DIV,
                            }),
                            schema: x_schema, // Binary preserves LHS schema
                        };
                        Ok(plan.add_node(div_node))
                    }

                    // Unit ratio (ur): Risk-adjusted returns
                    // Canonical: (ur w step x) → (/ x (* 1587.4507866 (rolling-std-min2 w x)))
                    // Uses min2 (relaxed) rolling-std to match CLISPI semantics
                    // Where: 1587.45... = 100 * sqrt(252) = percentage scale * annualization
                    // step param ignored (compatibility)
                    // Used for: normalizing log returns by rolling volatility
                    "ur" => {
                        if elements.len() != 4 {
                            return Err("ur expects 3 arguments: (ur w step x)".to_string());
                        }

                        // Parse w as positive integer
                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => return Err(format!("ur w must be positive, got {}", i)),
                            Expr::Float(_) => return Err("ur w must be integer, not float".to_string()),
                            _ => return Err("ur w must be integer literal".to_string()),
                        };

                        // step param (elements[2]) is ignored for compatibility

                        // Plan input x ONCE
                        let x_node = plan_expr(&elements[3], plan, ctx, interner)?;
                        let x_schema = plan.get_node(x_node).ok_or("Invalid x node")?.schema.clone();

                        // Create rolling-std (min2/relaxed) node for CLISPI compatibility
                        let std_node_id = NodeId(plan.nodes.len());
                        let std_node = Node {
                            id: std_node_id,
                            op: Operation::Unary(UnaryOp::MapNumeric {
                                input: x_node,
                                func: NumericFunc::SHF_WIN_MIN2_NLN_SDV { w },
                            }),
                            schema: x_schema.clone(),
                        };
                        let std_node_id = plan.add_node(std_node);

                        // Create scalar node for 1587.4507866
                        let scalar_node_id = NodeId(plan.nodes.len());
                        let scalar_node = Node {
                            id: scalar_node_id,
                            op: Operation::Binary(BinaryOp::MapNumeric2 {
                                lhs: std_node_id,
                                rhs: ValueRef::Scalar(1587.4507866),
                                func: BinaryFunc::MUL,
                            }),
                            schema: x_schema.clone(),
                        };
                        let scalar_node_id = plan.add_node(scalar_node);

                        // Create division node: x / (1587.45... * rolling-std)
                        let div_node_id = NodeId(plan.nodes.len());
                        let div_node = Node {
                            id: div_node_id,
                            op: Operation::Binary(BinaryOp::MapNumeric2 {
                                lhs: x_node,
                                rhs: ValueRef::Frame(scalar_node_id),
                                func: BinaryFunc::DIV,
                            }),
                            schema: x_schema,
                        };
                        Ok(plan.add_node(div_node))
                    }

                    // DEPRECATED: Legacy alias for ur
                    "ur-col" => {
                        eprintln!("Warning: 'ur-col' is deprecated, use 'ur' instead");
                        if elements.len() != 4 {
                            return Err("ur-col expects 3 arguments: (ur-col w step x)".to_string());
                        }

                        // Parse w as positive integer
                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => return Err(format!("ur-col w must be positive, got {}", i)),
                            Expr::Float(_) => return Err("ur-col w must be integer, not float".to_string()),
                            _ => return Err("ur-col w must be integer literal".to_string()),
                        };

                        // step param (elements[2]) is ignored for compatibility

                        // Plan input x ONCE
                        let x_node = plan_expr(&elements[3], plan, ctx, interner)?;
                        let x_schema = plan.get_node(x_node).ok_or("Invalid x node")?.schema.clone();

                        // Create rolling-std (min2/relaxed) node for CLISPI compatibility
                        let std_node_id = NodeId(plan.nodes.len());
                        let std_node = Node {
                            id: std_node_id,
                            op: Operation::Unary(UnaryOp::MapNumeric {
                                input: x_node,
                                func: NumericFunc::SHF_WIN_MIN2_NLN_SDV { w },
                            }),
                            schema: x_schema.clone(),
                        };
                        let std_node_id = plan.add_node(std_node);

                        // Create scalar node for 1587.4507866
                        let scalar_node_id = NodeId(plan.nodes.len());
                        let scalar_node = Node {
                            id: scalar_node_id,
                            op: Operation::Binary(BinaryOp::MapNumeric2 {
                                lhs: std_node_id,
                                rhs: ValueRef::Scalar(1587.4507866),
                                func: BinaryFunc::MUL,
                            }),
                            schema: x_schema.clone(),
                        };
                        let scalar_node_id = plan.add_node(scalar_node);

                        // Create division node: x / (1587.45... * rolling-std)
                        let div_node_id = NodeId(plan.nodes.len());
                        let div_node = Node {
                            id: div_node_id,
                            op: Operation::Binary(BinaryOp::MapNumeric2 {
                                lhs: x_node,
                                rhs: ValueRef::Frame(scalar_node_id),
                                func: BinaryFunc::DIV,
                            }),
                            schema: x_schema,
                        };
                        Ok(plan.add_node(div_node))
                    }

                    // Feature zscore: (ft-zscore w x) → (/ (- x (ft-mean w x)) (ft-std w x))
                    // No self-reference: compares x[i] to distribution from i-1 and earlier (no lookahead)
                    // Uses RollMeanMin2ExclCurrent / RollStdMin2ExclCurrent (window ending at i-1)
                    "ft-zscore" => {
                        if elements.len() != 3 {
                            return Err("ft-zscore expects 2 arguments: (ft-zscore w x)".to_string());
                        }

                        // Parse w as positive integer
                        let w = match &elements[1] {
                            Expr::Int(i) if *i > 0 => *i as usize,
                            Expr::Int(i) => return Err(format!("ft-zscore w must be positive, got {}", i)),
                            Expr::Float(_) => return Err("ft-zscore w must be integer, not float".to_string()),
                            _ => return Err("ft-zscore w must be integer literal".to_string()),
                        };

                        // Plan input x
                        let x_node = plan_expr(&elements[2], plan, ctx, interner)?;

                        // Plan ft-mean: rolling mean excluding current observation (window ending at i-1)
                        let ft_mean_node_id = plan_unary(NumericFunc::SHF_WIN_MIN2_LIN_AVG_EXCL { w }, &[elements[2].clone()], plan, ctx, interner)?;

                        // Plan ft-std: rolling std excluding current observation (window ending at i-1)
                        let ft_std_node_id = plan_unary(NumericFunc::SHF_WIN_MIN2_NLN_SDV_EXCL { w }, &[elements[2].clone()], plan, ctx, interner)?;

                        // Plan (- x ft-mean)
                        let sub_node_id = NodeId(plan.nodes.len());
                        let x_schema = plan.get_node(x_node).ok_or("Invalid x node")?.schema.clone();
                        let sub_node = Node {
                            id: sub_node_id,
                            op: Operation::Binary(BinaryOp::MapNumeric2 {
                                lhs: x_node,
                                rhs: ValueRef::Frame(ft_mean_node_id),
                                func: BinaryFunc::SUB,
                            }),
                            schema: x_schema.clone(),
                        };
                        let sub_node_id = plan.add_node(sub_node);

                        // Plan (/ (- x ft-mean) ft-std)
                        let div_node_id = NodeId(plan.nodes.len());
                        let div_node = Node {
                            id: div_node_id,
                            op: Operation::Binary(BinaryOp::MapNumeric2 {
                                lhs: sub_node_id,
                                rhs: ValueRef::Frame(ft_std_node_id),
                                func: BinaryFunc::DIV,
                            }),
                            schema: x_schema,
                        };
                        Ok(plan.add_node(div_node))
                    }

                    // Binary numeric operations
                    "+" => plan_binary(BinaryFunc::ADD, &elements[1..], plan, ctx, interner),
                    "-" => plan_binary(BinaryFunc::SUB, &elements[1..], plan, ctx, interner),
                    "*" => plan_binary(BinaryFunc::MUL, &elements[1..], plan, ctx, interner),
                    "/" => plan_binary(BinaryFunc::DIV, &elements[1..], plan, ctx, interner),
                    ">" => plan_binary(BinaryFunc::GTR, &elements[1..], plan, ctx, interner),

                    // Join operations
                    "mapr" => plan_join(JoinKind::MapR, &elements[1..], plan, ctx, interner),
                    "asofr" => plan_join(JoinKind::AsofR, &elements[1..], plan, ctx, interner),

                    // Schema-transforming operations
                    "xminus" => {
                        if elements.len() != 3 {
                            return Err("xminus expects 2 arguments: (xminus data half)".to_string());
                        }

                        // Parse half as boolean (0/false or 1/true)
                        let half = match &elements[2] {
                            Expr::Int(0) => false,
                            Expr::Int(1) => true,
                            Expr::Int(i) => return Err(format!("xminus half must be 0 or 1, got {}", i)),
                            _ => return Err("xminus half must be integer (0 or 1)".to_string()),
                        };

                        // Plan input
                        let input = plan_expr(&elements[1], plan, ctx, interner)?;

                        // Create xminus node
                        let node_id = NodeId(plan.nodes.len());
                        let node = Node {
                            id: node_id,
                            op: Operation::Schema(SchemaOp::SHF_PTW_LIN_SPR { input, half }),
                            schema: SchemaInfo::unknown(),  // Schema will be rebuilt at runtime
                        };
                        Ok(plan.add_node(node))
                    }

                    // Mask operations
                    "mask-weekend" => {
                        if elements.len() < 2 || elements.len() > 3 {
                            return Err("mask-weekend expects 1-2 arguments: (mask-weekend frame [name])".to_string());
                        }

                        // Parse optional name
                        let name = if elements.len() == 3 {
                            match &elements[2] {
                                Expr::Str(s) => Some(s.clone()),
                                Expr::Sym(s) => Some(interner.resolve(*s).to_string()),
                                _ => return Err("mask-weekend name must be string or symbol".to_string()),
                            }
                        } else {
                            None  // default: "weekend"
                        };

                        // Plan input
                        let input = plan_expr(&elements[1], plan, ctx, interner)?;

                        // Create mask-weekend node
                        let node_id = NodeId(plan.nodes.len());
                        let node = Node {
                            id: node_id,
                            op: Operation::Schema(SchemaOp::MSK_WKE_DEF { input, name }),
                            schema: SchemaInfo::unknown(),
                        };
                        Ok(plan.add_node(node))
                    }

                    "with-mask" => {
                        if elements.len() != 3 {
                            return Err("with-mask expects 2 arguments: (with-mask frame mask-expr)".to_string());
                        }

                        // Plan input
                        let input = plan_expr(&elements[1], plan, ctx, interner)?;

                        // Parse mask expression
                        let mask_expr = parse_mask_expr_from_ast(&elements[2], interner)?;

                        // Create with-mask node
                        let node_id = NodeId(plan.nodes.len());
                        let node = Node {
                            id: node_id,
                            op: Operation::Schema(SchemaOp::WTH_MSK { input, mask_expr }),
                            schema: SchemaInfo::unknown(),
                        };
                        Ok(plan.add_node(node))
                    }

                    // Let bindings: (let ((name1 expr1) (name2 expr2) ...) body)
                    "let" => {
                        if elements.len() != 3 {
                            return Err("let expects 2 arguments: (let ((bindings...)) body)".to_string());
                        }

                        // Parse bindings
                        let bindings_list = match &elements[1] {
                            Expr::List(bindings) => bindings,
                            _ => return Err("let expects list of bindings".to_string()),
                        };

                        // Process bindings sequentially (let* semantics)
                        for binding in bindings_list {
                            match binding {
                                Expr::List(pair) if pair.len() == 2 => {
                                    let name = match &pair[0] {
                                        Expr::Sym(s) => *s,
                                        _ => return Err("let binding expects symbol".to_string()),
                                    };

                                    let value_node = plan_expr(&pair[1], plan, ctx, interner)?;
                                    ctx.bind(name, value_node);
                                }
                                _ => return Err("let binding must be (symbol expr)".to_string()),
                            }
                        }

                        // Plan body with bindings in scope
                        plan_expr(&elements[2], plan, ctx, interner)
                    }

                    _ => Err(format!("Unknown function: {}", func_name)),
                }
            } else {
                Err("Function call must start with a symbol".to_string())
            }
        }

        _ => Err(format!("Cannot plan expression: {:?}", expr)),
    }
}

enum JoinKind {
    MapR,
    AsofR,
}

fn plan_unary(
    func: NumericFunc,
    args: &[Expr],
    plan: &mut Plan,
    ctx: &mut PlanContext,
    interner: &Interner,
) -> Result<NodeId, String> {
    if args.len() != 1 {
        return Err(format!("Unary op expects 1 argument, got {}", args.len()));
    }

    let input = plan_expr(&args[0], plan, ctx, interner)?;
    let input_schema = &plan.get_node(input).unwrap().schema;

    let node_id = NodeId(plan.nodes.len());
    let node = Node {
        id: node_id,
        op: Operation::Unary(UnaryOp::MapNumeric { input, func }),
        // Preserve schema (I1-I3)
        schema: SchemaInfo {
            index_type: input_schema.index_type,
            colnames: input_schema.colnames.clone(),
            nrows: input_schema.nrows,
        },
    };

    Ok(plan.add_node(node))
}

fn plan_join(
    kind: JoinKind,
    args: &[Expr],
    plan: &mut Plan,
    ctx: &mut PlanContext,
    interner: &Interner,
) -> Result<NodeId, String> {
    if args.len() != 2 {
        return Err(format!("Join op expects 2 arguments, got {}", args.len()));
    }

    let x = plan_expr(&args[0], plan, ctx, interner)?;
    let y = plan_expr(&args[1], plan, ctx, interner)?;

    let x_schema = &plan.get_node(x).unwrap().schema;
    let y_schema = &plan.get_node(y).unwrap().schema;

    // Check index type compatibility (if known at plan time)
    if let (Some(x_idx), Some(y_idx)) = (&x_schema.index_type, &y_schema.index_type) {
        if x_idx != y_idx {
            return Err(format!(
                "Index type mismatch in join: {:?} vs {:?} (no coercion allowed per contracts.md)",
                x_idx, y_idx
            ));
        }
    }

    let node_id = NodeId(plan.nodes.len());
    let node = Node {
        id: node_id,
        op: match kind {
            JoinKind::MapR => Operation::Join(JoinOp::ALIGN { x, y }),
            JoinKind::AsofR => Operation::Join(JoinOp::ASOF_ALIGN { x, y }),
        },
        // Join contract: y's index, x's colnames, y's nrows
        schema: SchemaInfo {
            index_type: y_schema.index_type,
            colnames: x_schema.colnames.clone(),
            nrows: y_schema.nrows,
        },
    };

    Ok(plan.add_node(node))
}

fn plan_binary(
    func: BinaryFunc,
    args: &[Expr],
    plan: &mut Plan,
    ctx: &mut PlanContext,
    interner: &Interner,
) -> Result<NodeId, String> {
    if args.len() != 2 {
        return Err(format!("Binary op expects 2 arguments, got {}", args.len()));
    }

    // LHS is always a frame expression
    let lhs = plan_expr(&args[0], plan, ctx, interner)?;

    // Clone schema before RHS planning to avoid borrow issues
    let lhs_schema = plan.get_node(lhs).unwrap().schema.clone();

    // RHS can be a scalar (number literal) or frame expression
    let rhs = match &args[1] {
        Expr::Float(f) => ValueRef::Scalar(*f),
        Expr::Int(i) => ValueRef::Scalar(*i as f64),
        _ => {
            // Frame expression
            let rhs_node = plan_expr(&args[1], plan, ctx, interner)?;
            ValueRef::Frame(rhs_node)
        }
    };

    // Output schema: LHS tags preserved (I1-I3)
    let node_id = NodeId(plan.nodes.len());
    let node = Node {
        id: node_id,
        op: Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, func }),
        schema: lhs_schema,
    };

    Ok(plan.add_node(node))
}

/// Parse a mask expression from AST
fn parse_mask_expr_from_ast(expr: &Expr, interner: &Interner) -> Result<crate::mask::MaskExpr, String> {
    use crate::mask::MaskExpr;

    match expr {
        // Symbol: 'weekend → Name("weekend")
        Expr::Sym(s) => {
            let name = interner.resolve(*s).to_string();
            Ok(MaskExpr::Name(name))
        }

        // Quote: (quote weekend) → Name("weekend")
        Expr::Quote(inner) => {
            match **inner {
                Expr::Sym(s) => {
                    let name = interner.resolve(s).to_string();
                    Ok(MaskExpr::Name(name))
                }
                _ => Err("Quoted mask name must be a symbol".to_string()),
            }
        }

        // List: (not expr) or (and ...) or (or ...)
        Expr::List(elements) if !elements.is_empty() => {
            match &elements[0] {
                Expr::Sym(op_sym) => {
                    let op_name = interner.resolve(*op_sym);

                    match op_name {
                        "not" => {
                            if elements.len() != 2 {
                                return Err("mask expr 'not' expects 1 argument".to_string());
                            }
                            let inner = parse_mask_expr_from_ast(&elements[1], interner)?;
                            Ok(MaskExpr::Not(Box::new(inner)))
                        }

                        "and" => {
                            if elements.len() < 2 {
                                return Err("mask expr 'and' expects at least 1 argument".to_string());
                            }
                            let mut sub_exprs = Vec::new();
                            for arg in &elements[1..] {
                                sub_exprs.push(parse_mask_expr_from_ast(arg, interner)?);
                            }
                            Ok(MaskExpr::And(sub_exprs))
                        }

                        "or" => {
                            if elements.len() < 2 {
                                return Err("mask expr 'or' expects at least 1 argument".to_string());
                            }
                            let mut sub_exprs = Vec::new();
                            for arg in &elements[1..] {
                                sub_exprs.push(parse_mask_expr_from_ast(arg, interner)?);
                            }
                            Ok(MaskExpr::Or(sub_exprs))
                        }

                        _ => Err(format!("Unknown mask operator: {}", op_name)),
                    }
                }
                _ => Err("Mask expression list must start with operator symbol".to_string()),
            }
        }

        _ => Err(format!("Invalid mask expression: {:?}", expr)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::normalize;

    #[test]
    fn test_plan_file_source() {
        let mut interner = Interner::new();

        // (read-csv "data.csv")
        let expr = Expr::List(vec![
            Expr::Sym(interner.intern("read-csv")),
            Expr::Str("data.csv".to_string()),
        ]);

        let normalized = normalize(expr, &mut interner);
        let plan_result = plan(&normalized, &interner);

        assert!(plan_result.is_ok());
        let plan = plan_result.unwrap();
        assert_eq!(plan.nodes.len(), 1);

        match &plan.nodes[0].op {
            Operation::Source(Source::File { path }) => {
                assert_eq!(path, "data.csv");
            }
            _ => panic!("Expected file source"),
        }
    }

    #[test]
    fn test_plan_unary_dlog() {
        let mut interner = Interner::new();

        // (dlog (read-csv "data.csv"))
        let expr = Expr::List(vec![
            Expr::Sym(interner.intern("dlog")),
            Expr::List(vec![
                Expr::Sym(interner.intern("read-csv")),
                Expr::Str("data.csv".to_string()),
            ]),
        ]);

        let normalized = normalize(expr, &mut interner);
        let plan_result = plan(&normalized, &interner);

        assert!(plan_result.is_ok());
        let plan = plan_result.unwrap();
        assert_eq!(plan.nodes.len(), 2);

        // Node 0: file source
        match &plan.nodes[0].op {
            Operation::Source(Source::File { .. }) => {}
            _ => panic!("Expected file source"),
        }

        // Node 1: dlog (should map to OBS variant)
        match &plan.nodes[1].op {
            Operation::Unary(UnaryOp::MapNumeric { input, func }) => {
                assert_eq!(*input, NodeId(0));
                assert_eq!(*func, NumericFunc::SHF_PTW_OBS_NLN_DLOG);
            }
            _ => panic!("Expected unary dlog"),
        }
    }

    #[test]
    fn test_plan_join_mapr() {
        let mut interner = Interner::new();

        // (mapr (read-csv "x.csv") (read-csv "y.csv"))
        let expr = Expr::List(vec![
            Expr::Sym(interner.intern("mapr")),
            Expr::List(vec![
                Expr::Sym(interner.intern("read-csv")),
                Expr::Str("x.csv".to_string()),
            ]),
            Expr::List(vec![
                Expr::Sym(interner.intern("read-csv")),
                Expr::Str("y.csv".to_string()),
            ]),
        ]);

        let normalized = normalize(expr, &mut interner);
        let plan_result = plan(&normalized, &interner);

        assert!(plan_result.is_ok());
        let plan = plan_result.unwrap();
        assert_eq!(plan.nodes.len(), 3);

        // Node 2: mapr join
        match &plan.nodes[2].op {
            Operation::Join(JoinOp::ALIGN { x, y }) => {
                assert_eq!(*x, NodeId(0));
                assert_eq!(*y, NodeId(1));
            }
            _ => panic!("Expected mapr join"),
        }
    }

    #[test]
    fn test_plan_thread_first() {
        let mut interner = Interner::new();

        // (-> (read-csv "data.csv") dlog (mapr (read-csv "y.csv")))
        // Should normalize to: (mapr (dlog (read-csv "data.csv")) (read-csv "y.csv"))
        let expr = Expr::List(vec![
            Expr::Sym(interner.intern("->")),
            Expr::List(vec![
                Expr::Sym(interner.intern("read-csv")),
                Expr::Str("data.csv".to_string()),
            ]),
            Expr::Sym(interner.intern("dlog")),
            Expr::List(vec![
                Expr::Sym(interner.intern("mapr")),
                Expr::List(vec![
                    Expr::Sym(interner.intern("read-csv")),
                    Expr::Str("y.csv".to_string()),
                ]),
            ]),
        ]);

        let normalized = normalize(expr, &mut interner);
        let plan_result = plan(&normalized, &interner);

        assert!(plan_result.is_ok());
        let plan = plan_result.unwrap();

        // Should have: data.csv, dlog, y.csv, mapr
        assert_eq!(plan.nodes.len(), 4);

        // Final node should be mapr
        match &plan.nodes[3].op {
            Operation::Join(JoinOp::ALIGN { .. }) => {}
            _ => panic!("Expected mapr as final op"),
        }
    }

    #[test]
    fn test_plan_let_binding() {
        let mut interner = Interner::new();

        // (let ((x (read-csv "data.csv"))) (dlog x))
        let let_expr = Expr::List(vec![
            Expr::Sym(interner.intern("let")),
            Expr::List(vec![
                Expr::List(vec![
                    Expr::Sym(interner.intern("x")),
                    Expr::List(vec![
                        Expr::Sym(interner.intern("read-csv")),
                        Expr::Str("data.csv".to_string()),
                    ]),
                ]),
            ]),
            Expr::List(vec![
                Expr::Sym(interner.intern("dlog")),
                Expr::Sym(interner.intern("x")),
            ]),
        ]);

        let normalized = normalize(let_expr, &mut interner);
        let plan_result = plan(&normalized, &interner);

        assert!(plan_result.is_ok());
        let plan_obj = plan_result.unwrap();
        assert_eq!(plan_obj.nodes.len(), 2); // file + dlog
    }
}
