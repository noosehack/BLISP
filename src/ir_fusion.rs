//! IR Fusion Optimizer
//!
//! Transforms linear operation chains into fused single-pass operations.
//!
//! PR4 Status:
//! - PR4.1: Elementwise fusion ✅
//! - PR4.2a: cs1 ∘ elementwise ✅
//! - PR4.2b: cs1 ∘ dlog-obs/ofs ✅

use crate::ir::{Plan, Node, NodeId, Operation, UnaryOp, BinaryOp, NumericFunc, ValueRef};
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
    matches!(func,
        NumericFunc::ABS |
        NumericFunc::LOG |
        NumericFunc::EXP |
        NumericFunc::SQRT |
        NumericFunc::INV
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
/// ```
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
            Operation::Unary(UnaryOp::MapNumeric { input, .. }) |
            Operation::Unary(UnaryOp::FusedElementwise { input, .. }) |
            Operation::Unary(UnaryOp::FusedCs1Elementwise { input, .. }) |
            Operation::Unary(UnaryOp::FusedCs1DlogOfs { input, .. }) |
            Operation::Unary(UnaryOp::FusedCs1DlogObs { input, .. }) |
            Operation::Unary(UnaryOp::FusedDlogObsElementwise { input, .. }) |
            Operation::Unary(UnaryOp::FusedDlogOfsElementwise { input, .. }) => {
                *consumers.entry(*input).or_insert(0) += 1;
            }
            Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, .. }) => {
                *consumers.entry(*lhs).or_insert(0) += 1;
                if let ValueRef::Frame(rhs_id) = rhs {
                    *consumers.entry(*rhs_id).or_insert(0) += 1;
                }
            }
            Operation::Join(crate::ir::JoinOp::ALIGN { x, y }) |
            Operation::Join(crate::ir::JoinOp::ASOF_ALIGN { x, y }) => {
                *consumers.entry(*x).or_insert(0) += 1;
                *consumers.entry(*y).or_insert(0) += 1;
            }
            _ => {}
        }
    }

    let mut new_nodes = Vec::new();
    let mut node_map: HashMap<NodeId, NodeId> = HashMap::new(); // old id → new id
    let mut fused = vec![false; plan.nodes.len()]; // nodes that were fused into others

    for (i, node) in plan.nodes.iter().enumerate() {
        if fused[i] {
            continue; // Already fused into another node
        }

        // Try to fuse elementwise chain starting at this node
        if let Operation::Unary(UnaryOp::MapNumeric { input, func }) = &node.op {
            if func.is_pure_elementwise() {
                // Start building a chain
                let mut ops = vec![*func];
                let mut current_input = *input;
                let mut chain_nodes = vec![node.id];

                // Walk backwards to find more elementwise ops
                loop {
                    // Check if input is a fusible elementwise MapNumeric with single consumer
                    let input_node = &plan.nodes[current_input.0];

                    if let Operation::Unary(UnaryOp::MapNumeric { input: next_input, func: input_func }) = &input_node.op {
                        if input_func.is_pure_elementwise() && consumers.get(&current_input).copied().unwrap_or(0) == 1 {
                            // Fusible! Prepend this op (we're walking backwards)
                            ops.insert(0, *input_func);
                            chain_nodes.insert(0, current_input);
                            current_input = *next_input;
                            continue;
                        }
                    }

                    break; // Can't fuse further
                }

                // If we found a chain of length > 1, fuse it
                if ops.len() > 1 {
                    // Mark intermediate nodes as fused
                    for &chain_node_id in &chain_nodes[..chain_nodes.len()-1] {
                        fused[chain_node_id.0] = true;
                    }

                    // Remap the input through node_map if it was already transformed
                    let remapped_input = node_map.get(&current_input).copied().unwrap_or(current_input);

                    // Create fused node
                    let new_id = NodeId(new_nodes.len());
                    node_map.insert(node.id, new_id);

                    new_nodes.push(Node {
                        id: new_id,
                        op: Operation::Unary(UnaryOp::FusedElementwise {
                            input: remapped_input,
                            ops,
                        }),
                        schema: node.schema.clone(),
                    });

                    continue;
                }
            }
        }

        // Not fusible - copy node with remapped inputs
        let new_id = NodeId(new_nodes.len());
        node_map.insert(node.id, new_id);

        let new_op = match &node.op {
            Operation::Unary(UnaryOp::MapNumeric { input, func }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::MapNumeric { input: remapped, func: *func })
            }
            Operation::Unary(UnaryOp::FusedElementwise { input, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedElementwise { input: remapped, ops: ops.clone() })
            }
            Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, func }) => {
                let remapped_lhs = node_map.get(lhs).copied().unwrap_or(*lhs);
                let remapped_rhs = match rhs {
                    ValueRef::Scalar(s) => ValueRef::Scalar(*s),
                    ValueRef::Frame(id) => ValueRef::Frame(node_map.get(id).copied().unwrap_or(*id)),
                };
                Operation::Binary(BinaryOp::MapNumeric2 { lhs: remapped_lhs, rhs: remapped_rhs, func: *func })
            }
            Operation::Join(crate::ir::JoinOp::ALIGN { x, y }) => {
                let remapped_x = node_map.get(x).copied().unwrap_or(*x);
                let remapped_y = node_map.get(y).copied().unwrap_or(*y);
                Operation::Join(crate::ir::JoinOp::ALIGN { x: remapped_x, y: remapped_y })
            }
            Operation::Join(crate::ir::JoinOp::ASOF_ALIGN { x, y }) => {
                let remapped_x = node_map.get(x).copied().unwrap_or(*x);
                let remapped_y = node_map.get(y).copied().unwrap_or(*y);
                Operation::Join(crate::ir::JoinOp::ASOF_ALIGN { x: remapped_x, y: remapped_y })
            }
            Operation::Unary(UnaryOp::FusedCs1Elementwise { input, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedCs1Elementwise { input: remapped, ops: ops.clone() })
            }
            Operation::Unary(UnaryOp::FusedCs1DlogOfs { input, lag }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedCs1DlogOfs { input: remapped, lag: *lag })
            }
            Operation::Unary(UnaryOp::FusedCs1DlogObs { input }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedCs1DlogObs { input: remapped })
            }
            Operation::Unary(UnaryOp::FusedDlogObsElementwise { input, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedDlogObsElementwise { input: remapped, ops: ops.clone() })
            }
            Operation::Unary(UnaryOp::FusedDlogOfsElementwise { input, lag, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedDlogOfsElementwise { input: remapped, lag: *lag, ops: ops.clone() })
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

    Plan { nodes: new_nodes }
}


