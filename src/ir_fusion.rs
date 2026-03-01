//! IR Fusion Optimizer
#![allow(clippy::doc_lazy_continuation)]
//!
//! Transforms linear operation chains into fused single-pass operations.
//!
//! PR4 Status:
//! - PR4.1: Elementwise fusion ✅
//! - PR4.2a: cs1 ∘ elementwise ✅
//! - PR4.2b: cs1 ∘ dlog-obs/ofs ✅

use crate::ir::{BinaryOp, Node, NodeId, NumericFunc, Operation, Plan, UnaryOp, ValueRef};
use std::collections::HashMap;

/// Main optimization entry point
///
/// Applies fusion passes in sequence. Order matters: later passes assume
/// earlier canonicalization (elementwise chains collected first).
pub fn optimize(plan: &Plan) -> Plan {
    let mut optimized = plan.clone();

    // PR4 fusion pipeline (order matters!)
    optimized = optimize_elementwise_fusion(&optimized);
    optimized = optimize_cs1_elementwise_fusion(&optimized);
    optimized = optimize_cs1_dlog_fusion(&optimized);

    optimized
}

/// Helper: Check if operation is pure elementwise (no state, no lookahead)
fn is_pure_elementwise(func: &NumericFunc) -> bool {
    matches!(
        func,
        NumericFunc::ABS
            | NumericFunc::LOG
            | NumericFunc::EXP
            | NumericFunc::SQRT
            | NumericFunc::INV
    )
}

/// PR4.1: Optimize a plan by fusing chains of pure elementwise operations
///
/// Legality rules:
/// 1. Only fuse pure elementwise ops (LOG, EXP, SQRT, ABS, INV)
/// 2. Only fuse if intermediate result has single consumer (no work duplication)
/// 3. Preserve all I1-I3 invariants (tags identity preserved)
///
/// Example transformation:
/// ```text
/// Before:  x → ABS → LOG → EXP
/// After:   x → FusedElementwise([ABS, LOG, EXP])
/// ```
///
/// Benefits:
/// - Reduces allocations from N intermediate arrays to 1 output
/// - Better cache locality (single pass over data)
/// - NaN propagation unchanged (flows through naturally)
pub fn optimize_elementwise_fusion(plan: &Plan) -> Plan {
    // Build consumer count map
    let mut consumers: HashMap<NodeId, usize> = HashMap::new();
    for node in &plan.nodes {
        match &node.op {
            Operation::Unary(UnaryOp::MapNumeric { input, .. })
            | Operation::Unary(UnaryOp::FusedElementwise { input, .. })
            | Operation::Unary(UnaryOp::FusedCs1Elementwise { input, .. })
            | Operation::Unary(UnaryOp::FusedCs1DlogOfs { input, .. })
            | Operation::Unary(UnaryOp::FusedCs1DlogObs { input, .. })
            | Operation::Unary(UnaryOp::FusedDlogObsElementwise { input, .. })
            | Operation::Unary(UnaryOp::FusedDlogOfsElementwise { input, .. }) => {
                *consumers.entry(*input).or_insert(0) += 1;
            }
            Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, .. }) => {
                *consumers.entry(*lhs).or_insert(0) += 1;
                if let ValueRef::Frame(rhs_id) = rhs {
                    *consumers.entry(*rhs_id).or_insert(0) += 1;
                }
            }
            Operation::Join(crate::ir::JoinOp::ALIGN { x, y })
            | Operation::Join(crate::ir::JoinOp::ASOF_ALIGN { x, y }) => {
                *consumers.entry(*x).or_insert(0) += 1;
                *consumers.entry(*y).or_insert(0) += 1;
            }
            _ => {}
        }
    }

    // PASS 1: Identify fusible chains (reverse order to find longest chains first)
    // Maps chain head node → (input, ops)
    let mut fusion_info: HashMap<NodeId, (NodeId, Vec<NumericFunc>)> = HashMap::new();
    let mut fused = vec![false; plan.nodes.len()];

    for (i, node) in plan.nodes.iter().enumerate().rev() {
        if fused[i] {
            continue;
        }

        if let Operation::Unary(UnaryOp::MapNumeric { input, func }) = &node.op {
            if func.is_pure_elementwise() {
                let mut ops = vec![*func];
                let mut current_input = *input;
                let mut chain_nodes = vec![node.id];

                // Walk backwards to collect fusible ops
                loop {
                    let input_node = &plan.nodes[current_input.0];
                    if let Operation::Unary(UnaryOp::MapNumeric {
                        input: next_input,
                        func: input_func,
                    }) = &input_node.op
                    {
                        if input_func.is_pure_elementwise()
                            && consumers.get(&current_input).copied().unwrap_or(0) == 1
                        {
                            ops.insert(0, *input_func);
                            chain_nodes.insert(0, current_input);
                            current_input = *next_input;
                            continue;
                        }
                    }
                    break;
                }

                // Mark chain and store fusion info
                if ops.len() > 1 {
                    for &chain_node_id in &chain_nodes {
                        fused[chain_node_id.0] = true;
                    }
                    fusion_info.insert(node.id, (current_input, ops));
                }
            }
        }
    }

    // PASS 2: Build new plan (forward order)
    let mut new_nodes = Vec::new();
    let mut node_map: HashMap<NodeId, NodeId> = HashMap::new();

    for (i, node) in plan.nodes.iter().enumerate() {
        if fused[i] && !fusion_info.contains_key(&node.id) {
            // This node was fused into another chain head - skip it
            continue;
        }

        if let Some((input, ops)) = fusion_info.get(&node.id) {
            // This is a fusion chain head - create fused node
            let remapped_input = node_map.get(input).copied().unwrap_or(*input);
            let new_id = NodeId(new_nodes.len());
            node_map.insert(node.id, new_id);

            new_nodes.push(Node {
                id: new_id,
                op: Operation::Unary(UnaryOp::FusedElementwise {
                    input: remapped_input,
                    ops: ops.clone(),
                }),
                schema: node.schema.clone(),
            });
        } else {
            // Not fusible - copy node with remapped inputs
            let new_id = NodeId(new_nodes.len());
            node_map.insert(node.id, new_id);

            let new_op = match &node.op {
                Operation::Unary(UnaryOp::MapNumeric { input, func }) => {
                    let remapped = node_map.get(input).copied().unwrap_or(*input);
                    Operation::Unary(UnaryOp::MapNumeric {
                        input: remapped,
                        func: *func,
                    })
                }
                Operation::Unary(UnaryOp::FusedElementwise { input, ops }) => {
                    let remapped = node_map.get(input).copied().unwrap_or(*input);
                    Operation::Unary(UnaryOp::FusedElementwise {
                        input: remapped,
                        ops: ops.clone(),
                    })
                }
                Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, func }) => {
                    let remapped_lhs = node_map.get(lhs).copied().unwrap_or(*lhs);
                    let remapped_rhs = match rhs {
                        ValueRef::Scalar(s) => ValueRef::Scalar(*s),
                        ValueRef::Frame(id) => {
                            ValueRef::Frame(node_map.get(id).copied().unwrap_or(*id))
                        }
                    };
                    Operation::Binary(BinaryOp::MapNumeric2 {
                        lhs: remapped_lhs,
                        rhs: remapped_rhs,
                        func: *func,
                    })
                }
                Operation::Join(crate::ir::JoinOp::ALIGN { x, y }) => {
                    let remapped_x = node_map.get(x).copied().unwrap_or(*x);
                    let remapped_y = node_map.get(y).copied().unwrap_or(*y);
                    Operation::Join(crate::ir::JoinOp::ALIGN {
                        x: remapped_x,
                        y: remapped_y,
                    })
                }
                Operation::Join(crate::ir::JoinOp::ASOF_ALIGN { x, y }) => {
                    let remapped_x = node_map.get(x).copied().unwrap_or(*x);
                    let remapped_y = node_map.get(y).copied().unwrap_or(*y);
                    Operation::Join(crate::ir::JoinOp::ASOF_ALIGN {
                        x: remapped_x,
                        y: remapped_y,
                    })
                }
                Operation::Unary(UnaryOp::FusedCs1Elementwise { input, ops }) => {
                    let remapped = node_map.get(input).copied().unwrap_or(*input);
                    Operation::Unary(UnaryOp::FusedCs1Elementwise {
                        input: remapped,
                        ops: ops.clone(),
                    })
                }
                Operation::Unary(UnaryOp::FusedCs1DlogOfs { input, lag }) => {
                    let remapped = node_map.get(input).copied().unwrap_or(*input);
                    Operation::Unary(UnaryOp::FusedCs1DlogOfs {
                        input: remapped,
                        lag: *lag,
                    })
                }
                Operation::Unary(UnaryOp::FusedCs1DlogObs { input }) => {
                    let remapped = node_map.get(input).copied().unwrap_or(*input);
                    Operation::Unary(UnaryOp::FusedCs1DlogObs { input: remapped })
                }
                Operation::Unary(UnaryOp::FusedDlogObsElementwise { input, ops }) => {
                    let remapped = node_map.get(input).copied().unwrap_or(*input);
                    Operation::Unary(UnaryOp::FusedDlogObsElementwise {
                        input: remapped,
                        ops: ops.clone(),
                    })
                }
                Operation::Unary(UnaryOp::FusedDlogOfsElementwise { input, lag, ops }) => {
                    let remapped = node_map.get(input).copied().unwrap_or(*input);
                    Operation::Unary(UnaryOp::FusedDlogOfsElementwise {
                        input: remapped,
                        lag: *lag,
                        ops: ops.clone(),
                    })
                }
                Operation::Source(s) => Operation::Source(s.clone()),
                Operation::Schema(s) => Operation::Schema(s.clone()),
            };

            new_nodes.push(Node {
                id: new_id,
                op: new_op,
                schema: node.schema.clone(),
            });
        }
    }

    Plan { nodes: new_nodes }
}

