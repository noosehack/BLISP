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
    /// Schema-transforming operation (I1+I3 preserved, I2 rebuilt)
    Schema(SchemaOp),
}

/// Data source operations
#[derive(Debug, Clone)]
pub enum Source {
    /// Load from CSV file
    File {
        path: String,
    },
    /// Read from stdin
    Stdin,
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
    /// Last observation carried forward (fill NA with last valid)
    ///
    /// Contract:
    /// - Leading NAs preserved until first valid value
    /// - After first valid: NA[i] = last valid value before i
    /// - Valid values pass through unchanged
    /// - Idempotent: locf(locf(x)) == locf(x)
    /// - Shape preserved (I1-I3)
    /// - NA policy: "skip" (carries previous value, not "poison")
    /// - NA mask: can only shrink after first valid (fills existing NAs)
    /// - Different from most ops: REDUCES NAs, doesn't grow them
    Locf,

    /// Weekday mask (wkd): Set weekend values to NA (shape-preserving)
    ///
    /// Contract:
    /// - **Shape-preserving**: All invariants maintained (I1, I2, I3)
    /// - For each row: if Saturday or Sunday → set all column values to NA
    /// - Weekday rows: values unchanged
    /// - Index unchanged (preserves time alignment for joins)
    /// - Colnames unchanged
    /// - Nrows unchanged
    /// - Composable: (wkd (wkd x)) == (wkd x)
    /// - Safe for backtesting: no time axis distortion
    /// - Works with rolling ops: window size remains consistent
    ///
    /// Why masking not filtering:
    /// - Preserves alignment with other series
    /// - No hidden schema rebuild
    /// - Deterministic shape
    /// - Composable with mask arithmetic: (* x (wkd signal))
    ///
    /// Notes:
    /// - Requires date index to determine weekday/weekend
    /// - Day of week: 0=Sunday, 1=Monday, ..., 6=Saturday
    /// - Weekdays: Monday-Friday (1-5)
    /// - Weekends: Saturday-Sunday (0, 6)
    Wkd,

    /// Cumulative sum starting at 1.0
    ///
    /// Contract (updated for shape-preserving wkd):
    /// - Starts at 1.0 (not 0.0!)
    /// - For valid values: cs1[i] = cs1[i-1] + x[i]
    /// - For NA values: cs1[i] = NA (preserves input NA)
    /// - NA policy: "skip and preserve"
    ///   - NA input → NA output (preserves weekend masks from wkd)
    ///   - Running sum continues across NA positions
    /// - Compatible with masked time series operations
    /// - Shape preserved (I1-I3)
    /// - Used for index reconstruction from differences
    /// - NOT idempotent (cs1(cs1(x)) != cs1(x))
    CumSum,
    /// Shift (lag): shift k rows down
    ///
    /// Contract:
    /// - k ≥ 0 only (v1: no forward-looking)
    /// - Output[i] = Input[i-k] for i >= k, NA for i < k
    /// - Shape preserved (I1-I3)
    /// - NA mask monotone (only grows)
    Shift { k: usize },
    /// Mask-aware shift: lag by k eligible (unmasked) observations
    /// Skips masked rows only (not NA values)
    /// For matching CLISPI's wkd-filtered behavior
    LagObs { k: usize },
    /// Keep every k-th row (shape-preserving)
    ///
    /// Contract:
    /// - Keeps rows where row_index % k == 0
    /// - Other rows filled with NA
    /// - Shape preserved (I1-I3): nrows unchanged
    /// - Used for downsampling while maintaining alignment
    /// - Example: k=5 keeps rows 0, 5, 10, 15, ...
    Keep { k: usize },
    /// Rolling mean: trailing window mean
    ///
    /// Contract (see contracts.md §5):
    /// - Trailing window: [i-w+1 .. i] inclusive
    /// - Skip NA in window, require w valid values (strict min_periods)
    /// - Prefix i < w-1 always NA
    /// - Shape preserved (I1-I3)
    /// - NA mask monotone
    RollMean { w: usize },
    /// Rolling standard deviation: trailing window std (population, ddof=0)
    ///
    /// Contract (see contracts.md §5):
    /// - Trailing window: [i-w+1 .. i] inclusive
    /// - Skip NA in window, require w valid values (strict min_periods)
    /// - Population std: σ = sqrt((1/w) * Σ(x-μ)²)
    /// - Constant series → σ = 0.0 (not NA)
    /// - Window=1 → σ = 0.0 for valid values
    /// - Prefix i < w-1 always NA
    /// - Shape preserved (I1-I3)
    /// - NA mask monotone
    RollStd { w: usize },
    /// Rolling mean (min 2 observations): relaxed min_periods for masked calendars
    ///
    /// Contract:
    /// - Trailing window: [i-w+1 .. i] inclusive
    /// - Skip NA in window, require ≥2 valid values (relaxed, not strict w)
    /// - Use available valid values only
    /// - Designed for: weekday-masked calendars with weekend NAs
    /// - Prefix i < w-1 always NA
    /// - Shape preserved (I1-I3)
    RollMeanMin2 { w: usize },
    /// Rolling std (min 2 observations): relaxed min_periods for masked calendars
    ///
    /// Contract:
    /// - Trailing window: [i-w+1 .. i] inclusive
    /// - Skip NA in window, require ≥2 valid values (relaxed, not strict w)
    /// - Use available valid values only
    /// - Designed for: weekday-masked calendars with weekend NAs
    /// - Prefix i < w-1 always NA
    /// - Shape preserved (I1-I3)
    RollStdMin2 { w: usize },
    /// Rolling mean (min 2 observations) excluding current observation - for ft-zscore
    ///
    /// Contract:
    /// - Window ending at i-1: [i-w .. i-1] inclusive, excluding current row i
    /// - At row i, uses stats from w eligible observations ending at or before i-1
    /// - Skip masked and NA observations when counting back
    /// - Require ≥2 valid values in window
    /// - Used by ft-zscore to avoid lookahead: zscore[i] uses stats from i-1 and earlier
    RollMeanMin2ExclCurrent { w: usize },
    /// Rolling std (min 2 observations) excluding current observation - for ft-zscore
    ///
    /// Same contract as RollMeanMin2ExclCurrent but for standard deviation
    RollStdMin2ExclCurrent { w: usize },
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
    /// Greater than: x > y → 1.0 (true), 0.0 (false), NA (if either is NA)
    Gt,
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

/// Schema-transforming operations (I2_schema invariant)
///
/// Contract:
/// - I1 preserved: index Arc ptr_eq
/// - I2_schema: colnames Arc REBUILT (not preserved, but deterministic)
/// - I3 preserved: nrows unchanged
#[derive(Debug, Clone)]
pub enum SchemaOp {
    /// Pairwise spreads: column differences
    ///
    /// Semantics: xminus(data, half)
    /// - half=false: All pairs (nc*(nc-1) columns) - A-B, B-A, C-A, C-B, B-C, A-C
    /// - half=true: Upper triangle only (nc*(nc-1)/2 columns) - A-B, A-C, B-C
    /// - Column naming: "colA\colB" (backslash separator)
    /// - NA policy: if either input NA, output NA
    /// - Output ncols: depends on half mode
    /// - Output colnames: newly generated Arc (deterministic order)
    Xminus {
        input: NodeId,
        half: bool,  // false = all pairs, true = upper triangle only
    },

