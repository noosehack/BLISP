/// BLADE Phase 3: Intermediate Representation v1
///
/// Purpose: Minimal IR for pipeline compilation and optimization
///
/// Design principles:
/// 1. **Minimal node set** - Only proven primitives from Phase 2
/// 2. **Schema handles** - Arc references for zero-copy tag propagation
/// 3. **No index coercion** - Type safety enforced at IR level
/// 4. **Join semantics explicit** - Exact vs asof distinguished
///
/// This IR is designed to be:
/// - Easy to validate against contracts.md
/// - Easy to execute using frozen primitives
/// - Easy to extend with optimizations (future phases)

use std::sync::Arc;
use crate::frame::{Tags, IndexColumn};
use crate::ast::SymbolId;

/// IR node ID (unique within a plan)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

/// IR Plan - A DAG of operations
///
/// Invariants:
/// - Nodes are in topological order (dependencies before dependents)
/// - All NodeId references point to valid nodes in the plan
/// - Schema tracking is consistent with contracts.md
#[derive(Debug, Clone)]
pub struct Plan {
    pub nodes: Vec<Node>,
}

impl Plan {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Add a node to the plan and return its ID
    pub fn add_node(&mut self, node: Node) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(node);
        id
    }

    /// Get a node by ID
    pub fn get_node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0)
    }
}

/// IR Node - A single operation in the plan
#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub op: Operation,
    pub schema: SchemaInfo,
}

/// Schema information tracked at IR level
///
/// This enables compile-time validation of contracts:
/// - Index type matching for joins
/// - Arc preservation tracking
/// - Row count propagation
#[derive(Debug, Clone)]
pub struct SchemaInfo {
    /// Expected index type (None if unknown at compile time)
    pub index_type: Option<IndexType>,
    /// Expected column names (None if unknown at compile time)
    pub colnames: Option<Arc<Vec<String>>>,
    /// Expected row count (None if unknown at compile time)
    pub nrows: Option<usize>,
}

impl SchemaInfo {
    pub fn unknown() -> Self {
        Self {
            index_type: None,
            colnames: None,
            nrows: None,
        }
    }

    pub fn from_tags(tags: &Tags, nrows: usize) -> Self {
        Self {
            index_type: Some(IndexType::from_index_column(&tags.index)),
            colnames: Some(Arc::clone(&tags.colnames)),
            nrows: Some(nrows),
        }
    }
}

/// Index type discriminator (no coercion allowed)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexType {
    Date,
    Timestamp,
    String,
}

impl IndexType {
    pub fn from_index_column(index: &IndexColumn) -> Self {
        match index {
            IndexColumn::Date(_) => IndexType::Date,
            IndexColumn::Timestamp(_) => IndexType::Timestamp,
            IndexColumn::String(_) => IndexType::String,
        }
    }
}

/// IR Operation - The actual computation
#[derive(Debug, Clone)]
pub enum Operation {
    /// Load data from a source
    Source(Source),
    /// Apply a unary operation (preserves tags via I1-I3)
    Unary(UnaryOp),
    /// Apply a binary operation
    Binary(BinaryOp),
    /// Join two frames
    Join(JoinOp),
}

/// Data source operations
#[derive(Debug, Clone)]
pub enum Source {
    /// Load from CSV file
    File {
        path: String,
    },
    /// Reference a variable in the environment
    Variable {
        name: SymbolId,
    },
}

/// Unary operations (all preserve tags via map_numeric_preserve_tags)
///
/// Contract: Output has same index, colnames, and nrows as input
#[derive(Debug, Clone)]
pub enum UnaryOp {
    /// Map numeric function over columns
    MapNumeric {
        input: NodeId,
        func: NumericFunc,
    },
}

/// Numeric functions that can be mapped over columns
///
/// Each corresponds to a proven kernel from blawktrust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericFunc {
    /// Difference log: dlog(x) = log(x / x[-1])
    Dlog,
    /// Simple return: ret(x) = x / x[-1] - 1
    Ret,
    /// Natural log
    Log,
    /// Exponential
    Exp,
    /// Square root
    Sqrt,
    /// Absolute value
    Abs,
    /// Inverse: 1/x
    Inv,
    /// Shift (lag): shift k rows down
    ///
    /// Contract:
    /// - k ≥ 0 only (v1: no forward-looking)
    /// - Output[i] = Input[i-k] for i >= k, NA for i < k
    /// - Shape preserved (I1-I3)
    /// - NA mask monotone (only grows)
    Shift { k: usize },
}

/// Binary operations (element-wise combination of two inputs)
///
/// Contract: LHS tags preserved (Arc identity I1-I3)
/// RHS can be:
/// - Scalar: broadcast to all cells
/// - Frame: must have compatible tags (same index type, values, colnames)
#[derive(Debug, Clone)]
pub enum BinaryOp {
    /// Binary numeric operation
    MapNumeric2 {
        lhs: NodeId,
        rhs: ValueRef,
        func: BinaryFunc,
    },
}