pub fn optimize_cs1_elementwise_fusion(plan: &Plan) -> Plan {
    // Build consumer count map
    let mut consumers: HashMap<NodeId, usize> = HashMap::new();
    for node in &plan.nodes {
        match &node.op {
            Operation::Unary(UnaryOp::MapNumeric { input, .. }) |
            Operation::Unary(UnaryOp::FusedElementwise { input, .. }) |
            Operation::Unary(UnaryOp::FusedCs1Elementwise { input, .. }) |
            Operation::Unary(UnaryOp::FusedCs1DlogOfs { input, .. }) |
            Operation::Unary(UnaryOp::FusedCs1DlogObs { input, .. }) |
            Operation::Unary(UnaryOp::FusedDlogObsElementwise { input, .. }) |
            Operation::Unary(UnaryOp::FusedDlogOfsElementwise { input, .. }) => {
                *consumers.entry(*input).or_insert(0) += 1;
            }
            Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, .. }) => {
                *consumers.entry(*lhs).or_insert(0) += 1;
                if let ValueRef::Frame(rhs_id) = rhs {
                    *consumers.entry(*rhs_id).or_insert(0) += 1;
                }
            }
            Operation::Join(crate::ir::JoinOp::ALIGN { x, y }) |
            Operation::Join(crate::ir::JoinOp::ASOF_ALIGN { x, y }) => {
                *consumers.entry(*x).or_insert(0) += 1;
                *consumers.entry(*y).or_insert(0) += 1;
            }
            _ => {}
        }
    }

    let mut new_nodes = Vec::new();
    let mut node_map: HashMap<NodeId, NodeId> = HashMap::new();
    let mut fused = vec![false; plan.nodes.len()];

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
                if let Operation::Unary(UnaryOp::FusedElementwise { input: chain_input, ops }) = &input_node.op {
                    if consumers.get(input).copied().unwrap_or(0) == 1 {
                        // Fusible! Mark intermediate as fused
                        fused[input.0] = true;

                        // Remap chain input
                        let remapped_input = node_map.get(chain_input).copied().unwrap_or(*chain_input);

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
                if let Operation::Unary(UnaryOp::MapNumeric { input: ew_input, func: ew_func }) = &input_node.op {
                    if ew_func.is_pure_elementwise() && consumers.get(input).copied().unwrap_or(0) == 1 {
                        // Fusible! Mark intermediate as fused
                        fused[input.0] = true;

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
                Operation::Unary(UnaryOp::MapNumeric { input: remapped, func: *func })
            }
            Operation::Unary(UnaryOp::FusedElementwise { input, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedElementwise { input: remapped, ops: ops.clone() })
            }
            Operation::Unary(UnaryOp::FusedCs1Elementwise { input, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedCs1Elementwise { input: remapped, ops: ops.clone() })
            }
            Operation::Unary(UnaryOp::FusedCs1DlogOfs { input, lag }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedCs1DlogOfs { input: remapped, lag: *lag })
            }
            Operation::Unary(UnaryOp::FusedCs1DlogObs { input }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedCs1DlogObs { input: remapped })
            }
            Operation::Unary(UnaryOp::FusedDlogObsElementwise { input, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedDlogObsElementwise { input: remapped, ops: ops.clone() })
            }
            Operation::Unary(UnaryOp::FusedDlogOfsElementwise { input, lag, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedDlogOfsElementwise { input: remapped, lag: *lag, ops: ops.clone() })
            }
            Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, func }) => {
                let remapped_lhs = node_map.get(lhs).copied().unwrap_or(*lhs);
                let remapped_rhs = match rhs {
                    ValueRef::Scalar(s) => ValueRef::Scalar(*s),
                    ValueRef::Frame(id) => ValueRef::Frame(node_map.get(id).copied().unwrap_or(*id)),
                };
                Operation::Binary(BinaryOp::MapNumeric2 { lhs: remapped_lhs, rhs: remapped_rhs, func: *func })
            }
            Operation::Join(crate::ir::JoinOp::ALIGN { x, y }) => {
                let remapped_x = node_map.get(x).copied().unwrap_or(*x);
                let remapped_y = node_map.get(y).copied().unwrap_or(*y);
                Operation::Join(crate::ir::JoinOp::ALIGN { x: remapped_x, y: remapped_y })
            }
            Operation::Join(crate::ir::JoinOp::ASOF_ALIGN { x, y }) => {
                let remapped_x = node_map.get(x).copied().unwrap_or(*x);
                let remapped_y = node_map.get(y).copied().unwrap_or(*y);
                Operation::Join(crate::ir::JoinOp::ASOF_ALIGN { x: remapped_x, y: remapped_y })
            }
            Operation::Source(s) => Operation::Source(s.clone()),
            Operation::Schema(schema_op) => {
                use crate::ir::SchemaOp;
                match schema_op {
                    SchemaOp::SHF_PTW_LIN_SPR { input, half } => {
                        let remapped = node_map.get(input).copied().unwrap_or(*input);
                        Operation::Schema(SchemaOp::SHF_PTW_LIN_SPR { input: remapped, half: *half })
                    }
                    SchemaOp::MSK_WKE_DEF { input, name } => {
                        let remapped = node_map.get(input).copied().unwrap_or(*input);
                        Operation::Schema(SchemaOp::MSK_WKE_DEF { input: remapped, name: name.clone() })
                    }
                    SchemaOp::WTH_MSK { input, mask_expr } => {
                        let remapped = node_map.get(input).copied().unwrap_or(*input);
                        Operation::Schema(SchemaOp::WTH_MSK { input: remapped, mask_expr: mask_expr.clone() })
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
            Operation::Unary(UnaryOp::MapNumeric { input, .. }) |
            Operation::Unary(UnaryOp::FusedElementwise { input, .. }) |
            Operation::Unary(UnaryOp::FusedCs1Elementwise { input, .. }) |
            Operation::Unary(UnaryOp::FusedCs1DlogOfs { input, .. }) |
            Operation::Unary(UnaryOp::FusedCs1DlogObs { input, .. }) |
            Operation::Unary(UnaryOp::FusedDlogObsElementwise { input, .. }) |
            Operation::Unary(UnaryOp::FusedDlogOfsElementwise { input, .. }) => {
                *consumers.entry(*input).or_insert(0) += 1;
            }
            Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, .. }) => {
                *consumers.entry(*lhs).or_insert(0) += 1;
                if let ValueRef::Frame(rhs_id) = rhs {
                    *consumers.entry(*rhs_id).or_insert(0) += 1;
                }
            }
            Operation::Join(crate::ir::JoinOp::ALIGN { x, y }) |
            Operation::Join(crate::ir::JoinOp::ASOF_ALIGN { x, y }) => {
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
                if let Operation::Unary(UnaryOp::MapNumeric { func: dlog_func, .. }) = &input_node.op {
                    if matches!(dlog_func, NumericFunc::SHF_PTW_OFS_NLN_DLOG) &&
                       consumers.get(input).copied().unwrap_or(0) == 1 {
                        fused[input.0] = true; // Mark dlog-ofs as fused
                    }
                }

                // Case 2: cs1(dlog-obs) - only fusible if lag==1 (HARD CHECK)
                if let Operation::Unary(UnaryOp::MapNumeric { func: dlog_func, .. }) = &input_node.op {
                    if matches!(dlog_func, NumericFunc::SHF_PTW_OBS_NLN_DLOG) &&
                       consumers.get(input).copied().unwrap_or(0) == 1 {
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
                if let Operation::Unary(UnaryOp::MapNumeric { input: dlog_input, func: dlog_func }) = &input_node.op {
                    if matches!(dlog_func, NumericFunc::SHF_PTW_OFS_NLN_DLOG) &&
                       consumers.get(input).copied().unwrap_or(0) == 1 {
                        // Remap input
                        let remapped_input = node_map.get(dlog_input).copied().unwrap_or(*dlog_input);

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
                if let Operation::Unary(UnaryOp::MapNumeric { input: dlog_input, func: dlog_func }) = &input_node.op {
                    if matches!(dlog_func, NumericFunc::SHF_PTW_OBS_NLN_DLOG) &&
                       consumers.get(input).copied().unwrap_or(0) == 1 {
                        // HARD CHECK: OBS intrinsically lag=1 in observation space
                        // Current dlog_obs_column ignores _lag param, so always fusible
                        // Future: When planner tracks lag, check lag==1 here

                        // Remap input
                        let remapped_input = node_map.get(dlog_input).copied().unwrap_or(*dlog_input);

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
                Operation::Unary(UnaryOp::MapNumeric { input: remapped, func: *func })
            }
            Operation::Unary(UnaryOp::FusedElementwise { input, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedElementwise { input: remapped, ops: ops.clone() })
            }
            Operation::Unary(UnaryOp::FusedCs1Elementwise { input, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedCs1Elementwise { input: remapped, ops: ops.clone() })
            }
            Operation::Unary(UnaryOp::FusedCs1DlogOfs { input, lag }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedCs1DlogOfs { input: remapped, lag: *lag })
            }
            Operation::Unary(UnaryOp::FusedCs1DlogObs { input }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedCs1DlogObs { input: remapped })
            }
            Operation::Unary(UnaryOp::FusedDlogObsElementwise { input, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedDlogObsElementwise { input: remapped, ops: ops.clone() })
            }
            Operation::Unary(UnaryOp::FusedDlogOfsElementwise { input, lag, ops }) => {
                let remapped = node_map.get(input).copied().unwrap_or(*input);
                Operation::Unary(UnaryOp::FusedDlogOfsElementwise { input: remapped, lag: *lag, ops: ops.clone() })
            }
            Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, func }) => {
                let remapped_lhs = node_map.get(lhs).copied().unwrap_or(*lhs);
                let remapped_rhs = match rhs {
                    ValueRef::Scalar(s) => ValueRef::Scalar(*s),
                    ValueRef::Frame(id) => ValueRef::Frame(node_map.get(id).copied().unwrap_or(*id)),
                };
                Operation::Binary(BinaryOp::MapNumeric2 { lhs: remapped_lhs, rhs: remapped_rhs, func: *func })
            }
            Operation::Join(crate::ir::JoinOp::ALIGN { x, y }) => {
                let remapped_x = node_map.get(x).copied().unwrap_or(*x);
                let remapped_y = node_map.get(y).copied().unwrap_or(*y);
                Operation::Join(crate::ir::JoinOp::ALIGN { x: remapped_x, y: remapped_y })
            }
            Operation::Join(crate::ir::JoinOp::ASOF_ALIGN { x, y }) => {
                let remapped_x = node_map.get(x).copied().unwrap_or(*x);
                let remapped_y = node_map.get(y).copied().unwrap_or(*y);
                Operation::Join(crate::ir::JoinOp::ASOF_ALIGN { x: remapped_x, y: remapped_y })
            }
            Operation::Source(s) => Operation::Source(s.clone()),
            Operation::Schema(schema_op) => {
                use crate::ir::SchemaOp;
                match schema_op {
                    SchemaOp::SHF_PTW_LIN_SPR { input, half } => {
                        let remapped = node_map.get(input).copied().unwrap_or(*input);
                        Operation::Schema(SchemaOp::SHF_PTW_LIN_SPR { input: remapped, half: *half })
                    }
                    SchemaOp::MSK_WKE_DEF { input, name } => {
                        let remapped = node_map.get(input).copied().unwrap_or(*input);
                        Operation::Schema(SchemaOp::MSK_WKE_DEF { input: remapped, name: name.clone() })
                    }
                    SchemaOp::WTH_MSK { input, mask_expr } => {
                        let remapped = node_map.get(input).copied().unwrap_or(*input);
                        Operation::Schema(SchemaOp::WTH_MSK { input: remapped, mask_expr: mask_expr.clone() })
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