    /// Create weekend mask: (mask-weekend frame [name])
    ///
    /// Contract:
    /// - I1 preserved: index Arc ptr_eq
    /// - I2 preserved: colnames Arc ptr_eq
    /// - I3 preserved: nrows unchanged
    /// - Tags modified: adds named mask to masks, active_mask unchanged
    /// - Data columns unchanged (Arc ptr_eq)
    ///
    /// Semantics:
    /// - Computes weekend bitvec from index (Saturday=true, Sunday=true)
    /// - Stores in frame.tags.masks[name]
    /// - Does NOT activate the mask
    MaskWeekend {
        input: NodeId,
        name: Option<String>,  // default: "weekend"
    },

    /// Activate mask: (with-mask frame mask-expr)
    ///
    /// Contract:
    /// - I1 preserved: index Arc ptr_eq
    /// - I2 preserved: colnames Arc ptr_eq
    /// - I3 preserved: nrows unchanged
    /// - Tags modified: active_mask set to compiled expression
    /// - Masks unchanged: does not add/remove named masks
    /// - Data columns unchanged (Arc ptr_eq)
    ///
    /// Semantics:
    /// - Compiles mask_expr using frame.tags.masks
    /// - Sets frame.tags.active_mask to result
    /// - All subsequent operations respect active_mask
    WithMask {
        input: NodeId,
        mask_expr: crate::mask::MaskExpr,
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
                Operation::Schema(SchemaOp::Xminus { input, .. }) => {
                    let input_node = self.get_node(*input).ok_or("Invalid input node reference")?;

                    // Verify I1 + I3 preserved (I2_schema allows colname rebuild)
                    if node.schema.index_type != input_node.schema.index_type {
                        return Err("Schema op must preserve index type (I1)".to_string());
                    }
                    if node.schema.nrows != input_node.schema.nrows {
                        return Err("Schema op must preserve nrows (I3)".to_string());
                    }
                    // Note: colnames (I2) are intentionally rebuilt for schema ops
                }

                Operation::Schema(SchemaOp::MaskWeekend { input, .. }) => {
                    let input_node = self.get_node(*input).ok_or("Invalid input node reference")?;

                    // Verify I1 + I2 + I3 all preserved (mask ops only modify Tags metadata)
                    if node.schema.index_type != input_node.schema.index_type {
                        return Err("mask-weekend must preserve index type (I1)".to_string());
                    }
                    if node.schema.nrows != input_node.schema.nrows {
                        return Err("mask-weekend must preserve nrows (I3)".to_string());
                    }
                    // I2 (colnames) also preserved for mask ops
                }

                Operation::Schema(SchemaOp::WithMask { input, .. }) => {
                    let input_node = self.get_node(*input).ok_or("Invalid input node reference")?;

                    // Verify I1 + I2 + I3 all preserved (mask ops only modify Tags metadata)
                    if node.schema.index_type != input_node.schema.index_type {
                        return Err("with-mask must preserve index type (I1)".to_string());
                    }
                    if node.schema.nrows != input_node.schema.nrows {
                        return Err("with-mask must preserve nrows (I3)".to_string());
                    }
                    // I2 (colnames) also preserved for mask ops
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