/// Value reference - either a scalar constant or a frame reference
#[derive(Debug, Clone)]
pub enum ValueRef {
    /// Scalar constant (broadcast to all cells)
    Scalar(f64),
    /// Reference to another frame node
    Frame(NodeId),
}

/// Binary numeric functions
///
/// Semantics: operate cell-wise on LHS with RHS
/// NA propagation: if either cell is NA, result is NA
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryFunc {
    /// Addition
    Add,
    /// Subtraction
    Sub,
    /// Multiplication
    Mul,
    /// Division
    Div,
}

/// Join operations
///
/// Contract: Output has y's index and x's colnames
#[derive(Debug, Clone)]
pub enum JoinOp {
    /// Exact match join (RIGHT OUTER JOIN)
    ///
    /// Semantics: mapr(x, y)
    /// - Output index = y's index (Arc preserved)
    /// - Output colnames = x's colnames (Arc preserved)
    /// - Output nrows = y's nrows
    /// - Missing rows → NA
    MapR {
        x: NodeId,  // Source frame (provides data columns)
        y: NodeId,  // Target frame (provides index)
    },

    /// As-of join (RIGHT OUTER ASOF JOIN)
    ///
    /// Semantics: asofr(x, y)
    /// - Output index = y's index (Arc preserved)
    /// - Output colnames = x's colnames (Arc preserved)
    /// - Output nrows = y's nrows
    /// - At-or-before matching (no forward-looking)
    AsofR {
        x: NodeId,  // Source frame (provides data columns)
        y: NodeId,  // Target frame (provides index)
    },
}

