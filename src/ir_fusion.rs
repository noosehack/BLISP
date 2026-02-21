//! IR Fusion Framework - Safe operation fusion with correctness guarantees
//!
//! Purpose: Identify and fuse pipeline segments to reduce overhead
//!
//! Fusion Rules (conservative, correctness-preserving):
//! 1. **Unary chain fusion**: Multiple MapNumeric ops on same input
//!    - Fuse: (log (sqrt (abs x))) → FusedUnary([abs, sqrt, log], x)
//!    - Safety: All preserve tags (I1-I3), composition is valid
//!
//! 2. **Binary-scalar chain fusion**: Multiple scalar binary ops
//!    - Fuse: (+ (* x 2.0) 5.0) → FusedScalarBinary([(Mul, 2.0), (Add, 5.0)], x)
//!    - Safety: Scalar broadcast preserves shape, composition is valid
//!
//! 3. **NOT FUSED** (for now):
//!    - Join operations (complex alignment semantics)
//!    - Binary frame-frame operations (compatibility checks)
//!    - Rolling operations (already O(n), minimal fusion gain)
//!
//! Correctness guarantee: Differential testing (execute before/after, assert equivalence)

use crate::ir::{Plan, Node, NodeId, Operation, UnaryOp, BinaryOp, NumericFunc, BinaryFunc, ValueRef};
use crate::frame::{Frame, map_numeric_preserve_tags};
use blawktrust::Column;
use std::collections::HashMap;
use std::sync::Arc;

/// Fused operation types (extended IR)
#[derive(Debug, Clone)]
pub enum FusedOperation {
    /// Fused unary chain: apply multiple numeric functions in sequence
    FusedUnary {
        input: NodeId,
        funcs: Vec<NumericFunc>,  // Applied in order: funcs[0], then funcs[1], etc.
    },
    /// Fused binary-scalar chain: apply multiple scalar operations in sequence
    FusedScalarBinary {
        input: NodeId,
        ops: Vec<(BinaryFunc, f64)>,  // Applied in order: (func, scalar_rhs)
    },
}

/// Pipeline segment - a maximal fusible sequence
#[derive(Debug, Clone)]
pub struct Segment {
    /// Nodes in this segment (in topological order)
    pub nodes: Vec<NodeId>,
    /// Segment type
    pub kind: SegmentKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentKind {
    /// Chain of unary MapNumeric operations
    UnaryChain,
    /// Chain of binary scalar operations
    ScalarBinaryChain,
    /// Single unfusible operation
    Atomic,
}

/// Identify fusible segments in a plan
pub fn identify_segments(plan: &Plan) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut visited = vec![false; plan.nodes.len()];

    // Build dependency graph (who consumes each node)
    let mut consumers: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for node in &plan.nodes {
        match &node.op {
            Operation::Unary(UnaryOp::MapNumeric { input, .. }) => {
                consumers.entry(*input).or_default().push(node.id);
            }
            Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, .. }) => {
                consumers.entry(*lhs).or_default().push(node.id);
                if let ValueRef::Frame(rhs_id) = rhs {
                    consumers.entry(*rhs_id).or_default().push(node.id);
                }
            }
            Operation::Join(join_op) => {
                // Handle join inputs
                use crate::ir::JoinOp;
                match join_op {
                    JoinOp::MapR { x, y } | JoinOp::AsofR { x, y } => {
                        consumers.entry(*x).or_default().push(node.id);
                        consumers.entry(*y).or_default().push(node.id);
                    }
                }
            }
            _ => {}
        }
    }

    // Traverse in topological order, identifying segments
    for node in &plan.nodes {
        if visited[node.id.0] {
            continue;
        }

        match &node.op {
            Operation::Unary(UnaryOp::MapNumeric { input, func }) => {
                // Check if this starts a unary chain
                if is_fusible_unary(*func) {
                    let segment = extract_unary_chain(plan, node.id, &consumers, &mut visited);
                    if segment.nodes.len() > 1 {
                        segments.push(segment);
                    } else {
                        visited[node.id.0] = true;
                        segments.push(Segment {
                            nodes: vec![node.id],
                            kind: SegmentKind::Atomic,
                        });
                    }
                } else {
                    visited[node.id.0] = true;
                    segments.push(Segment {
                        nodes: vec![node.id],
                        kind: SegmentKind::Atomic,
                    });
                }
            }
            Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, func }) => {
                // Check if this starts a scalar binary chain
                if let ValueRef::Scalar(_) = rhs {
                    let segment = extract_scalar_binary_chain(plan, node.id, &consumers, &mut visited);
                    if segment.nodes.len() > 1 {
                        segments.push(segment);
                    } else {
                        visited[node.id.0] = true;
                        segments.push(Segment {
                            nodes: vec![node.id],
                            kind: SegmentKind::Atomic,
                        });
                    }
                } else {
                    visited[node.id.0] = true;
                    segments.push(Segment {
                        nodes: vec![node.id],
                        kind: SegmentKind::Atomic,
                    });
                }
            }
            _ => {
                visited[node.id.0] = true;
                segments.push(Segment {
                    nodes: vec![node.id],
                    kind: SegmentKind::Atomic,
                });
            }
        }
    }

    segments
}