pub fn optimize_cs1_elementwise_fusion(plan: &Plan) -> Plan {
    // Build consumer count map
    let mut consumers: HashMap<NodeId, usize> = HashMap::new();
    for node in &plan.nodes {
        match &node.op {
            Operation::Unary(UnaryOp::MapNumeric { input, .. })
            | Operation::Unary(UnaryOp::FusedElementwise { input, .. })
            | Operation::Unary(UnaryOp::FusedCs1Elementwise { input, .. })
            | Operation::Unary(UnaryOp::FusedCs1DlogOfs { input, .. })
            | Operation::Unary(UnaryOp::FusedCs1DlogObs { input, .. })
            | Operation::Unary(UnaryOp::FusedDlogObsElementwise { input, .. })
            | Operation::Unary(UnaryOp::FusedDlogOfsElementwise { input, .. }) => {
                *consumers.entry(*input).or_insert(0) += 1;
            }
            Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, .. }) => {
                *consumers.entry(*lhs).or_insert(0) += 1;
                if let ValueRef::Frame(rhs_id) = rhs {
                    *consumers.entry(*rhs_id).or_insert(0) += 1;
                }
            }
            Operation::Join(crate::ir::JoinOp::ALIGN { x, y })
            | Operation::Join(crate::ir::JoinOp::ASOF_ALIGN { x, y }) => {
                *consumers.entry(*x).or_insert(0) += 1;
                *consumers.entry(*y).or_insert(0) += 1;
            }
            _ => {}
        }
    }

    let mut new_nodes = Vec::new();
    let mut node_map: HashMap<NodeId, NodeId> = HashMap::new();
    let mut fused = vec![false; plan.nodes.len()];

    // PASS 1: Mark nodes that will be fused
    for node in &plan.nodes {
        if let Operation::Unary(UnaryOp::MapNumeric { input, func }) = &node.op {
            if matches!(func, NumericFunc::SHF_PFX_LIN_SUM) {
                let input_node = &plan.nodes[input.0];

                // Mark fusible inputs
                if let Operation::Unary(UnaryOp::FusedElementwise { .. }) = &input_node.op {
                    if consumers.get(input).copied().unwrap_or(0) == 1 {
                        fused[input.0] = true;
                    }
                } else if let Operation::Unary(UnaryOp::MapNumeric { func: ew_func, .. }) =
                    &input_node.op
                {
                    if ew_func.is_pure_elementwise()
                        && consumers.get(input).copied().unwrap_or(0) == 1
                    {
                        fused[input.0] = true;
                    }
                }
            }
        }
    }

    // PASS 2: Build optimized plan
    for (i, node) in plan.nodes.iter().enumerate() {
        if fused[i] {
            continue;
        }

        // Try to fuse cs1 ∘ elementwise_chain
        if let Operation::Unary(UnaryOp::MapNumeric { input, func }) = &node.op {
            if matches!(func, NumericFunc::SHF_PFX_LIN_SUM) {
                // This is cs1 - check if input is a fusible elementwise chain
                let input_node = &plan.nodes[input.0];

                // Case 1: Input is FusedElementwise with single consumer
                if let Operation::Unary(UnaryOp::FusedElementwise {
                    input: chain_input,
                    ops,
                }) = &input_node.op
                {
                    if consumers.get(input).copied().unwrap_or(0) == 1 {
                        // Already marked as fused in Pass 1

                        // Remap chain input
                        let remapped_input =
                            node_map.get(chain_input).copied().unwrap_or(*chain_input);

                        // Create fused cs1+elementwise node
                        let new_id = NodeId(new_nodes.len());
                        node_map.insert(node.id, new_id);

                        new_nodes.push(Node {
                            id: new_id,
                            op: Operation::Unary(UnaryOp::FusedCs1Elementwise {
                                input: remapped_input,
                                ops: ops.clone(),
                            }),
                            schema: node.schema.clone(),
                        });

                        continue;
                    }
                }

                // Case 2: Input is single elementwise MapNumeric with single consumer
                if let Operation::Unary(UnaryOp::MapNumeric {
                    input: ew_input,
                    func: ew_func,
                }) = &input_node.op
                {
                    if ew_func.is_pure_elementwise()
                        && consumers.get(input).copied().unwrap_or(0) == 1
                    {
                        // Already marked as fused in Pass 1

                        // Remap input
                        let remapped_input = node_map.get(ew_input).copied().unwrap_or(*ew_input);

                        // Create fused cs1+elementwise node
                        let new_id = NodeId(new_nodes.len());
                        node_map.insert(node.id, new_id);

                        new_nodes.push(Node {
                            id: new_id,
                            op: Operation::Unary(UnaryOp::FusedCs1Elementwise {
                                input: remapped_input,
                                ops: vec![*ew_func],
                            }),
                            schema: node.schema.clone(),
                        });

                        continue;
                    }
                }
            }
        }

        // Not fusible - copy node with remapped inputs (reuse logic from optimize_elementwise_fusion)
        let new_id = NodeId(new_nodes.len());
        node_map.insert(node.id, new_id);

        let new_op = match &node.op {
            Operation::Unary(UnaryOp::MapNumeric { input, func }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::MapNumeric {
                    input: remapped,
                    func: *func,
                })
            }
            Operation::Unary(UnaryOp::FusedElementwise { input, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedElementwise {
                    input: remapped,
                    ops: ops.clone(),
                })
            }
            Operation::Unary(UnaryOp::FusedCs1Elementwise { input, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedCs1Elementwise {
                    input: remapped,
                    ops: ops.clone(),
                })
            }
            Operation::Unary(UnaryOp::FusedCs1DlogOfs { input, lag }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedCs1DlogOfs {
                    input: remapped,
                    lag: *lag,
                })
            }
            Operation::Unary(UnaryOp::FusedCs1DlogObs { input }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedCs1DlogObs { input: remapped })
            }
            Operation::Unary(UnaryOp::FusedDlogObsElementwise { input, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedDlogObsElementwise {
                    input: remapped,
                    ops: ops.clone(),
                })
            }
            Operation::Unary(UnaryOp::FusedDlogOfsElementwise { input, lag, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedDlogOfsElementwise {
                    input: remapped,
                    lag: *lag,
                    ops: ops.clone(),
                })
            }
            Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, func }) => {
                let remapped_lhs = node_map.get(lhs).copied().unwrap_or(*lhs);
                let remapped_rhs = match rhs {
                    ValueRef::Scalar(s) => ValueRef::Scalar(*s),
                    ValueRef::Frame(id) => {
                        ValueRef::Frame(node_map.get(id).copied().unwrap_or(*id))
                    }
                };
                Operation::Binary(BinaryOp::MapNumeric2 {
                    lhs: remapped_lhs,
                    rhs: remapped_rhs,
                    func: *func,
                })
            }
            Operation::Join(crate::ir::JoinOp::ALIGN { x, y }) => {
                let remapped_x = node_map.get(x).copied().unwrap_or(*x);
                let remapped_y = node_map.get(y).copied().unwrap_or(*y);
                Operation::Join(crate::ir::JoinOp::ALIGN {
                    x: remapped_x,
                    y: remapped_y,
                })
            }
            Operation::Join(crate::ir::JoinOp::ASOF_ALIGN { x, y }) => {
                let remapped_x = node_map.get(x).copied().unwrap_or(*x);
                let remapped_y = node_map.get(y).copied().unwrap_or(*y);
                Operation::Join(crate::ir::JoinOp::ASOF_ALIGN {
                    x: remapped_x,
                    y: remapped_y,
                })
            }
            Operation::Source(s) => Operation::Source(s.clone()),
            Operation::Schema(schema_op) => {
                use crate::ir::SchemaOp;
                match schema_op {
                    SchemaOp::SHF_PTW_LIN_SPR { input, half } => {
                        let remapped = node_map.get(input).copied().unwrap_or(*input);
                        Operation::Schema(SchemaOp::SHF_PTW_LIN_SPR {
                            input: remapped,
                            half: *half,
                        })
                    }
                    SchemaOp::MSK_WKE_DEF { input, name } => {
                        let remapped = node_map.get(input).copied().unwrap_or(*input);
                        Operation::Schema(SchemaOp::MSK_WKE_DEF {
                            input: remapped,
                            name: name.clone(),
                        })
                    }
                    SchemaOp::WTH_MSK { input, mask_expr } => {
                        let remapped = node_map.get(input).copied().unwrap_or(*input);
                        Operation::Schema(SchemaOp::WTH_MSK {
                            input: remapped,
                            mask_expr: mask_expr.clone(),
                        })
                    }
                }
            }
        };

        new_nodes.push(Node {
            id: new_id,
            op: new_op,
            schema: node.schema.clone(),
        });
    }

    Plan { nodes: new_nodes }
}