impl Plan {
    /// Validate the plan against contracts.md
    ///
    /// Checks:
    /// 1. No index type coercion in joins
    /// 2. Schema consistency (Arc preservation)
    /// 3. Row count propagation
    pub fn validate(&self) -> Result<(), String> {
        for node in &self.nodes {
            match &node.op {
                Operation::Join(JoinOp::MapR { x, y }) | Operation::Join(JoinOp::AsofR { x, y }) => {
                    let x_node = self.get_node(*x).ok_or("Invalid x node reference")?;
                    let y_node = self.get_node(*y).ok_or("Invalid y node reference")?;

                    // Check index type compatibility
                    if let (Some(x_idx), Some(y_idx)) = (&x_node.schema.index_type, &y_node.schema.index_type) {
                        if x_idx != y_idx {
                            return Err(format!(
                                "Index type mismatch in join: {:?} vs {:?} (no coercion allowed)",
                                x_idx, y_idx
                            ));
                        }
                    }

                    // Verify output schema follows contracts
                    // Output index = y's index
                    if let Some(y_idx) = &y_node.schema.index_type {
                        if node.schema.index_type.as_ref() != Some(y_idx) {
                            return Err("Join output index must match y's index".to_string());
                        }
                    }

                    // Output nrows = y's nrows
                    if let Some(y_nrows) = y_node.schema.nrows {
                        if node.schema.nrows != Some(y_nrows) {
                            return Err("Join output nrows must match y's nrows".to_string());
                        }
                    }
                }
                Operation::Unary(UnaryOp::MapNumeric { input, .. }) => {
                    let input_node = self.get_node(*input).ok_or("Invalid input node reference")?;

                    // Verify I1-I3 invariants
                    if node.schema.index_type != input_node.schema.index_type {
                        return Err("Unary op must preserve index type (I1)".to_string());
                    }
                    if node.schema.nrows != input_node.schema.nrows {
                        return Err("Unary op must preserve nrows (I3)".to_string());
                    }
                    // Note: Arc equality (I2) checked at execution time
                }
                Operation::Binary(BinaryOp::MapNumeric2 { lhs, rhs, .. }) => {
                    let lhs_node = self.get_node(*lhs).ok_or("Invalid LHS node reference")?;

                    // Verify LHS tags preserved (I1-I3)
                    if node.schema.index_type != lhs_node.schema.index_type {
                        return Err("Binary op must preserve LHS index type (I1)".to_string());
                    }
                    if node.schema.nrows != lhs_node.schema.nrows {
                        return Err("Binary op must preserve LHS nrows (I3)".to_string());
                    }

                    // If RHS is a frame, enforce strict compatibility
                    if let ValueRef::Frame(rhs_id) = rhs {
                        let rhs_node = self.get_node(*rhs_id).ok_or("Invalid RHS node reference")?;

                        // Same index type (no coercion)
                        if let (Some(lhs_idx), Some(rhs_idx)) =
                            (&lhs_node.schema.index_type, &rhs_node.schema.index_type) {
                            if lhs_idx != rhs_idx {
                                return Err(format!(
                                    "Binary op requires compatible index types: {:?} vs {:?}. Use mapr/asofr for alignment.",
                                    lhs_idx, rhs_idx
                                ));
                            }
                        }

                        // Same nrows (strict shape match)
                        if let (Some(lhs_nrows), Some(rhs_nrows)) =
                            (lhs_node.schema.nrows, rhs_node.schema.nrows) {
                            if lhs_nrows != rhs_nrows {
                                return Err(format!(
                                    "Binary op requires compatible shapes: {} vs {} rows. Use mapr/asofr for alignment.",
                                    lhs_nrows, rhs_nrows
                                ));
                            }
                        }

                        // Same colnames (strict compatibility)
                        if let (Some(lhs_cols), Some(rhs_cols)) =
                            (&lhs_node.schema.colnames, &rhs_node.schema.colnames) {
                            if lhs_cols.len() != rhs_cols.len() {
                                return Err(format!(
                                    "Binary op requires same number of columns: {} vs {}",
                                    lhs_cols.len(), rhs_cols.len()
                                ));
                            }
                        }
                    }
                }
                Operation::Source(_) => {
                    // Sources have no inputs to validate
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_creation() {
        let mut plan = Plan::new();

        let source = plan.add_node(Node {
            id: NodeId(0),
            op: Operation::Source(Source::File {
                path: "data.csv".to_string(),
            }),
            schema: SchemaInfo::unknown(),
        });

        assert_eq!(source, NodeId(0));
        assert_eq!(plan.nodes.len(), 1);
    }

    #[test]
    fn test_unary_op() {
        let mut plan = Plan::new();

        let source = plan.add_node(Node {
            id: NodeId(0),
            op: Operation::Source(Source::File {
                path: "data.csv".to_string(),
            }),
            schema: SchemaInfo {
                index_type: Some(IndexType::Date),
                colnames: None,
                nrows: Some(100),
            },
        });

        let dlog = plan.add_node(Node {
            id: NodeId(1),
            op: Operation::Unary(UnaryOp::MapNumeric {
                input: source,
                func: NumericFunc::Dlog,
            }),
            schema: SchemaInfo {
                index_type: Some(IndexType::Date),
                colnames: None,
                nrows: Some(100),
            },
        });

        assert_eq!(dlog, NodeId(1));

        // Validate should pass (I1-I3 preserved)
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn test_join_op() {
        let mut plan = Plan::new();

        let x = plan.add_node(Node {
            id: NodeId(0),
            op: Operation::Source(Source::File {
                path: "x.csv".to_string(),
            }),
            schema: SchemaInfo {
                index_type: Some(IndexType::Date),
                colnames: None,
                nrows: Some(10),
            },
        });

        let y = plan.add_node(Node {
            id: NodeId(1),
            op: Operation::Source(Source::File {
                path: "y.csv".to_string(),
            }),
            schema: SchemaInfo {
                index_type: Some(IndexType::Date),
                colnames: None,
                nrows: Some(20),
            },
        });

        let join = plan.add_node(Node {
            id: NodeId(2),
            op: Operation::Join(JoinOp::MapR { x, y }),
            schema: SchemaInfo {
                index_type: Some(IndexType::Date),
                colnames: None,
                nrows: Some(20),  // y's nrows
            },
        });

        assert_eq!(join, NodeId(2));

        // Validate should pass
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn test_index_type_mismatch() {
        let mut plan = Plan::new();

        let x = plan.add_node(Node {
            id: NodeId(0),
            op: Operation::Source(Source::File {
                path: "x.csv".to_string(),
            }),
            schema: SchemaInfo {
                index_type: Some(IndexType::Date),
                colnames: None,
                nrows: Some(10),
            },
        });

        let y = plan.add_node(Node {
            id: NodeId(1),
            op: Operation::Source(Source::File {
                path: "y.csv".to_string(),
            }),
            schema: SchemaInfo {
                index_type: Some(IndexType::Timestamp),  // MISMATCH
                colnames: None,
                nrows: Some(20),
            },
        });

        let join = plan.add_node(Node {
            id: NodeId(2),
            op: Operation::Join(JoinOp::MapR { x, y }),
            schema: SchemaInfo {
                index_type: Some(IndexType::Timestamp),
                colnames: None,
                nrows: Some(20),
            },
        });

        // Validate should fail (index type mismatch)
        let result = plan.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Index type mismatch"));
    }

    #[test]
    fn test_unary_breaks_invariants() {
        let mut plan = Plan::new();

        let source = plan.add_node(Node {
            id: NodeId(0),
            op: Operation::Source(Source::File {
                path: "data.csv".to_string(),
            }),
            schema: SchemaInfo {
                index_type: Some(IndexType::Date),
                colnames: None,
                nrows: Some(100),
            },
        });

        // Bad: unary op changes nrows (violates I3)
        let _bad = plan.add_node(Node {
            id: NodeId(1),
            op: Operation::Unary(UnaryOp::MapNumeric {
                input: source,
                func: NumericFunc::Dlog,
            }),
            schema: SchemaInfo {
                index_type: Some(IndexType::Date),
                colnames: None,
                nrows: Some(99),  // WRONG - should be 100
            },
        });

        // Validate should fail (I3 violation)
        let result = plan.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("preserve nrows"));
    }
}