/// Check if a unary function is fusible
fn is_fusible_unary(func: NumericFunc) -> bool {
    match func {
        // Simple pointwise functions - fusible
        NumericFunc::Log | NumericFunc::Exp | NumericFunc::Sqrt |
        NumericFunc::Abs | NumericFunc::Inv => true,

        // Temporal functions - NOT fusible (different semantics)
        NumericFunc::Dlog | NumericFunc::Ret => false,

        // Locf - NOT fusible (stateful, maintains last_valid)
        NumericFunc::Locf => false,

        // CumSum - NOT fusible (stateful, maintains running sum)
        NumericFunc::CumSum => false,

        // Shift - NOT fusible (stateful)
        NumericFunc::Shift { .. } => false,

        // Rolling - NOT fusible (already O(n), complex state)
        NumericFunc::RollMean { .. } | NumericFunc::RollStd { .. } => false,
    }
}

/// Extract a maximal unary chain starting from a node
fn extract_unary_chain(
    plan: &Plan,
    start: NodeId,
    consumers: &HashMap<NodeId, Vec<NodeId>>,
    visited: &mut [bool],
) -> Segment {
    let mut chain = vec![start];
    visited[start.0] = true;

    let mut current = start;
    loop {
        // Check if current node has exactly one consumer that's a fusible unary
        let consumer_list = consumers.get(&current).map(|v| v.as_slice()).unwrap_or(&[]);
        if consumer_list.len() != 1 {
            break;  // Multiple consumers or no consumers - end chain
        }

        let next_id = consumer_list[0];
        if visited[next_id.0] {
            break;  // Already processed
        }

        let next_node = plan.get_node(next_id).unwrap();
        match &next_node.op {
            Operation::Unary(UnaryOp::MapNumeric { input, func }) if *input == current => {
                if is_fusible_unary(*func) {
                    chain.push(next_id);
                    visited[next_id.0] = true;
                    current = next_id;
                } else {
                    break;  // Not fusible
                }
            }
            _ => break,  // Not a unary op or wrong input
        }
    }

    Segment {
        nodes: chain,
        kind: SegmentKind::UnaryChain,
    }
}

/// Extract a maximal scalar binary chain starting from a node
fn extract_scalar_binary_chain(
    plan: &Plan,
    start: NodeId,
    consumers: &HashMap<NodeId, Vec<NodeId>>,
    visited: &mut [bool],
) -> Segment {
    let mut chain = vec![start];
    visited[start.0] = true;

    let mut current = start;
    loop {
        // Check if current node has exactly one consumer that's a scalar binary
        let consumer_list = consumers.get(&current).map(|v| v.as_slice()).unwrap_or(&[]);
        if consumer_list.len() != 1 {
            break;
        }

        let next_id = consumer_list[0];
        if visited[next_id.0] {
            break;
        }

        let next_node = plan.get_node(next_id).unwrap();
        match &next_node.op {
            Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, .. }) if *lhs == current => {
                if let ValueRef::Scalar(_) = rhs {
                    chain.push(next_id);
                    visited[next_id.0] = true;
                    current = next_id;
                } else {
                    break;  // Not a scalar binary
                }
            }
            _ => break,
        }
    }

    Segment {
        nodes: chain,
        kind: SegmentKind::ScalarBinaryChain,
    }
}