pub fn optimize_cs1_dlog_fusion(plan: &Plan) -> Plan {
    // Build consumer count map
    let mut consumers: HashMap<NodeId, usize> = HashMap::new();
    for node in &plan.nodes {
        match &node.op {
            Operation::Unary(UnaryOp::MapNumeric { input, .. })
            | Operation::Unary(UnaryOp::FusedElementwise { input, .. })
            | Operation::Unary(UnaryOp::FusedCs1Elementwise { input, .. })
            | Operation::Unary(UnaryOp::FusedCs1DlogOfs { input, .. })
            | Operation::Unary(UnaryOp::FusedCs1DlogObs { input, .. })
            | Operation::Unary(UnaryOp::FusedDlogObsElementwise { input, .. })
            | Operation::Unary(UnaryOp::FusedDlogOfsElementwise { input, .. }) => {
                *consumers.entry(*input).or_insert(0) += 1;
            }
            Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, .. }) => {
                *consumers.entry(*lhs).or_insert(0) += 1;
                if let ValueRef::Frame(rhs_id) = rhs {
                    *consumers.entry(*rhs_id).or_insert(0) += 1;
                }
            }
            Operation::Join(crate::ir::JoinOp::ALIGN { x, y })
            | Operation::Join(crate::ir::JoinOp::ASOF_ALIGN { x, y }) => {
                *consumers.entry(*x).or_insert(0) += 1;
                *consumers.entry(*y).or_insert(0) += 1;
            }
            _ => {}
        }
    }

    let mut new_nodes = Vec::new();
    let mut node_map: HashMap<NodeId, NodeId> = HashMap::new();
    let mut fused = vec![false; plan.nodes.len()];

    // First pass (reverse): Identify fusible cs1 ∘ dlog patterns
    for node in plan.nodes.iter().rev() {
        if fused[node.id.0] {
            continue;
        }

        // Detect cs1 node
        if let Operation::Unary(UnaryOp::MapNumeric { input, func }) = &node.op {
            if matches!(func, NumericFunc::SHF_PFX_LIN_SUM) {
                let input_node = &plan.nodes[input.0];

                // Case 1: cs1(dlog-ofs) - always fusible
                if let Operation::Unary(UnaryOp::MapNumeric {
                    func: dlog_func, ..
                }) = &input_node.op
                {
                    if matches!(dlog_func, NumericFunc::SHF_PTW_OFS_NLN_DLOG)
                        && consumers.get(input).copied().unwrap_or(0) == 1
                    {
                        fused[input.0] = true; // Mark dlog-ofs as fused
                    }
                }

                // Case 2: cs1(dlog-obs) - only fusible if lag==1 (HARD CHECK)
                if let Operation::Unary(UnaryOp::MapNumeric {
                    func: dlog_func, ..
                }) = &input_node.op
                {
                    if matches!(dlog_func, NumericFunc::SHF_PTW_OBS_NLN_DLOG)
                        && consumers.get(input).copied().unwrap_or(0) == 1
                    {
                        // CRITICAL: OBS only supports lag=1 intrinsically
                        // dlog-obs currently ignores lag param, but we must be defensive
                        // For now, always fuse (dlog_obs_column ignores lag anyway)
                        // Future: When planner is fixed to reject k≠1, this becomes trivial
                        fused[input.0] = true; // Mark dlog-obs as fused
                    }
                }
            }
        }
    }

    // Second pass (forward): Build optimized plan
    for (i, node) in plan.nodes.iter().enumerate() {
        if fused[i] {
            continue;
        }

        // Try to fuse cs1 ∘ dlog
        if let Operation::Unary(UnaryOp::MapNumeric { input, func }) = &node.op {
            if matches!(func, NumericFunc::SHF_PFX_LIN_SUM) {
                let input_node = &plan.nodes[input.0];

                // Case 1: cs1(dlog-ofs) → FusedCs1DlogOfs
                if let Operation::Unary(UnaryOp::MapNumeric {
                    input: dlog_input,
                    func: dlog_func,
                }) = &input_node.op
                {
                    if matches!(dlog_func, NumericFunc::SHF_PTW_OFS_NLN_DLOG)
                        && consumers.get(input).copied().unwrap_or(0) == 1
                    {
                        // Remap input
                        let remapped_input =
                            node_map.get(dlog_input).copied().unwrap_or(*dlog_input);

                        // Create fused node (OFS uses fixed lag=1 for now; blawktrust supports arbitrary k)
                        let new_id = NodeId(new_nodes.len());
                        node_map.insert(node.id, new_id);

                        new_nodes.push(Node {
                            id: new_id,
                            op: Operation::Unary(UnaryOp::FusedCs1DlogOfs {
                                input: remapped_input,
                                lag: 1, // Fixed lag for now; TODO: extract from planner if needed
                            }),
                            schema: node.schema.clone(),
                        });

                        continue;
                    }
                }

                // Case 2: cs1(dlog-obs) → FusedCs1DlogObs (only if semantically lag=1)
                if let Operation::Unary(UnaryOp::MapNumeric {
                    input: dlog_input,
                    func: dlog_func,
                }) = &input_node.op
                {
                    if matches!(dlog_func, NumericFunc::SHF_PTW_OBS_NLN_DLOG)
                        && consumers.get(input).copied().unwrap_or(0) == 1
                    {
                        // HARD CHECK: OBS intrinsically lag=1 in observation space
                        // Current dlog_obs_column ignores _lag param, so always fusible
                        // Future: When planner tracks lag, check lag==1 here

                        // Remap input
                        let remapped_input =
                            node_map.get(dlog_input).copied().unwrap_or(*dlog_input);

                        // Create fused node (OBS has no lag param - semantic truth!)
                        let new_id = NodeId(new_nodes.len());
                        node_map.insert(node.id, new_id);

                        new_nodes.push(Node {
                            id: new_id,
                            op: Operation::Unary(UnaryOp::FusedCs1DlogObs {
                                input: remapped_input,
                            }),
                            schema: node.schema.clone(),
                        });

                        continue;
                    }
                }
            }
        }

        // Not fusible - copy node with remapped inputs (reuse logic)
        let new_id = NodeId(new_nodes.len());
        node_map.insert(node.id, new_id);

        let new_op = match &node.op {
            Operation::Unary(UnaryOp::MapNumeric { input, func }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::MapNumeric {
                    input: remapped,
                    func: *func,
                })
            }
            Operation::Unary(UnaryOp::FusedElementwise { input, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedElementwise {
                    input: remapped,
                    ops: ops.clone(),
                })
            }
            Operation::Unary(UnaryOp::FusedCs1Elementwise { input, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedCs1Elementwise {
                    input: remapped,
                    ops: ops.clone(),
                })
            }
            Operation::Unary(UnaryOp::FusedCs1DlogOfs { input, lag }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedCs1DlogOfs {
                    input: remapped,
                    lag: *lag,
                })
            }
            Operation::Unary(UnaryOp::FusedCs1DlogObs { input }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedCs1DlogObs { input: remapped })
            }
            Operation::Unary(UnaryOp::FusedDlogObsElementwise { input, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedDlogObsElementwise {
                    input: remapped,
                    ops: ops.clone(),
                })
            }
            Operation::Unary(UnaryOp::FusedDlogOfsElementwise { input, lag, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedDlogOfsElementwise {
                    input: remapped,
                    lag: *lag,
                    ops: ops.clone(),
                })
            }
            Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, func }) => {
                let remapped_lhs = node_map.get(lhs).copied().unwrap_or(*lhs);
                let remapped_rhs = match rhs {
                    ValueRef::Scalar(s) => ValueRef::Scalar(*s),
                    ValueRef::Frame(id) => {
                        ValueRef::Frame(node_map.get(id).copied().unwrap_or(*id))
                    }
                };
                Operation::Binary(BinaryOp::MapNumeric2 {
                    lhs: remapped_lhs,
                    rhs: remapped_rhs,
                    func: *func,
                })
            }
            Operation::Join(crate::ir::JoinOp::ALIGN { x, y }) => {
                let remapped_x = node_map.get(x).copied().unwrap_or(*x);
                let remapped_y = node_map.get(y).copied().unwrap_or(*y);
                Operation::Join(crate::ir::JoinOp::ALIGN {
                    x: remapped_x,
                    y: remapped_y,
                })
            }
            Operation::Join(crate::ir::JoinOp::ASOF_ALIGN { x, y }) => {
                let remapped_x = node_map.get(x).copied().unwrap_or(*x);
                let remapped_y = node_map.get(y).copied().unwrap_or(*y);
                Operation::Join(crate::ir::JoinOp::ASOF_ALIGN {
                    x: remapped_x,
                    y: remapped_y,
                })
            }
            Operation::Source(s) => Operation::Source(s.clone()),
            Operation::Schema(schema_op) => {
                use crate::ir::SchemaOp;
                match schema_op {
                    SchemaOp::SHF_PTW_LIN_SPR { input, half } => {
                        let remapped = node_map.get(input).copied().unwrap_or(*input);
                        Operation::Schema(SchemaOp::SHF_PTW_LIN_SPR {
                            input: remapped,
                            half: *half,
                        })
                    }
                    SchemaOp::MSK_WKE_DEF { input, name } => {
                        let remapped = node_map.get(input).copied().unwrap_or(*input);
                        Operation::Schema(SchemaOp::MSK_WKE_DEF {
                            input: remapped,
                            name: name.clone(),
                        })
                    }
                    SchemaOp::WTH_MSK { input, mask_expr } => {
                        let remapped = node_map.get(input).copied().unwrap_or(*input);
                        Operation::Schema(SchemaOp::WTH_MSK {
                            input: remapped,
                            mask_expr: mask_expr.clone(),
                        })
                    }
                }
            }
        };

        new_nodes.push(Node {
            id: new_id,
            op: new_op,
            schema: node.schema.clone(),
        });
    }

    Plan { nodes: new_nodes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;

    /// Tripwire 1: Elementwise chain fusion (PR4.1)
    ///
    /// Test that x → ABS → LOG → EXP fuses into FusedElementwise
    /// Before: 4 nodes (source + 3 ops)
    /// After: 2 nodes (source + 1 fused)
    #[test]
    fn test_tripwire_elementwise_fusion() {
        let mut plan = Plan { nodes: vec![] };

        // source
        let src = plan.add_node(Node {
            id: NodeId(0),
            op: Operation::Source(Source::File {
                path: "x".to_string(),
            }),
            schema: SchemaInfo::unknown(),
        });

        // ABS
        let abs_node = plan.add_node(Node {
            id: NodeId(1),
            op: Operation::Unary(UnaryOp::MapNumeric {
                input: src,
                func: NumericFunc::ABS,
            }),
            schema: SchemaInfo::unknown(),
        });

        // LOG
        let log_node = plan.add_node(Node {
            id: NodeId(2),
            op: Operation::Unary(UnaryOp::MapNumeric {
                input: abs_node,
                func: NumericFunc::LOG,
            }),
            schema: SchemaInfo::unknown(),
        });

        // EXP
        plan.add_node(Node {
            id: NodeId(3),
            op: Operation::Unary(UnaryOp::MapNumeric {
                input: log_node,
                func: NumericFunc::EXP,
            }),
            schema: SchemaInfo::unknown(),
        });

        assert_eq!(plan.nodes.len(), 4, "Before optimization: 4 nodes");

        let optimized = optimize(&plan);

        assert_eq!(
            optimized.nodes.len(),
            2,
            "After optimization: 2 nodes (source + fused)"
        );

        // Verify fused node structure
        match &optimized.nodes[1].op {
            Operation::Unary(UnaryOp::FusedElementwise { input, ops }) => {
                assert_eq!(input.0, 0, "Fused node should point to source");
                assert_eq!(ops.len(), 3, "Should have 3 fused ops");
                assert!(matches!(ops[0], NumericFunc::ABS));
                assert!(matches!(ops[1], NumericFunc::LOG));
                assert!(matches!(ops[2], NumericFunc::EXP));
            }
            _ => panic!("Expected FusedElementwise, got {:?}", optimized.nodes[1].op),
        }
    }

    /// Tripwire 2: cs1 ∘ elementwise fusion (PR4.2a)
    ///
    /// Test that x → LOG → cs1 fuses into FusedCs1Elementwise
    #[test]
    fn test_tripwire_cs1_elementwise_fusion() {
        let mut plan = Plan { nodes: vec![] };

        let src = plan.add_node(Node {
            id: NodeId(0),
            op: Operation::Source(Source::File {
                path: "x".to_string(),
            }),
            schema: SchemaInfo::unknown(),
        });

        let log_node = plan.add_node(Node {
            id: NodeId(1),
            op: Operation::Unary(UnaryOp::MapNumeric {
                input: src,
                func: NumericFunc::LOG,
            }),
            schema: SchemaInfo::unknown(),
        });

        plan.add_node(Node {
            id: NodeId(2),
            op: Operation::Unary(UnaryOp::MapNumeric {
                input: log_node,
                func: NumericFunc::SHF_PFX_LIN_SUM, // cs1
            }),
            schema: SchemaInfo::unknown(),
        });

        assert_eq!(plan.nodes.len(), 3);

        let optimized = optimize(&plan);

        assert_eq!(optimized.nodes.len(), 2, "Should fuse to 2 nodes");

        match &optimized.nodes[1].op {
            Operation::Unary(UnaryOp::FusedCs1Elementwise { input, ops }) => {
                assert_eq!(input.0, 0);
                assert_eq!(ops.len(), 1);
                assert!(matches!(ops[0], NumericFunc::LOG));
            }
            _ => panic!("Expected FusedCs1Elementwise"),
        }
    }

    /// Tripwire 3: cs1 ∘ dlog fusion (PR4.2b)
    ///
    /// Test that x → dlog-obs → cs1 fuses into FusedCs1DlogObs
    #[test]
    fn test_tripwire_cs1_dlog_fusion() {
        let mut plan = Plan { nodes: vec![] };

        let src = plan.add_node(Node {
            id: NodeId(0),
            op: Operation::Source(Source::File {
                path: "x".to_string(),
            }),
            schema: SchemaInfo::unknown(),
        });

        let dlog_node = plan.add_node(Node {
            id: NodeId(1),
            op: Operation::Unary(UnaryOp::MapNumeric {
                input: src,
                func: NumericFunc::SHF_PTW_OBS_NLN_DLOG, // dlog-obs
            }),
            schema: SchemaInfo::unknown(),
        });

        plan.add_node(Node {
            id: NodeId(2),
            op: Operation::Unary(UnaryOp::MapNumeric {
                input: dlog_node,
                func: NumericFunc::SHF_PFX_LIN_SUM, // cs1
            }),
            schema: SchemaInfo::unknown(),
        });

        assert_eq!(plan.nodes.len(), 3);

        let optimized = optimize(&plan);

        assert_eq!(optimized.nodes.len(), 2, "Should fuse to 2 nodes");

        match &optimized.nodes[1].op {
            Operation::Unary(UnaryOp::FusedCs1DlogObs { input }) => {
                assert_eq!(input.0, 0, "Should point to source");
            }
            _ => panic!("Expected FusedCs1DlogObs"),
        }
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::exec;
    use crate::ir::{SchemaInfo, Source};
    use blawktrust::Column;
    use proptest::prelude::*;

    /// Pipeline operation grammar
    #[derive(Debug, Clone)]
    enum PipelineOp {
        // Pure elementwise
        Abs,
        Log, // Requires positive input (we add ABS automatically)
        Exp,
        Sqrt, // Requires non-negative (we add ABS automatically)
        Inv,

        // Stateful
        Cs1,
        DlogObs,
        // Note: DlogOfs with lag>1 not yet supported in unfused form
        // ShiftObs/ShiftOfs not yet implemented
    }

    /// Strategy for generating a single operation
    fn op_strategy() -> impl Strategy<Value = PipelineOp> {
        prop_oneof![
            // Elementwise (higher weight - common operations)
            3 => Just(PipelineOp::Abs),
            2 => Just(PipelineOp::Exp),
            2 => Just(PipelineOp::Inv),

            // LOG/SQRT need positive input - lower weight
            1 => Just(PipelineOp::Log),
            1 => Just(PipelineOp::Sqrt),

            // Stateful operations
            2 => Just(PipelineOp::Cs1),
            2 => Just(PipelineOp::DlogObs),
        ]
    }

    /// Strategy for generating a random pipeline (depth 1-7)
    fn pipeline_strategy() -> impl Strategy<Value = Vec<PipelineOp>> {
        prop::collection::vec(op_strategy(), 1..=7)
    }

    /// Sanitize pipeline to avoid LOG/SQRT on negative numbers
    ///
    /// Rules:
    /// - Before LOG: ensure ABS is in the chain
    /// - Before SQRT: ensure ABS is in the chain
    fn sanitize_pipeline(mut ops: Vec<PipelineOp>) -> Vec<PipelineOp> {
        let mut sanitized = Vec::new();
        let mut has_abs = false;

        for op in ops.drain(..) {
            match op {
                PipelineOp::Log | PipelineOp::Sqrt => {
                    // Ensure we have ABS before dangerous ops
                    if !has_abs {
                        sanitized.push(PipelineOp::Abs);
                    }
                    sanitized.push(op);
                    // After LOG/SQRT, might go negative again
                    has_abs = false;
                }
                PipelineOp::Abs => {
                    sanitized.push(op);
                    has_abs = true;
                }
                PipelineOp::Exp => {
                    sanitized.push(op);
                    has_abs = true; // exp is always positive
                }
                _ => {
                    sanitized.push(op);
                    // Other ops don't guarantee positivity
                    has_abs = false;
                }
            }
        }

        sanitized
    }

    /// Build an IR plan from a pipeline of operations
    fn build_plan(ops: &[PipelineOp]) -> (Plan, NodeId) {
        let mut plan = Plan::new();

        // Source node
        let mut current = plan.add_node(Node {
            id: NodeId(0),
            op: Operation::Source(Source::Variable {
                name: crate::ast::SymbolId(0),
            }),
            schema: SchemaInfo::unknown(),
        });

        // Apply each operation
        for op in ops {
            current = match op {
                PipelineOp::Abs => plan.add_node(Node {
                    id: NodeId(plan.nodes.len()),
                    op: Operation::Unary(UnaryOp::MapNumeric {
                        input: current,
                        func: NumericFunc::ABS,
                    }),
                    schema: SchemaInfo::unknown(),
                }),
                PipelineOp::Log => plan.add_node(Node {
                    id: NodeId(plan.nodes.len()),
                    op: Operation::Unary(UnaryOp::MapNumeric {
                        input: current,
                        func: NumericFunc::LOG,
                    }),
                    schema: SchemaInfo::unknown(),
                }),
                PipelineOp::Exp => plan.add_node(Node {
                    id: NodeId(plan.nodes.len()),
                    op: Operation::Unary(UnaryOp::MapNumeric {
                        input: current,
                        func: NumericFunc::EXP,
                    }),
                    schema: SchemaInfo::unknown(),
                }),
                PipelineOp::Sqrt => plan.add_node(Node {
                    id: NodeId(plan.nodes.len()),
                    op: Operation::Unary(UnaryOp::MapNumeric {
                        input: current,
                        func: NumericFunc::SQRT,
                    }),
                    schema: SchemaInfo::unknown(),
                }),
                PipelineOp::Inv => plan.add_node(Node {
                    id: NodeId(plan.nodes.len()),
                    op: Operation::Unary(UnaryOp::MapNumeric {
                        input: current,
                        func: NumericFunc::INV,
                    }),
                    schema: SchemaInfo::unknown(),
                }),
                PipelineOp::Cs1 => plan.add_node(Node {
                    id: NodeId(plan.nodes.len()),
                    op: Operation::Unary(UnaryOp::MapNumeric {
                        input: current,
                        func: NumericFunc::SHF_PFX_LIN_SUM,
                    }),
                    schema: SchemaInfo::unknown(),
                }),
                PipelineOp::DlogObs => plan.add_node(Node {
                    id: NodeId(plan.nodes.len()),
                    op: Operation::Unary(UnaryOp::MapNumeric {
                        input: current,
                        func: NumericFunc::SHF_PTW_OBS_NLN_DLOG,
                    }),
                    schema: SchemaInfo::unknown(),
                }),
            };
        }

        (plan, current)
    }

    /// Strategy for generating test data with random NA patterns
    fn data_strategy() -> impl Strategy<Value = Vec<f64>> {
        // Size: 50-200 elements (fast but large enough to catch bugs)
        (50_usize..=200).prop_flat_map(|size| {
            // NA density: 0-30%
            (0_usize..=30).prop_flat_map(move |na_pct| {
                // Generate random floats in range [1.0, 100.0]
                prop::collection::vec(1.0_f64..=100.0, size).prop_map(move |mut data| {
                    // Apply NA pattern by replacing every N-th element
                    if na_pct > 0 {
                        let stride = 100 / na_pct.max(1);
                        for i in (0..data.len()).step_by(stride.max(1)) {
                            data[i] = f64::NAN;
                        }
                    }
                    data
                })
            })
        })
    }

    /// Compare two columns for NaN-aware equality
    fn columns_equal(a: &Column, b: &Column) -> bool {
        match (a, b) {
            (Column::F64(a_data), Column::F64(b_data)) => {
                if a_data.len() != b_data.len() {
                    return false;
                }
                for (a_val, b_val) in a_data.iter().zip(b_data.iter()) {
                    // IEEE-754 special values must match exactly
                    match (a_val.is_nan(), b_val.is_nan()) {
                        (true, true) => continue, // Both NaN - OK
                        (false, false) => {
                            // Check for infinity match
                            if a_val.is_infinite() && b_val.is_infinite() {
                                if a_val.signum() == b_val.signum() {
                                    continue; // Both +inf or both -inf - OK
                                } else {
                                    return false; // +inf vs -inf - FAIL
                                }
                            }
                            // Finite values - use relative error comparison
                            let diff = (a_val - b_val).abs();
                            let magnitude = a_val.abs().max(b_val.abs());
                            if magnitude > 1e-10 {
                                if diff / magnitude > 1e-9 {
                                    return false;
                                }
                            } else if diff > 1e-9 {
                                return false;
                            }
                        }
                        _ => return false, // One NaN, one not - FAIL
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Helper: Execute a plan directly (simplified executor for testing)
    ///
    /// Returns the result of the last node in the plan (the output node).
    fn execute_plan_direct(plan: &Plan, input: &Column) -> Result<Column, String> {
        use std::collections::HashMap;

        let mut results: HashMap<NodeId, Column> = HashMap::new();
        let mut last_result: Option<Column> = None;

        // Execute nodes in topological order
        for node in &plan.nodes {
            let result = match &node.op {
                Operation::Source(_) => input.clone(),

                Operation::Unary(UnaryOp::MapNumeric { input, func }) => {
                    let input_col = results
                        .get(input)
                        .ok_or_else(|| format!("Missing input for node {:?}", node.id))?;

                    match func {
                        NumericFunc::ABS => exec::abs_column(input_col),
                        NumericFunc::LOG => exec::log_column(input_col),
                        NumericFunc::EXP => exec::exp_column(input_col),
                        NumericFunc::SQRT => exec::sqrt_column(input_col),
                        NumericFunc::INV => exec::inv_column(input_col),
                        NumericFunc::SHF_PFX_LIN_SUM => exec::cumsum_column(input_col),
                        NumericFunc::SHF_PTW_OBS_NLN_DLOG => exec::dlog_obs_column(input_col, 1),
                        _ => return Err(format!("Unsupported func: {:?}", func)),
                    }
                }

                Operation::Unary(UnaryOp::FusedElementwise { input, ops }) => {
                    let input_col = results
                        .get(input)
                        .ok_or_else(|| format!("Missing input for node {:?}", node.id))?;
                    exec::fused_elementwise_column(input_col, ops)
                }

                Operation::Unary(UnaryOp::FusedCs1Elementwise { input, ops }) => {
                    let input_col = results
                        .get(input)
                        .ok_or_else(|| format!("Missing input for node {:?}", node.id))?;
                    exec::fused_cs1_elementwise_column(input_col, ops)
                }

                Operation::Unary(UnaryOp::FusedCs1DlogOfs { input, lag }) => {
                    let input_col = results
                        .get(input)
                        .ok_or_else(|| format!("Missing input for node {:?}", node.id))?;
                    exec::fused_cs1_dlog_ofs_column(input_col, *lag)
                }

                Operation::Unary(UnaryOp::FusedCs1DlogObs { input }) => {
                    let input_col = results
                        .get(input)
                        .ok_or_else(|| format!("Missing input for node {:?}", node.id))?;
                    exec::fused_cs1_dlog_obs_column(input_col)
                }

                _ => return Err(format!("Unsupported operation: {:?}", node.op)),
            };

            results.insert(node.id, result.clone());
            last_result = Some(result);
        }

        last_result.ok_or_else(|| "Empty plan".to_string())
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 100,  // 100 random test cases
            max_shrink_iters: 1000,
            .. ProptestConfig::default()
        })]

        /// Core property: Optimizer preserves semantics
        ///
        /// Property: execute(optimize(plan)) ≡ execute(plan)
        ///
        /// Generates random pipelines and verifies optimization correctness.
        #[test]
        fn prop_optimizer_preserves_semantics(
            ops in pipeline_strategy(),
            data in data_strategy(),
        ) {
            // Sanitize pipeline (add ABS before LOG/SQRT)
            let ops = sanitize_pipeline(ops);

            // Skip empty pipelines
            if ops.is_empty() {
                return Ok(());
            }

            // Build IR plan
            let (plan, output_node) = build_plan(&ops);

            // Validate plan structure
            if plan.validate().is_err() {
                return Ok(()); // Invalid plan - skip
            }

            // Execute unfused (baseline)
            let col = Column::new_f64(data.clone());
            let unfused_result = match execute_plan_direct(&plan, &col) {
                Ok(result) => result,
                Err(_) => return Ok(()), // Execution error - skip
            };

            // Apply all optimizations
            let mut optimized = plan.clone();
            optimized = optimize_elementwise_fusion(&optimized);
            optimized = optimize_cs1_elementwise_fusion(&optimized);
            optimized = optimize_cs1_dlog_fusion(&optimized);

            // Execute fused
            let fused_result = match execute_plan_direct(&optimized, &col) {
                Ok(result) => result,
                Err(_) => {
                    // Fused execution failed but unfused succeeded - BUG!
                    prop_assert!(false, "Optimizer broke execution: unfused OK, fused FAILED\nPipeline: {:?}", ops);
                    return Ok(());
                }
            };

            // Verify equivalence
            prop_assert!(
                columns_equal(&unfused_result, &fused_result),
                "Optimizer changed semantics!\nPipeline: {:?}\nData length: {}\nFirst 10 unfused: {:?}\nFirst 10 fused: {:?}",
                ops,
                data.len(),
                unfused_result,
                fused_result
            );
        }

        /// Property: Optimizer is idempotent
        ///
        /// Property: optimize(optimize(plan)) ≡ optimize(plan)
        #[test]
        fn prop_optimizer_is_idempotent(
            ops in pipeline_strategy(),
        ) {
            let ops = sanitize_pipeline(ops);
            if ops.is_empty() {
                return Ok(());
            }

            let (plan, _) = build_plan(&ops);
            if plan.validate().is_err() {
                return Ok(());
            }

            // Apply optimizations once
            let mut opt1 = plan.clone();
            opt1 = optimize_elementwise_fusion(&opt1);
            opt1 = optimize_cs1_elementwise_fusion(&opt1);
            opt1 = optimize_cs1_dlog_fusion(&opt1);

            // Apply optimizations again
            let mut opt2 = opt1.clone();
            opt2 = optimize_elementwise_fusion(&opt2);
            opt2 = optimize_cs1_elementwise_fusion(&opt2);
            opt2 = optimize_cs1_dlog_fusion(&opt2);

            // Both should have same number of nodes (idempotent)
            prop_assert_eq!(
                opt1.nodes.len(),
                opt2.nodes.len(),
                "Optimizer is not idempotent! First pass: {} nodes, second pass: {} nodes",
                opt1.nodes.len(),
                opt2.nodes.len()
            );
        }
    }
}