/// Build a fused operation from a segment
pub fn fuse_segment(plan: &Plan, segment: &Segment) -> Option<FusedOperation> {
    match segment.kind {
        SegmentKind::UnaryChain if segment.nodes.len() > 1 => {
            let mut funcs = Vec::new();
            let mut input = None;

            for &node_id in &segment.nodes {
                let node = plan.get_node(node_id).unwrap();
                match &node.op {
                    Operation::Unary(UnaryOp::MapNumeric { input: inp, func }) => {
                        if input.is_none() {
                            input = Some(*inp);
                        }
                        funcs.push(*func);
                    }
                    _ => return None,  // Shouldn't happen
                }
            }

            Some(FusedOperation::FusedUnary {
                input: input.unwrap(),
                funcs,
            })
        }
        SegmentKind::ScalarBinaryChain if segment.nodes.len() > 1 => {
            let mut ops = Vec::new();
            let mut input = None;

            for &node_id in &segment.nodes {
                let node = plan.get_node(node_id).unwrap();
                match &node.op {
                    Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, func }) => {
                        if input.is_none() {
                            input = Some(*lhs);
                        }
                        if let ValueRef::Scalar(scalar) = rhs {
                            ops.push((*func, *scalar));
                        } else {
                            return None;  // Shouldn't happen
                        }
                    }
                    _ => return None,
                }
            }

            Some(FusedOperation::FusedScalarBinary {
                input: input.unwrap(),
                ops,
            })
        }
        _ => None,  // Not fusible or single node
    }
}

/// Execute a fused unary operation
///
/// Applies multiple numeric functions in sequence, reducing overhead from
/// intermediate frame allocations and Arc cloning.
pub fn execute_fused_unary(
    input: &Arc<Frame>,
    funcs: &[NumericFunc],
) -> Arc<Frame> {
    let result = map_numeric_preserve_tags(input, |col| {
        let mut current = col.clone();
        for func in funcs {
            current = apply_numeric_func(&current, *func);
        }
        current
    });
    Arc::new(result)
}

/// Execute a fused scalar binary operation
///
/// Applies multiple scalar operations in sequence, reducing overhead from
/// intermediate frame allocations.
pub fn execute_fused_scalar_binary(
    input: &Arc<Frame>,
    ops: &[(BinaryFunc, f64)],
) -> Arc<Frame> {
    let result = map_numeric_preserve_tags(input, |col| {
        let mut current = col.clone();
        for (func, scalar) in ops {
            current = apply_scalar_binary(&current, *func, *scalar);
        }
        current
    });
    Arc::new(result)
}

/// Apply a single numeric function to a column
fn apply_numeric_func(col: &Column, func: NumericFunc) -> Column {
    use crate::exec::{log_column, exp_column, sqrt_column, abs_column, inv_column};

    match func {
        NumericFunc::Log => log_column(col),
        NumericFunc::Exp => exp_column(col),
        NumericFunc::Sqrt => sqrt_column(col),
        NumericFunc::Abs => abs_column(col),
        NumericFunc::Inv => inv_column(col),

        // These should not be fused (filtered by is_fusible_unary)
        NumericFunc::Dlog | NumericFunc::Ret |
        NumericFunc::Locf | NumericFunc::CumSum |
        NumericFunc::Shift { .. } |
        NumericFunc::RollMean { .. } | NumericFunc::RollStd { .. } => {
            panic!("Attempted to fuse non-fusible operation: {:?}", func)
        }
    }
}

/// Apply a scalar binary operation to a column
fn apply_scalar_binary(col: &Column, func: BinaryFunc, scalar: f64) -> Column {
    match col {
        Column::F64(data) => {
            let result: Vec<f64> = data.iter().map(|&x| {
                if x.is_nan() {
                    f64::NAN
                } else {
                    match func {
                        BinaryFunc::Add => x + scalar,
                        BinaryFunc::Sub => x - scalar,
                        BinaryFunc::Mul => x * scalar,
                        BinaryFunc::Div => {
                            if scalar == 0.0 {
                                f64::NAN  // Division by zero
                            } else {
                                x / scalar
                            }
                        }
                        BinaryFunc::Gt => {
                            if x > scalar { 1.0 } else { 0.0 }
                        }
                    }
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
    use crate::ir::{SchemaInfo, IndexType};

    #[test]
    fn test_identify_unary_chain() {
        // Plan: x -> abs -> sqrt -> log
        let mut plan = Plan::new();

        let x_id = plan.add_node(Node {
            id: NodeId(0),
            op: Operation::Source(crate::ir::Source::Variable {
                name: crate::ast::SymbolId(0),
            }),
            schema: SchemaInfo::unknown(),
        });

        let abs_id = plan.add_node(Node {
            id: NodeId(1),
            op: Operation::Unary(UnaryOp::MapNumeric {
                input: x_id,
                func: NumericFunc::Abs,
            }),
            schema: SchemaInfo::unknown(),
        });

        let sqrt_id = plan.add_node(Node {
            id: NodeId(2),
            op: Operation::Unary(UnaryOp::MapNumeric {
                input: abs_id,
                func: NumericFunc::Sqrt,
            }),
            schema: SchemaInfo::unknown(),
        });

        let log_id = plan.add_node(Node {
            id: NodeId(3),
            op: Operation::Unary(UnaryOp::MapNumeric {
                input: sqrt_id,
                func: NumericFunc::Log,
            }),
            schema: SchemaInfo::unknown(),
        });

        let segments = identify_segments(&plan);

        // Should identify: [source (atomic), unary chain (abs->sqrt->log)]
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].kind, SegmentKind::Atomic);  // source
        assert_eq!(segments[1].kind, SegmentKind::UnaryChain);
        assert_eq!(segments[1].nodes.len(), 3);  // abs, sqrt, log
    }

    #[test]
    fn test_identify_scalar_binary_chain() {
        // Plan: x -> (*2.0) -> (+5.0)
        let mut plan = Plan::new();

        let x_id = plan.add_node(Node {
            id: NodeId(0),
            op: Operation::Source(crate::ir::Source::Variable {
                name: crate::ast::SymbolId(0),
            }),
            schema: SchemaInfo::unknown(),
        });

        let mul_id = plan.add_node(Node {
            id: NodeId(1),
            op: Operation::Binary(BinaryOp::MapNumeric2 {
                lhs: x_id,
                rhs: ValueRef::Scalar(2.0),
                func: BinaryFunc::Mul,
            }),
            schema: SchemaInfo::unknown(),
        });

        let add_id = plan.add_node(Node {
            id: NodeId(2),
            op: Operation::Binary(BinaryOp::MapNumeric2 {
                lhs: mul_id,
                rhs: ValueRef::Scalar(5.0),
                func: BinaryFunc::Add,
            }),
            schema: SchemaInfo::unknown(),
        });

        let segments = identify_segments(&plan);

        // Should identify: [source (atomic), scalar binary chain (mul->add)]
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].kind, SegmentKind::Atomic);  // source
        assert_eq!(segments[1].kind, SegmentKind::ScalarBinaryChain);
        assert_eq!(segments[1].nodes.len(), 2);  // mul, add
    }

    #[test]
    fn test_non_fusible_operations() {
        // Plan: x -> dlog (not fusible) -> log (fusible but alone)
        let mut plan = Plan::new();

        let x_id = plan.add_node(Node {
            id: NodeId(0),
            op: Operation::Source(crate::ir::Source::Variable {
                name: crate::ast::SymbolId(0),
            }),
            schema: SchemaInfo::unknown(),
        });

        let dlog_id = plan.add_node(Node {
            id: NodeId(1),
            op: Operation::Unary(UnaryOp::MapNumeric {
                input: x_id,
                func: NumericFunc::Dlog,
            }),
            schema: SchemaInfo::unknown(),
        });

        let log_id = plan.add_node(Node {
            id: NodeId(2),
            op: Operation::Unary(UnaryOp::MapNumeric {
                input: dlog_id,
                func: NumericFunc::Log,
            }),
            schema: SchemaInfo::unknown(),
        });

        let segments = identify_segments(&plan);

        // Should identify: [source (atomic), dlog (atomic), log (atomic)]
        // dlog breaks the chain because it's not fusible
        assert_eq!(segments.len(), 3);
        assert!(segments.iter().all(|s| s.kind == SegmentKind::Atomic));
    }

    #[test]
    fn test_fused_unary_equivalence() {
        // Test: (log (sqrt (abs x))) fused vs unfused produces same result
        use crate::frame::{Tags, IndexColumn};
        use blawktrust::Column;

        // Create test frame
        let data = vec![1.0, 4.0, 9.0, 16.0, 25.0];
        let col = Arc::new(Column::new_f64(data.clone()));

        let index = IndexColumn::Date(Arc::new(vec![1, 2, 3, 4, 5]));
        let tags = Tags::new("DATE".to_string(), index, vec!["x".to_string()]);
        let frame = Arc::new(Frame::new(tags, vec![Arc::clone(&col)]));

        // Execute unfused (step by step)
        let abs_result = crate::exec::abs_column(&col);
        let sqrt_result = crate::exec::sqrt_column(&abs_result);
        let log_result = crate::exec::log_column(&sqrt_result);

        // Execute fused
        let funcs = vec![NumericFunc::Abs, NumericFunc::Sqrt, NumericFunc::Log];
        let fused_result = execute_fused_unary(&frame, &funcs);

        // Compare results
        let unfused_col = &log_result;
        let fused_col = fused_result.get_col(0).unwrap();

        match (unfused_col, &**fused_col) {
            (Column::F64(unfused_data), Column::F64(fused_data)) => {
                assert_eq!(unfused_data.len(), fused_data.len());
                for (i, (&u, &f)) in unfused_data.iter().zip(fused_data.iter()).enumerate() {
                    if u.is_nan() && f.is_nan() {
                        // Both NA - OK
                    } else {
                        assert!((u - f).abs() < 1e-10, "Mismatch at index {}: {} vs {}", i, u, f);
                    }
                }
            }
            _ => panic!("Expected F64 columns"),
        }
    }

    #[test]
    fn test_fused_scalar_binary_equivalence() {
        // Test: (+ (* x 2.0) 5.0) fused vs unfused produces same result
        use crate::frame::{Tags, IndexColumn};
        use blawktrust::Column;

        // Create test frame
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let col = Arc::new(Column::new_f64(data.clone()));

        let index = IndexColumn::Date(Arc::new(vec![1, 2, 3, 4, 5]));
        let tags = Tags::new("DATE".to_string(), index, vec!["x".to_string()]);
        let frame = Arc::new(Frame::new(tags, vec![Arc::clone(&col)]));

        // Execute unfused (step by step)
        let mul_result = apply_scalar_binary(&col, BinaryFunc::Mul, 2.0);
        let add_result = apply_scalar_binary(&mul_result, BinaryFunc::Add, 5.0);

        // Execute fused
        let ops = vec![(BinaryFunc::Mul, 2.0), (BinaryFunc::Add, 5.0)];
        let fused_result = execute_fused_scalar_binary(&frame, &ops);

        // Compare results
        let unfused_col = &add_result;
        let fused_col = fused_result.get_col(0).unwrap();

        match (unfused_col, &**fused_col) {
            (Column::F64(unfused_data), Column::F64(fused_data)) => {
                assert_eq!(unfused_data.len(), fused_data.len());
                for (i, (&u, &f)) in unfused_data.iter().zip(fused_data.iter()).enumerate() {
                    assert!((u - f).abs() < 1e-10, "Mismatch at index {}: {} vs {}", i, u, f);
                }
            }
            _ => panic!("Expected F64 columns"),
        }
    }

    #[test]
    fn test_fused_preserves_arc_identity() {
        // Test: Fused operations preserve Arc identity (I1-I3)
        use crate::frame::{Tags, IndexColumn};
        use blawktrust::Column;

        let data = vec![1.0, 2.0, 3.0];
        let col = Arc::new(Column::new_f64(data));

        let index = IndexColumn::Date(Arc::new(vec![1, 2, 3]));
        let tags = Tags::new("DATE".to_string(), index, vec!["x".to_string()]);
        let frame = Arc::new(Frame::new(tags, vec![col]));

        // Execute fused operation
        let funcs = vec![NumericFunc::Abs, NumericFunc::Sqrt];
        let result = execute_fused_unary(&frame, &funcs);

        // Verify Arc identity (I1-I3)
        assert!(Arc::ptr_eq(&frame.tags.index, &result.tags.index),
            "I1: Index Arc identity not preserved");
        assert!(Arc::ptr_eq(&frame.tags.colnames, &result.tags.colnames),
            "I2: Colnames Arc identity not preserved");
        assert_eq!(frame.nrows, result.nrows,
            "I3: Row count not preserved");
    }
}
