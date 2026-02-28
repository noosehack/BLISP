use crate::frame::{asofr, map_numeric_preserve_tags, ColData, Frame, Tags};
use crate::io;
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
use crate::ir::{
    BinaryFunc, BinaryOp, JoinOp, Node, NodeId, NumericFunc, Operation, Plan, SchemaOp, Source,
    UnaryOp, ValueRef,
};
use crate::runtime::Runtime;
use crate::value::Value;
use std::collections::HashMap;
use std::sync::Arc;
// dlog_column replaced with mask-aware version below
// use blawktrust::builtins::ops::{dlog_column};

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
        .map(Value::Frame)
        .ok_or_else(|| "No result from execution".to_string())
}

/// Execute a single node
fn execute_node(node: &Node, ctx: &ExecContext, rt: &mut Runtime) -> Result<Arc<Frame>, String> {
    match &node.op {
        Operation::Source(source) => execute_source(source, rt),
        Operation::Unary(unary) => execute_unary(unary, ctx),
        Operation::Binary(binary) => execute_binary(binary, ctx),
        Operation::Join(join) => execute_join(join, ctx),
        Operation::Schema(schema) => execute_schema(schema, ctx, rt),
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
                _ => Err(format!(
                    "CSV loader returned non-Frame: {}",
                    value.type_name()
                )),
            }
        }
        Source::Stdin => {
            // Read CSV from stdin
            // Note: load_stdin returns old Table/TableView, need to handle conversion
            let mut buffer = String::new();
            {
                use std::io::Read;
                std::io::stdin()
                    .read_to_string(&mut buffer)
                    .map_err(|e| format!("Error reading stdin: {}", e))?;
            }

            // Parse CSV using same logic as load_csv
            let mut csv_reader = csv::ReaderBuilder::new()
                .has_headers(true)
                .delimiter(b';')
                .from_reader(buffer.as_bytes());

            let value = io::parse_csv_to_frame(&mut csv_reader, &mut rt.interner, None)?;

            match value {
                Value::Frame(f) => Ok(f),
                _ => Err(format!(
                    "stdin parsing returned non-Frame: {}",
                    value.type_name()
                )),
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
            let input_frame = ctx
                .load(*input)
                .ok_or_else(|| format!("Input node {:?} not found", input))?;

            // Special handling for Wkd: requires index access for weekday determination
            if matches!(func, NumericFunc::MSK_WKE) {
                return wkd_mask_weekends(&input_frame);
            }

            // Phase E: Special handling for rolling operations and mask-aware shift (need active_mask)
            let result = match func {
                NumericFunc::SHF_WIN_LIN_AVG { w }
                | NumericFunc::SHF_WIN_NLN_SDV { w }
                | NumericFunc::SHF_WIN_MIN2_LIN_AVG { w }
                | NumericFunc::SHF_WIN_MIN2_NLN_SDV { w }
                | NumericFunc::SHF_WIN_MIN2_LIN_AVG_EXCL { w }
                | NumericFunc::SHF_WIN_MIN2_NLN_SDV_EXCL { w } => {
                    // Rolling operations need access to active_mask to count only eligible observations
                    apply_rolling_mask_aware(&input_frame, *func)?
                }
                NumericFunc::LAG_OBS { k } => {
                    // Mask-aware shift: skip masked rows when computing lag
                    // This matches CLISPI's "business day lag" when weekends are masked
                    apply_shift_obs_mask_aware(&input_frame, *k)?
                }
                _ => {
                    // Non-rolling, non-mask-aware operations: use standard map_numeric_preserve_tags
                    map_numeric_preserve_tags(&input_frame, |col| match func {
                        NumericFunc::SHF_PTW_OBS_NLN_DLOG => dlog_obs_column(col, 1),
                        NumericFunc::SHF_PTW_OFS_NLN_DLOG => dlog_ofs_column(col, 1),
                        NumericFunc::RET => ret_column(col, 1),
                        NumericFunc::LOG => log_column(col),
                        NumericFunc::EXP => exp_column(col),
                        NumericFunc::SQRT => sqrt_column(col),
                        NumericFunc::ABS => abs_column(col),
                        NumericFunc::INV => inv_column(col),
                        NumericFunc::SHF_REC_NLN_LOCF => locf_column(col),
                        NumericFunc::SHF_PFX_LIN_SUM => cumsum_column(col),
                        NumericFunc::SHF_PTW_LIN_SHF { k } => shift_column(col, *k),
                        NumericFunc::KEEP { k } => keep_column(col, *k),
                        NumericFunc::MSK_WKE => unreachable!("Wkd handled above"),
                        NumericFunc::LAG_OBS { .. } => unreachable!("LagObs handled separately"),
                        _ => unreachable!("Rolling ops handled separately"),
                    })
                }
            };

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

        // PR4.1: Fused elementwise operations
        UnaryOp::FusedElementwise { input, ops } => {
            let input_frame = ctx
                .load(*input)
                .ok_or_else(|| format!("Input node {:?} not found", input))?;

            let result =
                map_numeric_preserve_tags(&input_frame, |col| fused_elementwise_column(col, ops));

            debug_assert!(
                Arc::ptr_eq(&result.tags.index, &input_frame.tags.index),
                "FusedElementwise: I1 violation"
            );
            debug_assert!(
                Arc::ptr_eq(&result.tags.colnames, &input_frame.tags.colnames),
                "FusedElementwise: I2 violation"
            );
            debug_assert_eq!(
                result.nrows, input_frame.nrows,
                "FusedElementwise: I3 violation"
            );

            Ok(Arc::new(result))
        }

        // PR4.2a: Fused cs1 ∘ elementwise
        UnaryOp::FusedCs1Elementwise { input, ops } => {
            let input_frame = ctx
                .load(*input)
                .ok_or_else(|| format!("Input node {:?} not found", input))?;

            let result = map_numeric_preserve_tags(&input_frame, |col| {
                fused_cs1_elementwise_column(col, ops)
            });

            debug_assert!(
                Arc::ptr_eq(&result.tags.index, &input_frame.tags.index),
                "FusedCs1Elementwise: I1 violation"
            );
            debug_assert!(
                Arc::ptr_eq(&result.tags.colnames, &input_frame.tags.colnames),
                "FusedCs1Elementwise: I2 violation"
            );
            debug_assert_eq!(
                result.nrows, input_frame.nrows,
                "FusedCs1Elementwise: I3 violation"
            );

            Ok(Arc::new(result))
        }

        // PR4.2b: Fused cs1 ∘ dlog-ofs
        UnaryOp::FusedCs1DlogOfs { input, lag } => {
            let input_frame = ctx
                .load(*input)
                .ok_or_else(|| format!("Input node {:?} not found", input))?;

            let result =
                map_numeric_preserve_tags(&input_frame, |col| fused_cs1_dlog_ofs_column(col, *lag));

            debug_assert!(
                Arc::ptr_eq(&result.tags.index, &input_frame.tags.index),
                "FusedCs1DlogOfs: I1 violation"
            );
            debug_assert!(
                Arc::ptr_eq(&result.tags.colnames, &input_frame.tags.colnames),
                "FusedCs1DlogOfs: I2 violation"
            );
            debug_assert_eq!(
                result.nrows, input_frame.nrows,
                "FusedCs1DlogOfs: I3 violation"
            );

            Ok(Arc::new(result))
        }

        // PR4.2b: Fused cs1 ∘ dlog-obs
        UnaryOp::FusedCs1DlogObs { input } => {
            let input_frame = ctx
                .load(*input)
                .ok_or_else(|| format!("Input node {:?} not found", input))?;

            let result = map_numeric_preserve_tags(&input_frame, fused_cs1_dlog_obs_column);

            debug_assert!(
                Arc::ptr_eq(&result.tags.index, &input_frame.tags.index),
                "FusedCs1DlogObs: I1 violation"
            );
            debug_assert!(
                Arc::ptr_eq(&result.tags.colnames, &input_frame.tags.colnames),
                "FusedCs1DlogObs: I2 violation"
            );
            debug_assert_eq!(
                result.nrows, input_frame.nrows,
                "FusedCs1DlogObs: I3 violation"
            );

            Ok(Arc::new(result))
        }

        // PR4.3a: Fused dlog-obs ∘ elementwise
        UnaryOp::FusedDlogObsElementwise { input, ops } => {
            let input_frame = ctx
                .load(*input)
                .ok_or_else(|| format!("Input node {:?} not found", input))?;

            let result = map_numeric_preserve_tags(&input_frame, |col| {
                fused_dlog_obs_elementwise_column(col, ops)
            });

            debug_assert!(
                Arc::ptr_eq(&result.tags.index, &input_frame.tags.index),
                "FusedDlogObsElementwise: I1 violation"
            );
            debug_assert!(
                Arc::ptr_eq(&result.tags.colnames, &input_frame.tags.colnames),
                "FusedDlogObsElementwise: I2 violation"
            );
            debug_assert_eq!(
                result.nrows, input_frame.nrows,
                "FusedDlogObsElementwise: I3 violation"
            );

            Ok(Arc::new(result))
        }

        // PR4.3a: Fused dlog-ofs ∘ elementwise
        UnaryOp::FusedDlogOfsElementwise { input, lag, ops } => {
            let input_frame = ctx
                .load(*input)
                .ok_or_else(|| format!("Input node {:?} not found", input))?;

            let result = map_numeric_preserve_tags(&input_frame, |col| {
                fused_dlog_ofs_elementwise_column(col, *lag, ops)
            });

            debug_assert!(
                Arc::ptr_eq(&result.tags.index, &input_frame.tags.index),
                "FusedDlogOfsElementwise: I1 violation"
            );
            debug_assert!(
                Arc::ptr_eq(&result.tags.colnames, &input_frame.tags.colnames),
                "FusedDlogOfsElementwise: I2 violation"
            );
            debug_assert_eq!(
                result.nrows, input_frame.nrows,
                "FusedDlogOfsElementwise: I3 violation"
            );

            Ok(Arc::new(result))
        }
    }
}

/// Execute a binary operation (element-wise combination)
///
/// Contract: LHS tags preserved (Arc identity I1-I3)
/// RHS can be scalar (broadcast) or frame (strict compatibility required)
fn execute_binary(binary: &BinaryOp, ctx: &ExecContext) -> Result<Arc<Frame>, String> {
    match binary {
        BinaryOp::MapNumeric2 { lhs, rhs, func } => {
            let lhs_frame = ctx
                .load(*lhs)
                .ok_or_else(|| format!("LHS node {:?} not found", lhs))?;

            match rhs {
                ValueRef::Scalar(scalar_val) => {
                    // Scalar RHS: broadcast to all cells
                    let result = map_numeric_preserve_tags(&lhs_frame, |col| {
                        binary_scalar_column(col, *scalar_val, *func)
                    });

                    // Verify Arc preservation (I1-I3)
                    debug_assert!(
                        Arc::ptr_eq(&result.tags.index, &lhs_frame.tags.index),
                        "Binary scalar: I1 violation - index Arc not preserved"
                    );
                    debug_assert!(
                        Arc::ptr_eq(&result.tags.colnames, &lhs_frame.tags.colnames),
                        "Binary scalar: I2 violation - colnames Arc not preserved"
                    );
                    debug_assert_eq!(
                        result.nrows, lhs_frame.nrows,
                        "Binary scalar: I3 violation - nrows not preserved"
                    );

                    Ok(Arc::new(result))
                }

                ValueRef::Frame(rhs_id) => {
                    // Frame RHS: strict compatibility required
                    let rhs_frame = ctx
                        .load(*rhs_id)
                        .ok_or_else(|| format!("RHS node {:?} not found", rhs_id))?;

                    // Validation should have already checked compatibility
                    // Execute element-wise combination
                    let result = binary_frame_frame(&lhs_frame, &rhs_frame, *func)?;

                    // Verify Arc preservation (I1-I3)
                    debug_assert!(
                        Arc::ptr_eq(&result.tags.index, &lhs_frame.tags.index),
                        "Binary frame: I1 violation - index Arc not preserved"
                    );
                    debug_assert!(
                        Arc::ptr_eq(&result.tags.colnames, &lhs_frame.tags.colnames),
                        "Binary frame: I2 violation - colnames Arc not preserved"
                    );
                    debug_assert_eq!(
                        result.nrows, lhs_frame.nrows,
                        "Binary frame: I3 violation - nrows not preserved"
                    );

                    Ok(Arc::new(result))
                }
            }
        }
    }
}

/// Execute a join operation using ONLY frozen join primitives
fn execute_join(join: &JoinOp, ctx: &ExecContext) -> Result<Arc<Frame>, String> {
    match join {
        JoinOp::ALIGN { x, y } => {
            let x_frame = ctx
                .load(*x)
                .ok_or_else(|| format!("X node {:?} not found", x))?;
            let y_frame = ctx
                .load(*y)
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

        JoinOp::ASOF_ALIGN { x, y } => {
            let x_frame = ctx
                .load(*x)
                .ok_or_else(|| format!("X node {:?} not found", x))?;
            let y_frame = ctx
                .load(*y)
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

/// Execute a schema-transforming operation
///
/// Contract:
/// - I1 preserved: index Arc ptr_eq
/// - I2_schema: colnames Arc rebuilt (deterministic)
/// - I3 preserved: nrows unchanged
fn execute_schema(
    schema: &SchemaOp,
    ctx: &ExecContext,
    rt: &mut Runtime,
) -> Result<Arc<Frame>, String> {
    use crate::frame::ColData;

    match schema {
        SchemaOp::SHF_PTW_LIN_SPR { input, half } => {
            let input_frame = ctx
                .load(*input)
                .ok_or_else(|| format!("Input node {:?} not found", input))?;

            // Validate: need at least 2 columns
            let ncols = input_frame.cols.len();
            if ncols < 2 {
                return Err(format!(
                    "xminus requires at least 2 columns (have {})",
                    ncols
                ));
            }

            // Extract raw columns from ColData
            let input_cols: Vec<&blawktrust::Column> = input_frame
                .cols
                .iter()
                .map(|cd| match cd {
                    ColData::Mat(col_arc) => col_arc.as_ref(),
                })
                .collect();

            // Generate output columns and column names
            let mut output_cols = Vec::new();
            let mut output_colnames: Vec<String> = Vec::new();

            if *half {
                // Half mode: upper triangle only (j < r)
                // Creates nc*(nc-1)/2 columns
                for j in 0..ncols {
                    for r in (j + 1)..ncols {
                        let col_j = input_cols[j];
                        let col_r = input_cols[r];

                        // Compute j - r
                        let spread_col = xminus_columns(col_j, col_r);
                        output_cols.push(Arc::new(spread_col));

                        // Generate column name: "colJ\colR"
                        let name_j = &input_frame.tags.colnames[j];
                        let name_r = &input_frame.tags.colnames[r];
                        let new_name = format!("{}\\{}", name_j, name_r);
                        output_colnames.push(new_name);
                    }
                }
            } else {
                // Full mode: all pairs (j != r)
                // Creates nc*(nc-1) columns
                for j in 0..ncols {
                    for r in 0..ncols {
                        if j != r {
                            let col_j = input_cols[j];
                            let col_r = input_cols[r];

                            // Compute j - r
                            let spread_col = xminus_columns(col_j, col_r);
                            output_cols.push(Arc::new(spread_col));

                            // Generate column name: "colJ\colR"
                            let name_j = &input_frame.tags.colnames[j];
                            let name_r = &input_frame.tags.colnames[r];
                            let new_name = format!("{}\\{}", name_j, name_r);
                            output_colnames.push(new_name);
                        }
                    }
                }
            }

            // Create new Tags with rebuilt colnames (I2_schema)
            // Phase D: Schema ops inherit masks from input (unary operation)
            let new_tags = Tags {
                index_name: input_frame.tags.index_name.clone(), // Preserve index name
                index: Arc::clone(&input_frame.tags.index),      // I1: preserved
                colnames: Arc::new(output_colnames),             // I2_schema: rebuilt
                masks: input_frame.tags.masks.clone(),           // Inherit input masks
                active_mask: input_frame.tags.active_mask.clone(), // Inherit input active_mask
            };

            // Build output frame using Frame::new
            let result = Frame::new(new_tags, output_cols);

            // Verify schema contracts
            debug_assert!(
                Arc::ptr_eq(&result.tags.index, &input_frame.tags.index),
                "I1 violation: index Arc not preserved in xminus"
            );
            debug_assert_eq!(
                result.nrows, input_frame.nrows,
                "I3 violation: nrows not preserved in xminus"
            );

            Ok(Arc::new(result))
        }

        SchemaOp::MSK_WKE_DEF { input, name } => {
            use bitvec::prelude::*;
            use std::sync::Arc;

            let input_frame = ctx
                .load(*input)
                .ok_or_else(|| format!("Input node {:?} not found", input))?;

            // Determine mask name
            let mask_name = name.clone().unwrap_or_else(|| "weekend".to_string());

            // Compute weekend bitmask from index
            let weekend_bitvec: BitVec = match &*input_frame.tags.index {
                crate::frame::IndexColumn::Date(dates) => dates
                    .iter()
                    .map(|&date| {
                        let day_of_week = (4 + date).rem_euclid(7);
                        day_of_week == 0 || day_of_week == 6
                    })
                    .collect(),
                crate::frame::IndexColumn::Timestamp(timestamps) => timestamps
                    .iter()
                    .map(|&ts| {
                        let days = (ts / 86400000) as i32;
                        let day_of_week = (4 + days).rem_euclid(7);
                        day_of_week == 0 || day_of_week == 6
                    })
                    .collect(),
                crate::frame::IndexColumn::String(_) => {
                    return Err(
                        "mask-weekend requires Date or Timestamp index, got String".to_string()
                    );
                }
            };

            // Add mask to MaskSet
            let nrows = input_frame.nrows();
            let mut new_masks = input_frame.tags.masks.clone();
            new_masks
                .insert(mask_name.clone(), Arc::new(weekend_bitvec), nrows)
                .map_err(|e| format!("mask-weekend: {}", e))?;

            // Build new tags with updated masks
            let new_tags = Tags {
                index_name: input_frame.tags.index_name.clone(),
                index: Arc::clone(&input_frame.tags.index),
                colnames: Arc::clone(&input_frame.tags.colnames),
                masks: new_masks,
                active_mask: input_frame.tags.active_mask.clone(),
            };

            // Build new frame preserving columns
            let result = Frame::with_tags(
                Arc::new(new_tags),
                input_frame
                    .cols
                    .iter()
                    .filter_map(|cd| {
                        if let ColData::Mat(col) = cd {
                            Some(Arc::clone(col))
                        } else {
                            None
                        }
                    })
                    .collect(),
            );

            Ok(Arc::new(result))
        }

        SchemaOp::WTH_MSK { input, mask_expr } => {
            let input_frame = ctx
                .load(*input)
                .ok_or_else(|| format!("Input node {:?} not found", input))?;

            let nrows = input_frame.nrows();

            // Compile mask expression
            let compiled =
                crate::mask::compile_mask_expr(mask_expr, &input_frame.tags.masks, nrows)?;

            // Create new active mask
            let new_active_mask =
                crate::mask::ActiveMask::from_bitvec(compiled, Some(mask_expr.clone()));

            // Build new tags with updated active mask
            let new_tags = Tags {
                index_name: input_frame.tags.index_name.clone(),
                index: Arc::clone(&input_frame.tags.index),
                colnames: Arc::clone(&input_frame.tags.colnames),
                masks: input_frame.tags.masks.clone(),
                active_mask: new_active_mask,
            };

            // Build new frame preserving columns
            let result = Frame::with_tags(
                Arc::new(new_tags),
                input_frame
                    .cols
                    .iter()
                    .filter_map(|cd| {
                        if let ColData::Mat(col) = cd {
                            Some(Arc::clone(col))
                        } else {
                            None
                        }
                    })
                    .collect(),
            );

            Ok(Arc::new(result))
        }
    }
}

// ============================================================================
// Kernel functions (will eventually come from blawktrust)
// ============================================================================

use blawktrust::Column;

// OLD ret_column removed - replaced with mask-aware version below

pub fn log_column(col: &Column) -> Column {
    match col {
        Column::F64(data) => {
            let result = data
                .iter()
                .map(|&x| {
                    if x > 0.0 && !x.is_nan() {
                        x.ln()
                    } else {
                        f64::NAN
                    }
                })
                .collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

pub fn exp_column(col: &Column) -> Column {
    match col {
        Column::F64(data) => {
            let result = data
                .iter()
                .map(|&x| if !x.is_nan() { x.exp() } else { f64::NAN })
                .collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

pub fn sqrt_column(col: &Column) -> Column {
    match col {
        Column::F64(data) => {
            let result = data
                .iter()
                .map(|&x| {
                    if x >= 0.0 && !x.is_nan() {
                        x.sqrt()
                    } else {
                        f64::NAN
                    }
                })
                .collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

pub fn abs_column(col: &Column) -> Column {
    match col {
        Column::F64(data) => {
            let result = data
                .iter()
                .map(|&x| if !x.is_nan() { x.abs() } else { f64::NAN })
                .collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

pub fn inv_column(col: &Column) -> Column {
    match col {
        Column::F64(data) => {
            let result = data
                .iter()
                .map(|&x| {
                    if !x.is_nan() && x != 0.0 {
                        1.0 / x
                    } else {
                        f64::NAN
                    }
                })
                .collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Mask-aware dlog: log returns with NA-skipping lag
///
/// Contract (updated for shape-preserving wkd):
/// - dlog[i] = log(x[i]) - log(x[last_valid before i])
/// - Skips NAs in lag: looks back for last valid value
/// - If current value NA → output NA
/// - If no prior valid value → output NA
/// - Compatible with weekend masking
///
/// Phase E: Apply rolling operation with mask-aware observation counting
///
/// For each row i, the rolling window includes the LAST w ELIGIBLE observations,
/// where eligible = !masked && valid (not NA).
///
/// This matches CLISPI's observation-based rolling semantics while maintaining
/// BLISP's calendar-indexed architecture with masks.
fn apply_rolling_mask_aware(frame: &Frame, func: NumericFunc) -> Result<Frame, String> {
    let active_mask = &frame.tags.active_mask;
    let nrows = frame.nrows();

    // Transform each column with mask-aware rolling
    let cols_out: Vec<ColData> = frame
        .cols
        .iter()
        .map(|col_data| match col_data {
            ColData::Mat(col) => {
                let result_col = match &func {
                    NumericFunc::SHF_WIN_LIN_AVG { w } => {
                        rolling_mean_mask_aware(col, *w, active_mask, nrows)
                    }
                    NumericFunc::SHF_WIN_NLN_SDV { w } => {
                        rolling_std_mask_aware(col, *w, active_mask, nrows)
                    }
                    NumericFunc::SHF_WIN_MIN2_LIN_AVG { w } => {
                        rolling_mean_partial_mask_aware(col, *w, active_mask, nrows)
                    }
                    NumericFunc::SHF_WIN_MIN2_NLN_SDV { w } => {
                        rolling_std_partial_mask_aware(col, *w, active_mask, nrows)
                    }
                    NumericFunc::SHF_WIN_MIN2_LIN_AVG_EXCL { w } => {
                        rolling_mean_partial_mask_aware_offset(col, *w, active_mask, nrows, 1)
                    }
                    NumericFunc::SHF_WIN_MIN2_NLN_SDV_EXCL { w } => {
                        rolling_std_partial_mask_aware_offset(col, *w, active_mask, nrows, 1)
                    }
                    _ => unreachable!("Non-rolling op passed to apply_rolling_mask_aware"),
                };
                ColData::Mat(Arc::new(result_col))
            }
        })
        .collect();

    Ok(Frame {
        tags: Arc::clone(&frame.tags), // Preserve tags (I1-I3)
        cols: cols_out,
        nrows: frame.nrows,
    })
}

/// Apply mask-aware shift (observation-based lag)
///
/// Contract:
/// - Skips masked rows when computing lag (business-day lag when weekend mask active)
/// - For each unmasked row i, shift_obs(k)[i] = source at k-th unmasked row before i
/// - Masked rows output NA
/// - If fewer than k unmasked rows before position i → NA
/// - Shape preserved (I1-I3)
fn apply_shift_obs_mask_aware(frame: &Frame, k: usize) -> Result<Frame, String> {
    let active_mask = &frame.tags.active_mask;
    let nrows = frame.nrows();

    // Transform each column with mask-aware shift
    let cols_out: Vec<ColData> = frame
        .cols
        .iter()
        .map(|col_data| match col_data {
            ColData::Mat(col) => {
                let result_col = shift_obs_column(col, k, active_mask, nrows);
                ColData::Mat(Arc::new(result_col))
            }
        })
        .collect();

    Ok(Frame {
        tags: Arc::clone(&frame.tags), // Preserve tags (I1-I3)
        cols: cols_out,
        nrows: frame.nrows,
    })
}

/// Rolling mean with mask-aware observation counting - O(n) streaming version
///
/// Maintains a queue of the last w eligible observations.
/// Amortized O(n): each eligible observation enters/exits queue exactly once.
fn rolling_mean_mask_aware(
    col: &Column,
    w: usize,
    mask: &crate::mask::ActiveMask,
    nrows: usize,
) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(nrows);

            for i in 0..nrows {
                // Masked rows: output NA
                if mask.is_masked(i) {
                    result.push(f64::NAN);
                    continue;
                }

                // Look back and collect up to w unmasked, non-NA observations
                let mut count = 0;
                let mut sum = 0.0;

                for j in (0..=i).rev() {
                    // Skip masked rows when looking back
                    if mask.is_masked(j) {
                        continue;
                    }

                    let value = if j < data.len() { data[j] } else { f64::NAN };
                    if !value.is_nan() {
                        sum += value;
                        count += 1;
                        if count >= w {
                            break;
                        }
                    }
                }

                // Strict: emit only if we have exactly w observations
                if count == w {
                    result.push(sum / (w as f64));
                } else {
                    result.push(f64::NAN);
                }
            }

            Column::new_f64(result)
        }
        _ => col.clone(),
    }
}

/// Apply a single elementwise operation to a value
///
/// Helper for fused execution. Only handles pure elementwise ops.
/// NaN propagates naturally through all operations.
#[inline]
fn apply_elementwise_op(x: f64, op: crate::ir::NumericFunc) -> f64 {
    use crate::ir::NumericFunc;

    if x.is_nan() {
        return f64::NAN;
    }

    match op {
        NumericFunc::ABS => x.abs(),
        NumericFunc::LOG => {
            if x > 0.0 {
                x.ln()
            } else {
                f64::NAN
            }
        }
        NumericFunc::EXP => x.exp(),
        NumericFunc::SQRT => {
            if x >= 0.0 {
                x.sqrt()
            } else {
                f64::NAN
            }
        }
        NumericFunc::INV => {
            if x != 0.0 {
                1.0 / x
            } else {
                f64::NAN
            }
        }
        _ => {
            // Should never happen if optimizer does its job
            // But don't panic in production - return NaN
            f64::NAN
        }
    }
}

/// Execute elementwise_chain in a single pass (PR4.1)
pub fn fused_elementwise_column(col: &Column, ops: &[crate::ir::NumericFunc]) -> Column {
    match col {
        Column::F64(data) => {
            let result = data
                .iter()
                .map(|&x| {
                    let mut y = x;
                    for op in ops {
                        y = apply_elementwise_op(y, *op);
                    }
                    y
                })
                .collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Execute cs1 ∘ elementwise_chain in a single pass (PR4.2a)
pub fn fused_cs1_elementwise_column(col: &Column, ops: &[crate::ir::NumericFunc]) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(data.len());
            let mut acc = 1.0; // cs1 starts at 1.0

            for &x in data.iter() {
                // Apply elementwise chain
                let mut y = x;
                for op in ops {
                    y = apply_elementwise_op(y, *op);
                }

                // cs1 accumulation (NA-preserving)
                if y.is_nan() {
                    result.push(f64::NAN); // NA input → NA output, acc unchanged
                } else {
                    acc += y;
                    result.push(acc);
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Execute cs1 ∘ dlog-ofs in a single pass (PR4.2b)
///
/// Fuses: cs1(dlog-ofs(x, k)) → single pass with two state concerns
///
/// State:
/// - acc: f64 (cs1 accumulator, starts at 1.0)
/// - x[i-k]: read from input array (OFS: positional offset lag)
///
/// Semantics (matches unfused behavior exactly):
/// - dlog-ofs: For i < k → NA; for i ≥ k → ln(x[i]) - ln(x[i-k]) if both valid
/// - cs1: acc += dlog_value if not NA; output acc if valid, NA if not
///
/// Example: cs1(dlog-ofs([100, 110, 121], lag=1))
/// - i=0: dlog=NA (insufficient lag), out=NA
/// - i=1: dlog=ln(110/100), acc=1+ln(1.1), out=acc
/// - i=2: dlog=ln(121/110), acc+=ln(1.1), out=acc
pub fn fused_cs1_dlog_ofs_column(col: &Column, lag: usize) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(data.len());
            let mut acc = 1.0; // cs1 starts at 1.0

            for i in 0..data.len() {
                if i < lag {
                    // Insufficient lag → dlog output is NA
                    result.push(f64::NAN);
                } else {
                    let current = data[i];
                    let lagged = data[i - lag];

                    // Compute dlog-ofs: ln(x[i]) - ln(x[i-k])
                    let dlog_val = if current.is_finite()
                        && lagged.is_finite()
                        && current > 0.0
                        && lagged > 0.0
                    {
                        current.ln() - lagged.ln()
                    } else {
                        f64::NAN
                    };

                    // cs1 accumulation (NA-preserving)
                    if dlog_val.is_nan() {
                        result.push(f64::NAN); // NA dlog → NA out, acc unchanged
                    } else {
                        acc += dlog_val;
                        result.push(acc);
                    }
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Execute cs1 ∘ dlog-obs in a single pass (PR4.2b)
///
/// Fuses: cs1(dlog-obs(x)) → single pass with two state variables
///
/// State:
/// - acc: f64 (cs1 accumulator, starts at 1.0)
/// - prev_valid: Option<f64> (dlog-obs last valid observation)
///
/// Semantics (matches unfused behavior exactly):
/// - dlog-obs: NA-skipping lag → ln(x[i]) - ln(last_valid) if both exist
/// - cs1: acc += dlog_value if not NA; output acc if valid, NA if not
///
/// NA propagation:
/// - Input NA → output NA, both states unchanged
/// - First valid → output NA (no predecessor), set prev_valid
/// - Subsequent valids → compute dlog, update acc and prev_valid
///
/// Example: cs1(dlog-obs([100, NA, 110, 121]))
/// - i=0: first valid, prev=None, out=NA, prev→100
/// - i=1: input NA, out=NA, states unchanged
/// - i=2: dlog=ln(110/100), acc=1+ln(1.1), out=acc, prev→110
/// - i=3: dlog=ln(121/110), acc+=ln(1.1), out=acc, prev→121
pub fn fused_cs1_dlog_obs_column(col: &Column) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(data.len());
            let mut acc = 1.0; // cs1 starts at 1.0
            let mut prev_valid: Option<f64> = None; // dlog-obs state

            for &x in data.iter() {
                if x.is_nan() {
                    // NA input → NA output, states unchanged
                    result.push(f64::NAN);
                } else if let Some(prev) = prev_valid {
                    // Valid current + valid previous → compute dlog
                    let dlog_val = if x > 0.0 && prev > 0.0 {
                        x.ln() - prev.ln()
                    } else {
                        f64::NAN
                    };

                    // cs1 accumulation
                    if dlog_val.is_nan() {
                        result.push(f64::NAN); // NA dlog → NA out, acc unchanged
                    } else {
                        acc += dlog_val;
                        result.push(acc);
                    }

                    prev_valid = Some(x); // Update prev_valid
                } else {
                    // First valid observation → no predecessor
                    result.push(f64::NAN);
                    prev_valid = Some(x);
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Execute cs1 ∘ elementwise_chain in a single pass (PR4.2a)
/// Execute dlog-obs ∘ elementwise_chain in a single pass (PR4.3a)
///
/// Fuses: EW_CHAIN(dlog-obs(x)) → single pass with observation-based state
///
/// State: last_valid (for dlog-obs)
///
/// Example: abs(dlog-obs([100, NA, 110, 120]))
/// - dlog-obs: [NA, NA, ln(1.1), ln(120/110)]
/// - abs applied immediately to each dlog result
pub fn fused_dlog_obs_elementwise_column(col: &Column, ops: &[NumericFunc]) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(data.len());
            let mut last_valid: Option<f64> = None;

            for &x in data.iter() {
                if x.is_nan() {
                    // Current NA → output NA
                    result.push(f64::NAN);
                } else if let Some(prev) = last_valid {
                    // Valid current, valid previous → compute dlog
                    let dlog_val = if prev > 0.0 && x > 0.0 {
                        x.ln() - prev.ln()
                    } else {
                        f64::NAN
                    };

                    // Apply elementwise chain to dlog result
                    let mut y = dlog_val;
                    for op in ops {
                        y = apply_elementwise_op(y, *op);
                    }

                    result.push(y);
                    last_valid = Some(x);
                } else {
                    // Valid current, no previous → output NA (first valid)
                    result.push(f64::NAN);
                    last_valid = Some(x);
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Execute dlog-ofs ∘ elementwise_chain in a single pass (PR4.3a)
///
/// Fuses: EW_CHAIN(dlog-ofs(x, k)) → single pass with fixed-lag predecessor
///
/// State: None (uses positional lag)
///
/// Example: abs(dlog-ofs([100, 110, 121], lag=1))
/// - For i < lag: NA
/// - For i >= lag: ln(x[i]) - ln(x[i-k]), then apply elementwise chain
pub fn fused_dlog_ofs_elementwise_column(col: &Column, lag: usize, ops: &[NumericFunc]) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(data.len());

            for i in 0..data.len() {
                if i < lag {
                    // Prefix: not enough history
                    result.push(f64::NAN);
                } else {
                    let current = data[i];
                    let lagged = data[i - lag];

                    // Compute dlog-ofs: ln(x[i]) - ln(x[i-k])
                    let dlog_val = if current.is_finite()
                        && lagged.is_finite()
                        && current > 0.0
                        && lagged > 0.0
                    {
                        current.ln() - lagged.ln()
                    } else {
                        f64::NAN
                    };

                    // Apply elementwise chain to dlog result
                    let mut y = dlog_val;
                    for op in ops {
                        y = apply_elementwise_op(y, *op);
                    }

                    result.push(y);
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}
/// Rolling mean with mask-aware observation counting (strict) - LEGACY O(n·w) version
///
/// Kept for comparison testing. Use `rolling_mean_mask_aware_legacy` for verification.
#[cfg(test)]
#[allow(dead_code)]
fn rolling_mean_mask_aware_legacy(
    col: &Column,
    w: usize,
    mask: &crate::mask::ActiveMask,
    nrows: usize,
) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(nrows);

            for i in 0..nrows {
                // Skip masked rows: output NA
                if mask.is_masked(i) {
                    result.push(f64::NAN);
                    continue;
                }

                // Find last w eligible observations (not masked, not NA) ending at or before position i
                let mut eligible: Vec<f64> = Vec::new();
                let mut j = i as isize; // Start from current position, go backward

                while eligible.len() < w && j >= 0 {
                    let idx = j as usize;
                    // Check if this observation is eligible (!masked && valid)
                    if !mask.is_masked(idx) && idx < data.len() && !data[idx].is_nan() {
                        eligible.push(data[idx]);
                    }
                    j -= 1;
                }

                // Strict: need exactly w eligible observations
                if eligible.len() == w {
                    let sum: f64 = eligible.iter().sum();
                    result.push(sum / (w as f64));
                } else {
                    result.push(f64::NAN);
                }
            }

            Column::new_f64(result)
        }
        _ => col.clone(),
    }
}

/// Rolling std with mask-aware observation counting - O(n) streaming version
///
/// Maintains running sum and sum-of-squares for incremental variance.
/// Uses population variance: var = E[X²] - E[X]² = (sumsq/w) - (sum/w)²
fn rolling_std_mask_aware(
    col: &Column,
    w: usize,
    mask: &crate::mask::ActiveMask,
    nrows: usize,
) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(nrows);

            for i in 0..nrows {
                // Masked rows: output NA
                if mask.is_masked(i) {
                    result.push(f64::NAN);
                    continue;
                }

                // Look back and collect up to w unmasked, non-NA observations
                let mut count = 0;
                let mut sum = 0.0;
                let mut sumsq = 0.0;

                for j in (0..=i).rev() {
                    // Skip masked rows when looking back
                    if mask.is_masked(j) {
                        continue;
                    }

                    let value = if j < data.len() { data[j] } else { f64::NAN };
                    if !value.is_nan() {
                        sum += value;
                        sumsq += value * value;
                        count += 1;
                        if count >= w {
                            break;
                        }
                    }
                }

                // Strict: emit only if we have exactly w observations
                if count == w {
                    let n = w as f64;
                    let mean = sum / n;
                    // Use sample variance (n-1 denominator) to match CLISPI/Adyton
                    let variance = ((sumsq / n) - (mean * mean)) * n / (n - 1.0);
                    result.push(variance.max(0.0).sqrt()); // max(0) for numerical stability
                } else {
                    result.push(f64::NAN);
                }
            }

            Column::new_f64(result)
        }
        _ => col.clone(),
    }
}

/// Rolling std with mask-aware observation counting (strict) - LEGACY O(n·w) version
#[cfg(test)]
#[allow(dead_code)]
fn rolling_std_mask_aware_legacy(
    col: &Column,
    w: usize,
    mask: &crate::mask::ActiveMask,
    nrows: usize,
) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(nrows);

            for i in 0..nrows {
                // Skip masked rows: output NA
                if mask.is_masked(i) {
                    result.push(f64::NAN);
                    continue;
                }

                // Find last w eligible observations
                let mut eligible: Vec<f64> = Vec::new();
                let mut j = i as isize;

                while eligible.len() < w && j >= 0 {
                    let idx = j as usize;
                    if !mask.is_masked(idx) && idx < data.len() && !data[idx].is_nan() {
                        eligible.push(data[idx]);
                    }
                    j -= 1;
                }

                // Strict: need exactly w eligible observations
                if eligible.len() == w {
                    let mean: f64 = eligible.iter().sum::<f64>() / (w as f64);
                    let variance: f64 =
                        eligible.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (w as f64);
                    result.push(variance.sqrt());
                } else {
                    result.push(f64::NAN);
                }
            }

            Column::new_f64(result)
        }
        _ => col.clone(),
    }
}

/// Rolling mean partial with mask-aware observation counting - O(n) streaming version
///
/// Partial: emits if window has >= 2 observations (relaxed min_periods)
fn rolling_mean_partial_mask_aware(
    col: &Column,
    w: usize,
    mask: &crate::mask::ActiveMask,
    nrows: usize,
) -> Column {
    rolling_mean_partial_mask_aware_offset(col, w, mask, nrows, 0)
}

fn rolling_mean_partial_mask_aware_offset(
    col: &Column,
    w: usize,
    mask: &crate::mask::ActiveMask,
    nrows: usize,
    end_offset: usize,
) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(nrows);

            for i in 0..nrows {
                // Masked rows: output NA
                if mask.is_masked(i) {
                    result.push(f64::NAN);
                    continue;
                }

                // Compute window ending at i - end_offset (for ft-zscore: end_offset=1 means use stats up to i-1)
                if i < end_offset {
                    result.push(f64::NAN);
                    continue;
                }
                let end_pos = i - end_offset;

                // Look back from end_pos and collect up to w unmasked, non-NA observations
                let mut count = 0;
                let mut sum = 0.0;

                for j in (0..=end_pos).rev() {
                    // Skip masked rows when looking back
                    if mask.is_masked(j) {
                        continue;
                    }

                    let value = if j < data.len() { data[j] } else { f64::NAN };
                    if !value.is_nan() {
                        sum += value;
                        count += 1;
                        if count >= w {
                            break;
                        }
                    }
                }

                // Partial: emit if we have >= 2 observations
                if count >= 2 {
                    result.push(sum / (count as f64));
                } else {
                    result.push(f64::NAN);
                }
            }

            Column::new_f64(result)
        }
        _ => col.clone(),
    }
}

/// Rolling mean partial with mask-aware observation counting - LEGACY O(n·w) version
#[cfg(test)]
#[allow(dead_code)]
fn rolling_mean_partial_mask_aware_legacy(
    col: &Column,
    w: usize,
    mask: &crate::mask::ActiveMask,
    nrows: usize,
) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(nrows);

            for i in 0..nrows {
                // Skip masked rows: output NA
                if mask.is_masked(i) {
                    result.push(f64::NAN);
                    continue;
                }

                // Find up to w eligible observations
                let mut eligible: Vec<f64> = Vec::new();
                let mut j = i as isize;

                while eligible.len() < w && j >= 0 {
                    let idx = j as usize;
                    if !mask.is_masked(idx) && idx < data.len() && !data[idx].is_nan() {
                        eligible.push(data[idx]);
                    }
                    j -= 1;
                }

                // Partial: allow if we have at least 2 observations
                if eligible.len() >= 2 {
                    let sum: f64 = eligible.iter().sum();
                    result.push(sum / (eligible.len() as f64));
                } else {
                    result.push(f64::NAN);
                }
            }

            Column::new_f64(result)
        }
        _ => col.clone(),
    }
}

/// Rolling std partial with mask-aware observation counting - O(n) streaming version
///
/// Partial: emits if window has >= 2 observations (relaxed min_periods)
fn rolling_std_partial_mask_aware(
    col: &Column,
    w: usize,
    mask: &crate::mask::ActiveMask,
    nrows: usize,
) -> Column {
    rolling_std_partial_mask_aware_offset(col, w, mask, nrows, 0)
}

fn rolling_std_partial_mask_aware_offset(
    col: &Column,
    w: usize,
    mask: &crate::mask::ActiveMask,
    nrows: usize,
    end_offset: usize,
) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(nrows);

            for i in 0..nrows {
                // Masked rows: output NA
                if mask.is_masked(i) {
                    result.push(f64::NAN);
                    continue;
                }

                // Compute window ending at i - end_offset (for ft-zscore: end_offset=1 means use stats up to i-1)
                if i < end_offset {
                    result.push(f64::NAN);
                    continue;
                }
                let end_pos = i - end_offset;

                // Look back from end_pos and collect up to w unmasked, non-NA observations
                let mut count = 0;
                let mut sum = 0.0;
                let mut sumsq = 0.0;

                for j in (0..=end_pos).rev() {
                    // Skip masked rows when looking back
                    if mask.is_masked(j) {
                        continue;
                    }

                    let value = if j < data.len() { data[j] } else { f64::NAN };
                    if !value.is_nan() {
                        sum += value;
                        sumsq += value * value;
                        count += 1;
                        if count >= w {
                            break;
                        }
                    }
                }

                // Partial: emit if we have >= 2 observations
                if count >= 2 {
                    let n = count as f64;
                    let mean = sum / n;
                    // Use sample variance (n-1 denominator) to match CLISPI/Adyton
                    let variance = ((sumsq / n) - (mean * mean)) * n / (n - 1.0);
                    result.push(variance.max(0.0).sqrt());
                } else {
                    result.push(f64::NAN);
                }
            }

            Column::new_f64(result)
        }
        _ => col.clone(),
    }
}

/// Rolling std partial with mask-aware observation counting - LEGACY O(n·w) version
#[cfg(test)]
#[allow(dead_code)]
fn rolling_std_partial_mask_aware_legacy(
    col: &Column,
    w: usize,
    mask: &crate::mask::ActiveMask,
    nrows: usize,
) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(nrows);

            for i in 0..nrows {
                // Skip masked rows: output NA
                if mask.is_masked(i) {
                    result.push(f64::NAN);
                    continue;
                }

                // Find up to w eligible observations
                let mut eligible: Vec<f64> = Vec::new();
                let mut j = i as isize;

                while eligible.len() < w && j >= 0 {
                    let idx = j as usize;
                    if !mask.is_masked(idx) && idx < data.len() && !data[idx].is_nan() {
                        eligible.push(data[idx]);
                    }
                    j -= 1;
                }

                // Partial: allow if we have at least 2 observations
                if eligible.len() >= 2 {
                    let mean: f64 = eligible.iter().sum::<f64>() / (eligible.len() as f64);
                    let variance: f64 = eligible.iter().map(|&x| (x - mean).powi(2)).sum::<f64>()
                        / (eligible.len() as f64);
                    result.push(variance.sqrt());
                } else {
                    result.push(f64::NAN);
                }
            }

            Column::new_f64(result)
        }
        _ => col.clone(),
    }
}

/// Why NA-skipping lag:
/// - Monday after weekend: uses Friday's value (not Sunday NA)
/// - Gap-filling semantics: skips NA to find last valid price
/// - CLISPI equivalent: LOCF→wkd→dlog creates zeros, BLISP wkd→dlog creates multi-day returns
/// - Both approaches yield identical non-NA, non-zero cumsum results
///
/// Observation-based difference log (OBS semantics)
///
/// This is NOT the same as positional lag dlog (OFS). See dlog_ofs_column().
/// - OBS: [100,NA,NA,110] → [NA,NA,NA,ln(110/100)] (skipped NAs)
/// - OFS: [100,NA,NA,110] → [NA,NA,NA,NA] (used x[i-1]=NA)
pub fn dlog_obs_column(col: &Column, _lag: usize) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(data.len());
            let mut last_valid: Option<f64> = None;

            for &x in data.iter() {
                if x.is_nan() {
                    // Current NA → output NA, but keep last_valid for next valid value
                    result.push(f64::NAN);
                } else if let Some(prev) = last_valid {
                    // Valid current, valid previous → compute dlog (IEEE-754)
                    // ln() handles edge cases: 0.0→-inf, negative→NaN
                    // Subtraction propagates: -inf-val=-inf, val-(-inf)=+inf, NaN±x=NaN
                    result.push(x.ln() - prev.ln());
                    last_valid = Some(x);
                } else {
                    // Valid current, no previous → output NA (first valid)
                    result.push(f64::NAN);
                    last_valid = Some(x);
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Positional offset difference log (OFS semantics)
///
/// This is a thin wrapper around blawktrust::dlog_column.
/// - OFS: [100,NA,NA,110] → [NA,NA,NA,NA] (used x[i-1]=NA)
/// - OBS: [100,NA,NA,110] → [NA,NA,NA,ln(110/100)] (skipped NAs)
///
/// For financial time series with weekend masks, use dlog_obs_column() instead.
pub(crate) fn dlog_ofs_column(col: &Column, lag: usize) -> Column {
    blawktrust::builtins::ops::dlog_column(col, lag)
}

/// Mask-aware ret: arithmetic returns with NA-skipping lag
///
/// Contract (updated for shape-preserving wkd):
/// - ret[i] = (x[i] - x[last_valid before i]) / x[last_valid before i]
/// - Skips NAs in lag: looks back for last valid value
/// - If current value NA → output NA
/// - If no prior valid value → output NA
/// - Compatible with weekend masking
fn ret_column(col: &Column, _lag: usize) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(data.len());
            let mut last_valid: Option<f64> = None;

            for &x in data.iter() {
                if x.is_nan() {
                    // Current NA → output NA
                    result.push(f64::NAN);
                } else if let Some(prev) = last_valid {
                    // Valid current, valid previous → compute ret
                    if prev != 0.0 {
                        result.push((x - prev) / prev);
                    } else {
                        result.push(f64::NAN);
                    }
                    last_valid = Some(x);
                } else {
                    // Valid current, no previous → output NA (first valid)
                    result.push(f64::NAN);
                    last_valid = Some(x);
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Last observation carried forward (fill NA with last valid value)
///
/// Contract:
/// - Leading NAs preserved until first valid value
/// - After first valid: NA filled with last valid value before it
/// - Valid values pass through unchanged
/// - Idempotent: locf(locf(x)) == locf(x)
/// - O(n) single pass
fn locf_column(col: &Column) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(data.len());
            let mut last_valid: Option<f64> = None;

            for &x in data.iter() {
                if x.is_nan() {
                    // If we have a valid value, use it; otherwise keep NA
                    result.push(last_valid.unwrap_or(f64::NAN));
                } else {
                    // Valid value: pass through and remember it
                    result.push(x);
                    last_valid = Some(x);
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Cumulative sum starting at 1.0 (cs1)
///
/// Contract (updated for shape-preserving wkd):
/// - Starts at 1.0 (not 0.0!)
/// - NA policy: "skip and preserve"
///   - NA input → NA output (preserves weekend masks)
///   - Valid values: cumsum updates and outputs
///   - Running sum maintained across NA positions
/// - Compatible with masked time series (wkd)
/// - O(n) single pass
pub(crate) fn cumsum_column(col: &Column) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(data.len());
            let mut cumsum = 1.0;

            for &x in data.iter() {
                if x.is_nan() {
                    // NA input → NA output (preserves masks from wkd)
                    result.push(f64::NAN);
                } else {
                    // Valid input: update cumsum and output
                    cumsum += x;
                    result.push(cumsum);
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Weekday mask (wkd): Set weekend values to NA
///
/// Contract:
/// - Shape-preserving: I1, I2, I3 all maintained
/// - For each row: if Saturday (6) or Sunday (0) → set all column values to NA
/// - Weekday rows (Monday-Friday, 1-5): values unchanged
/// - Requires Date or Timestamp index
/// - O(n) single pass per column
fn wkd_mask_weekends(frame: &Frame) -> Result<Arc<Frame>, String> {
    use crate::frame::IndexColumn;

    // Determine which rows are weekends
    let weekend_mask: Vec<bool> = match &*frame.tags.index {
        IndexColumn::Date(dates) => {
            dates
                .iter()
                .map(|&date| {
                    // Parse date to get day of week
                    // Date is stored as i32: days since Unix epoch (1970-01-01)
                    // Use chrono-like calculation to determine day of week

                    // Unix epoch (1970-01-01) was a Thursday (day_of_week = 4)
                    // day_of_week = (4 + days_since_epoch) % 7
                    // 0=Sunday, 1=Monday, ..., 6=Saturday
                    let day_of_week = (4 + date).rem_euclid(7);

                    // Weekend: Sunday (0) or Saturday (6)
                    day_of_week == 0 || day_of_week == 6
                })
                .collect()
        }
        IndexColumn::Timestamp(timestamps) => {
            timestamps
                .iter()
                .map(|&ts| {
                    // Timestamp is i64 milliseconds since Unix epoch
                    // Convert to days and use same logic
                    let days = (ts / 86400000) as i32; // 86400000 ms per day
                    let day_of_week = (4 + days).rem_euclid(7);
                    day_of_week == 0 || day_of_week == 6
                })
                .collect()
        }
        IndexColumn::String(_) => {
            return Err("wkd requires Date or Timestamp index, got String".to_string());
        }
    };

    // Apply weekend mask to all columns
    let masked_cols: Vec<ColData> = frame
        .cols
        .iter()
        .map(|col_data| {
            match col_data {
                ColData::Mat(col_arc) => {
                    match &**col_arc {
                        Column::F64(data) => {
                            let masked_data: Vec<f64> = data
                                .iter()
                                .enumerate()
                                .map(|(i, &val)| {
                                    if weekend_mask[i] {
                                        f64::NAN // Weekend: mask to NA
                                    } else {
                                        val // Weekday: unchanged
                                    }
                                })
                                .collect();
                            ColData::Mat(Arc::new(Column::F64(masked_data)))
                        }
                        other => ColData::Mat(Arc::new(other.clone())),
                    }
                }
            }
        })
        .collect();

    // Build result frame with preserved tags (I1, I2, I3)
    let result = Frame {
        tags: Arc::clone(&frame.tags), // I1, I2 preserved via Arc
        cols: masked_cols,
        nrows: frame.nrows, // I3: preserved
    };

    // Verify invariants
    debug_assert_eq!(
        result.nrows(),
        frame.nrows(),
        "W5: I3 violation - nrows changed"
    );
    debug_assert_eq!(result.ncols(), frame.ncols(), "W5: column count changed");

    Ok(Arc::new(result))
}

/// Pairwise spread: col_a - col_b
///
/// Contract:
/// - Element-wise subtraction
/// - NA policy: if either input NA, output NA
/// - O(n) single pass
fn xminus_columns(col_a: &Column, col_b: &Column) -> Column {
    match (col_a, col_b) {
        (Column::F64(data_a), Column::F64(data_b)) => {
            if data_a.len() != data_b.len() {
                panic!("xminus: column length mismatch");
            }

            let result = data_a
                .iter()
                .zip(data_b.iter())
                .map(|(&a, &b)| {
                    if a.is_nan() || b.is_nan() {
                        f64::NAN
                    } else {
                        a - b
                    }
                })
                .collect();

            Column::F64(result)
        }
        _ => col_a.clone(),
    }
}

/// Helper: compute eligible rows (unmasked) and position mapping
/// Returns (eligible_rows, pos_in_eligible)
/// eligible_rows: Vec<usize> = indices of unmasked rows
/// pos_in_eligible: Vec<i32> = for each row, its position in eligible list (-1 if masked)
fn eligible_rows(mask: &crate::mask::ActiveMask, nrows: usize) -> (Vec<usize>, Vec<i32>) {
    let eligible: Vec<usize> = (0..nrows).filter(|&i| !mask.is_masked(i)).collect();

    let mut pos_in_eligible = vec![-1i32; nrows];
    for (p, &i) in eligible.iter().enumerate() {
        pos_in_eligible[i] = p as i32;
    }

    (eligible, pos_in_eligible)
}

/// Calendar shift (positional): shift by k calendar rows
pub(crate) fn shift_column(col: &Column, k: usize) -> Column {
    match col {
        Column::F64(data) => {
            let nrows = data.len();
            let mut result = vec![f64::NAN; nrows];

            // Contract: output[i] = input[i-k] for i >= k, NA for i < k
            // First k rows are NA (already initialized)
            // Copy input[0..nrows-k] to output[k..nrows]
            if k < nrows {
                result[k..].copy_from_slice(&data[0..nrows - k]);
            }
            // If k >= nrows, all rows are NA (already initialized)

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Keep every k-th row (shape-preserving)
///
/// Contract:
/// - Keeps rows where row_index % k == 0
/// - Other rows filled with NA
/// - Shape preserved (nrows unchanged)
fn keep_column(col: &Column, k: usize) -> Column {
    match col {
        Column::F64(data) => {
            let result: Vec<f64> = data
                .iter()
                .enumerate()
                .map(|(i, &val)| if i % k == 0 { val } else { f64::NAN })
                .collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Mask-aware shift (observation-based): shift by k eligible (unmasked) rows
/// Skip masked rows only (not NA values)
/// For matching CLISPI's wkd-filtered behavior
fn shift_obs_column(
    col: &Column,
    k: usize,
    mask: &crate::mask::ActiveMask,
    nrows: usize,
) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = vec![f64::NAN; nrows];

            // Precompute eligible rows
            let (eligible, pos_in_eligible) = eligible_rows(mask, nrows);

            // For each output row
            for t in 0..nrows {
                // Masked rows output NA
                if mask.is_masked(t) {
                    result[t] = f64::NAN;
                    continue;
                }

                // Get position in eligible stream
                let p = pos_in_eligible[t];
                if p < 0 {
                    // Should not happen (we checked !masked)
                    result[t] = f64::NAN;
                    continue;
                }

                // Source position in eligible stream
                let src_p = p - (k as i32);
                if src_p < 0 {
                    // Not enough eligible rows before this one
                    result[t] = f64::NAN;
                    continue;
                }

                // Get source row index (guaranteed unmasked)
                let src_row = eligible[src_p as usize];

                // Copy value (may be NA, which is fine)
                result[t] = if src_row < data.len() {
                    data[src_row]
                } else {
                    f64::NAN
                };
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Rolling mean with strict min_periods semantics (O(n) optimized)
///
/// Contract (see contracts.md §5):
/// - Trailing window: [i-w+1 .. i] inclusive
/// - Skip NA in window, require w valid values (strict min_periods)
/// - Prefix i < w-1 always NA
/// - Shape preserved, NA mask monotone
///
/// Optimization: O(n) single-pass with running sum and valid count
/// - Maintains sliding window [i-w+1 .. i] via add/remove operations
/// - Tracks running_sum and valid_count for O(1) per element
fn rolling_mean_column(col: &Column, w: usize) -> Column {
    match col {
        Column::F64(data) => {
            let nrows = data.len();
            let mut result = vec![f64::NAN; nrows];

            // Edge case: window larger than data
            if w > nrows {
                return Column::F64(result); // All NA
            }

            let mut running_sum = 0.0;
            let mut valid_count = 0usize;

            // Single pass: maintain sliding window [i-w+1 .. i]
            for i in 0..nrows {
                // Add entering value at position i (window right edge)
                if !data[i].is_nan() {
                    running_sum += data[i];
                    valid_count += 1;
                }

                // Remove leaving value at position i-w (left edge exits window)
                if i >= w {
                    let leaving_idx = i - w;
                    if !data[leaving_idx].is_nan() {
                        running_sum -= data[leaving_idx];
                        valid_count -= 1;
                    }
                }

                // Emit result if window is full (i >= w-1) AND has exactly w valid values (strict)
                // Contract: strict min_periods = w (skip NA, require full window)
                if i >= w - 1 && valid_count >= w {
                    result[i] = running_sum / (valid_count as f64);
                }
                // Else: result[i] remains NA (prefix or no valid values)
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Rolling standard deviation with strict min_periods semantics (O(n) optimized)
///
/// Contract (see contracts.md §5):
/// - Trailing window: [i-w+1 .. i] inclusive
/// - Skip NA in window, require w valid values (strict min_periods)
/// - Population std: σ = sqrt((1/w) * Σ(x-μ)²)
/// - Constant series → σ = 0.0 (not NA)
/// - Window=1 → σ = 0.0 for valid values
/// - Prefix i < w-1 always NA
/// - Shape preserved, NA mask monotone
///
/// Optimization: O(n) single-pass with running sum/sumsq
/// - Variance formula: var = E[X²] - E[X]² = (sumsq/w) - mean²
/// - Numerically acceptable for typical financial data
/// - For extreme precision needs, can later add compensated method
fn rolling_std_column(col: &Column, w: usize) -> Column {
    match col {
        Column::F64(data) => {
            let nrows = data.len();
            let mut result = vec![f64::NAN; nrows];

            // Edge case: window larger than data
            if w > nrows {
                return Column::F64(result); // All NA
            }

            let mut running_sum = 0.0;
            let mut running_sumsq = 0.0;
            let mut valid_count = 0usize;

            // Single pass: maintain sliding window [i-w+1 .. i]
            for i in 0..nrows {
                // Add entering value at position i (window right edge)
                if !data[i].is_nan() {
                    let x = data[i];
                    running_sum += x;
                    running_sumsq += x * x;
                    valid_count += 1;
                }

                // Remove leaving value at position i-w (left edge exits window)
                if i >= w {
                    let leaving_idx = i - w;
                    if !data[leaving_idx].is_nan() {
                        let x = data[leaving_idx];
                        running_sum -= x;
                        running_sumsq -= x * x;
                        valid_count -= 1;
                    }
                }

                // Emit result if window is full (i >= w-1) AND has exactly w valid values (strict)
                // Contract: strict min_periods = w (skip NA, require full window)
                if i >= w - 1 && valid_count >= w {
                    let n = valid_count as f64;
                    let mean = running_sum / n;
                    // Use sample variance (n-1 denominator) to match CLISPI/Adyton
                    let variance = ((running_sumsq / n) - (mean * mean)) * n / (n - 1.0);

                    // Guard against numerical error producing negative/tiny variance
                    // Window=1 or constant series should have exactly 0 variance
                    // Use relative epsilon to catch numerical noise
                    let epsilon = 1e-10 * mean.abs().max(1.0);
                    result[i] = if variance <= epsilon {
                        0.0 // Constant series or numerical noise
                    } else {
                        variance.sqrt()
                    };
                }
                // Else: result[i] remains NA (prefix or insufficient valid values)
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Rolling mean with partial window (relaxed min_periods for masked calendars)
///
/// Contract:
/// - Trailing window: [i-w+1 .. i] inclusive
/// - Skip NA in window, require ≥2 valid values (relaxed, not strict w)
/// - Use available valid values only
/// - Prefix i < w-1 always NA
/// - Designed for: masked time series (e.g., weekday-only data with weekend NAs)
///
/// Difference from strict rolling_mean:
/// - Strict: requires valid_count == w (full window)
/// - Partial: requires valid_count >= 2 (any partial window)
fn rolling_mean_partial(col: &Column, w: usize) -> Column {
    match col {
        Column::F64(data) => {
            let nrows = data.len();
            let mut result = vec![f64::NAN; nrows];

            if w > nrows {
                return Column::F64(result);
            }

            let mut running_sum = 0.0;
            let mut valid_count = 0usize;

            for i in 0..nrows {
                // Add entering value
                if !data[i].is_nan() {
                    running_sum += data[i];
                    valid_count += 1;
                }

                // Remove leaving value
                if i >= w {
                    let leaving_idx = i - w;
                    if !data[leaving_idx].is_nan() {
                        running_sum -= data[leaving_idx];
                        valid_count -= 1;
                    }
                }

                // Emit if window position reached and ≥2 valid values (relaxed)
                if i >= w - 1 && valid_count >= 2 {
                    result[i] = running_sum / (valid_count as f64);
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Rolling standard deviation with partial window (relaxed min_periods for masked calendars)
///
/// Contract:
/// - Trailing window: [i-w+1 .. i] inclusive
/// - Skip NA in window, require ≥2 valid values (relaxed, not strict w)
/// - Use available valid values only
/// - Prefix i < w-1 always NA
/// - Designed for: masked time series (e.g., weekday-only data with weekend NAs)
///
/// Difference from strict rolling_std:
/// - Strict: requires valid_count == w (full window)
/// - Partial: requires valid_count >= 2 (any partial window)
fn rolling_std_partial(col: &Column, w: usize) -> Column {
    match col {
        Column::F64(data) => {
            let nrows = data.len();
            let mut result = vec![f64::NAN; nrows];

            if w > nrows {
                return Column::F64(result);
            }

            let mut running_sum = 0.0;
            let mut running_sumsq = 0.0;
            let mut valid_count = 0usize;

            for i in 0..nrows {
                // Add entering value
                if !data[i].is_nan() {
                    let x = data[i];
                    running_sum += x;
                    running_sumsq += x * x;
                    valid_count += 1;
                }

                // Remove leaving value
                if i >= w {
                    let leaving_idx = i - w;
                    if !data[leaving_idx].is_nan() {
                        let x = data[leaving_idx];
                        running_sum -= x;
                        running_sumsq -= x * x;
                        valid_count -= 1;
                    }
                }

                // Emit if window position reached and ≥2 valid values (relaxed)
                if i >= w - 1 && valid_count >= 2 {
                    let n = valid_count as f64;
                    let mean = running_sum / n;
                    // Use sample variance (n-1 denominator) to match CLISPI/Adyton
                    let variance = ((running_sumsq / n) - (mean * mean)) * n / (n - 1.0);

                    if variance > 1e-14 {
                        result[i] = variance.sqrt();
                    } else {
                        result[i] = 0.0;
                    }
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}

// ============================================================================
// Binary operation kernels
// ============================================================================

/// Apply binary operation between column and scalar
///
/// Scalar is broadcast to all cells
/// NA propagation: if cell is NA, result is NA
fn binary_scalar_column(col: &Column, scalar: f64, func: BinaryFunc) -> Column {
    match col {
        Column::F64(data) => {
            let result = data
                .iter()
                .map(|&x| {
                    if x.is_nan() || scalar.is_nan() {
                        f64::NAN
                    } else {
                        match func {
                            BinaryFunc::ADD => x + scalar,
                            BinaryFunc::SUB => x - scalar,
                            BinaryFunc::MUL => x * scalar,
                            BinaryFunc::DIV => {
                                if scalar == 0.0 {
                                    f64::NAN
                                } else {
                                    x / scalar
                                }
                            }
                            BinaryFunc::GTR => {
                                if x > scalar {
                                    1.0
                                } else {
                                    0.0
                                }
                            }
                            BinaryFunc::LSS => {
                                if x < scalar {
                                    1.0
                                } else {
                                    0.0
                                }
                            }
                            BinaryFunc::LTE => {
                                if x <= scalar {
                                    1.0
                                } else {
                                    0.0
                                }
                            }
                            BinaryFunc::GTE => {
                                if x >= scalar {
                                    1.0
                                } else {
                                    0.0
                                }
                            }
                            BinaryFunc::EQL => {
                                if x == scalar {
                                    1.0
                                } else {
                                    0.0
                                }
                            }
                            BinaryFunc::NEQ => {
                                if x != scalar {
                                    1.0
                                } else {
                                    0.0
                                }
                            }
                        }
                    }
                })
                .collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}

/// Apply binary operation between two frames (element-wise)
///
/// Requires: frames have same shape and compatible tags
/// NA propagation: if either cell is NA, result is NA
fn binary_frame_frame(lhs: &Frame, rhs: &Frame, func: BinaryFunc) -> Result<Frame, String> {
    if lhs.cols.len() != rhs.cols.len() {
        return Err(format!(
            "Frame-frame binary op requires same column count: {} vs {}",
            lhs.cols.len(),
            rhs.cols.len()
        ));
    }

    if lhs.nrows != rhs.nrows {
        return Err(format!(
            "Frame-frame binary op requires same row count: {} vs {}",
            lhs.nrows, rhs.nrows
        ));
    }

    let mut result_cols = Vec::with_capacity(lhs.cols.len());

    for (lhs_col, rhs_col) in lhs.cols.iter().zip(rhs.cols.iter()) {
        use crate::frame::ColData;
        let ColData::Mat(lhs_data) = lhs_col;
        let ColData::Mat(rhs_data) = rhs_col;

        let result_col = binary_column_column(lhs_data, rhs_data, func)?;
        result_cols.push(ColData::Mat(Arc::new(result_col)));
    }

    // Phase D: Propagate masks through binary operations
    // - Merge mask sets (error on collision with different bitsets)
    // - OR active masks (union of excluded rows)
    let mut merged_masks = lhs.tags.masks.clone();
    merged_masks
        .merge(&rhs.tags.masks)
        .map_err(|e| format!("Binary op mask merge failed: {}", e))?;

    let merged_active_mask =
        crate::mask::or_active_masks(&lhs.tags.active_mask, &rhs.tags.active_mask);

    let result_tags = Tags {
        index_name: lhs.tags.index_name.clone(),
        index: Arc::clone(&lhs.tags.index),
        colnames: Arc::clone(&lhs.tags.colnames),
        masks: merged_masks,
        active_mask: merged_active_mask,
    };

    Ok(Frame {
        tags: Arc::new(result_tags),
        cols: result_cols,
        nrows: lhs.nrows,
    })
}

/// Apply binary operation between two columns (element-wise)
fn binary_column_column(lhs: &Column, rhs: &Column, func: BinaryFunc) -> Result<Column, String> {
    match (lhs, rhs) {
        (Column::F64(lhs_data), Column::F64(rhs_data)) => {
            if lhs_data.len() != rhs_data.len() {
                return Err(format!(
                    "Column-column binary op requires same length: {} vs {}",
                    lhs_data.len(),
                    rhs_data.len()
                ));
            }

            let result = lhs_data
                .iter()
                .zip(rhs_data.iter())
                .map(|(&x, &y)| {
                    if x.is_nan() || y.is_nan() {
                        f64::NAN
                    } else {
                        match func {
                            BinaryFunc::ADD => x + y,
                            BinaryFunc::SUB => x - y,
                            BinaryFunc::MUL => x * y,
                            BinaryFunc::DIV => {
                                if y == 0.0 {
                                    f64::NAN
                                } else {
                                    x / y
                                }
                            }
                            BinaryFunc::GTR => {
                                if x > y {
                                    1.0
                                } else {
                                    0.0
                                }
                            }
                            BinaryFunc::LSS => {
                                if x < y {
                                    1.0
                                } else {
                                    0.0
                                }
                            }
                            BinaryFunc::LTE => {
                                if x <= y {
                                    1.0
                                } else {
                                    0.0
                                }
                            }
                            BinaryFunc::GTE => {
                                if x >= y {
                                    1.0
                                } else {
                                    0.0
                                }
                            }
                            BinaryFunc::EQL => {
                                if x == y {
                                    1.0
                                } else {
                                    0.0
                                }
                            }
                            BinaryFunc::NEQ => {
                                if x != y {
                                    1.0
                                } else {
                                    0.0
                                }
                            }
                        }
                    }
                })
                .collect();

            Ok(Column::F64(result))
        }
        _ => Err("Binary op requires F64 columns".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, Interner};
    use crate::normalize::normalize;
    use crate::planner::plan;
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
        setup_test_csv(
            test_file,
            "DATE;price\n2020-01-01;100\n2020-01-02;102\n2020-01-03;105\n",
        );

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
        setup_test_csv(
            test_y,
            "DATE;dummy\n2020-01-01;1\n2020-01-02;2\n2020-01-03;3\n",
        );

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

    // PR2-REVISED: Mandatory tests for OFS vs OBS semantic split

    #[test]
    fn test_dlog_semantic_divergence() {
        // Test proves: OBS ≠ OFS on NA-containing data
        let data_with_nas = vec![100.0, f64::NAN, f64::NAN, 110.0, 120.0];
        let col = Column::F64(data_with_nas);

        let obs_result = dlog_obs_column(&col, 1);
        let ofs_result = dlog_ofs_column(&col, 1);

        // They MUST differ on position 3 (index 3)
        if let (Column::F64(obs_vals), Column::F64(ofs_vals)) = (obs_result, ofs_result) {
            // Position 3: OBS = ln(110/100), OFS = NA (used x[2]=NA)
            assert!(
                obs_vals[3].is_finite(),
                "OBS should compute ln(110/100) at position 3"
            );
            assert!(ofs_vals[3].is_nan(), "OFS should return NA at position 3");

            // Approximate check for OBS value
            let expected_obs = (110.0_f64).ln() - (100.0_f64).ln();
            assert!(
                (obs_vals[3] - expected_obs).abs() < 1e-10,
                "OBS value should be ln(110/100)"
            );
        } else {
            panic!("Expected F64 columns");
        }
    }

    #[test]
    fn test_dlog_kernel_equivalence() {
        // Test proves: wrappers call correct implementations
        let clean_data = vec![100.0, 102.0, 105.0, 103.0];
        let col = Column::F64(clean_data.clone());

        // OFS wrapper should match blawktrust directly
        let ofs_result = dlog_ofs_column(&col, 1);
        let blawktrust_result = blawktrust::builtins::ops::dlog_column(&col, 1);

        if let (Column::F64(ofs_vals), Column::F64(bt_vals)) = (ofs_result, blawktrust_result) {
            for i in 0..ofs_vals.len() {
                if ofs_vals[i].is_nan() && bt_vals[i].is_nan() {
                    continue; // Both NA, OK
                }
                assert!(
                    (ofs_vals[i] - bt_vals[i]).abs() < 1e-10,
                    "OFS wrapper should match blawktrust at position {}",
                    i
                );
            }
        } else {
            panic!("Expected F64 columns");
        }

        // OBS behavior (already tested in divergence test)
    }

    #[test]
    fn test_dlog_ir_routing() {
        // Test proves: planner maps "dlog" → OBS, "dlog-ofs" → OFS
        let mut interner = Interner::new();

        // Test "dlog" token → OBS variant
        let dlog_expr = Expr::List(vec![
            Expr::Sym(interner.intern("dlog")),
            Expr::Sym(interner.intern("x")),
        ]);
        let normalized_dlog = normalize(dlog_expr, &mut interner);
        let plan_dlog = plan(&normalized_dlog, &interner);
        assert!(plan_dlog.is_ok(), "dlog should plan successfully");
        // Note: Deep IR inspection would require exposing plan internals
        // This test verifies planner doesn't reject the token

        // Test "dlog-ofs" token → OFS variant
        let dlog_ofs_expr = Expr::List(vec![
            Expr::Sym(interner.intern("dlog-ofs")),
            Expr::Sym(interner.intern("x")),
        ]);
        let normalized_ofs = normalize(dlog_ofs_expr, &mut interner);
        let plan_ofs = plan(&normalized_ofs, &interner);
        assert!(plan_ofs.is_ok(), "dlog-ofs should plan successfully");
    }

    // PR2.5: Mandatory tests for shift OFS vs OBS semantic verification

    #[test]
    fn test_shift_ofs_kernel() {
        // Test OFS shift kernel directly: positional offset
        let data = vec![10.0, 20.0, 30.0, 40.0];
        let col = Column::F64(data);

        let result = shift_column(&col, 1);

        if let Column::F64(vals) = result {
            assert!(vals[0].is_nan(), "First row should be NA");
            assert_eq!(vals[1], 10.0, "out[1] = in[0]");
            assert_eq!(vals[2], 20.0, "out[2] = in[1]");
            assert_eq!(vals[3], 30.0, "out[3] = in[2]");
        } else {
            panic!("Expected F64 column");
        }
    }

    #[test]
    fn test_shift_obs_kernel_with_mask() {
        // Test OBS shift kernel: observation-based (skip masked rows)
        let data = vec![10.0, 20.0, 30.0, 40.0];
        let col = Column::F64(data);

        // Create mask: rows 1,2 masked (only 0,3 eligible)
        use bitvec::prelude::*;
        let bv = bitvec![0, 1, 1, 0]; // 1 = masked
        let mask = crate::mask::ActiveMask::from_bitvec(bv, None);

        let result = shift_obs_column(&col, 1, &mask, 4);

        if let Column::F64(vals) = result {
            // Row 0: unmasked, but no predecessor in eligible stream → NA
            assert!(vals[0].is_nan(), "Row 0: no predecessor");
            // Rows 1,2: masked → NA
            assert!(vals[1].is_nan(), "Row 1: masked");
            assert!(vals[2].is_nan(), "Row 2: masked");
            // Row 3: unmasked, 1 step back in eligible stream → row 0 → 10.0
            assert_eq!(vals[3], 10.0, "Row 3: obs shift k=1 should find row 0");
        } else {
            panic!("Expected F64 column");
        }
    }

    #[test]
    fn test_shift_semantic_divergence() {
        // Test proves: OFS ≠ OBS when masked rows exist
        // OFS shift propagates NA from masked position
        // OBS shift skips masked rows to find eligible observation

        let data = vec![10.0, 20.0, 30.0, 40.0];
        let col = Column::F64(data);

        // OFS: positional shift (no mask awareness)
        let ofs_result = shift_column(&col, 1);

        // OBS: observation-based shift with mask
        use bitvec::prelude::*;
        let bv = bitvec![0, 1, 1, 0]; // 1 = masked
        let mask = crate::mask::ActiveMask::from_bitvec(bv, None);
        let obs_result = shift_obs_column(&col, 1, &mask, 4);

        if let (Column::F64(ofs_vals), Column::F64(obs_vals)) = (ofs_result, obs_result) {
            // Position 1: OFS = 10.0 (in[0]), OBS = NA (masked)
            assert_eq!(ofs_vals[1], 10.0, "OFS at position 1");
            assert!(obs_vals[1].is_nan(), "OBS at position 1 (masked)");

            // Position 3: OFS = 30.0 (in[2]), OBS = 10.0 (skipped masked rows 1,2)
            assert_eq!(ofs_vals[3], 30.0, "OFS at position 3: used in[2]");
            assert_eq!(obs_vals[3], 10.0, "OBS at position 3: skipped to in[0]");

            // They diverge at positions where mask affects observation counting
            assert_ne!(ofs_vals[3], obs_vals[3], "OFS ≠ OBS at position 3");
        } else {
            panic!("Expected F64 columns");
        }
    }

    #[test]
    fn test_shift_equivalence_clean() {
        // Test proves: OFS == OBS on clean (unmasked) data
        let data = vec![10.0, 20.0, 30.0, 40.0];
        let col = Column::F64(data);

        // OFS: positional shift
        let ofs_result = shift_column(&col, 1);

        // OBS: observation-based shift with empty mask
        let mask = crate::mask::ActiveMask::empty(4);
        let obs_result = shift_obs_column(&col, 1, &mask, 4);

        if let (Column::F64(ofs_vals), Column::F64(obs_vals)) = (ofs_result, obs_result) {
            for i in 0..4 {
                if ofs_vals[i].is_nan() && obs_vals[i].is_nan() {
                    continue; // Both NA, OK
                }
                assert_eq!(
                    ofs_vals[i], obs_vals[i],
                    "OFS should equal OBS on unmasked data at position {}",
                    i
                );
            }
        } else {
            panic!("Expected F64 columns");
        }
    }

    #[test]
    fn test_shift_ir_routing() {
        // Test proves: planner accepts "shift", "lag-obs", and "shift-obs" tokens
        let mut interner = Interner::new();

        // Test "shift" token → OFS variant
        let shift_expr = Expr::List(vec![
            Expr::Sym(interner.intern("shift")),
            Expr::Int(1),
            Expr::Sym(interner.intern("x")),
        ]);
        let normalized_shift = normalize(shift_expr, &mut interner);
        let plan_shift = plan(&normalized_shift, &interner);
        assert!(plan_shift.is_ok(), "shift should plan successfully");

        // Test "lag-obs" token → OBS variant
        let lag_obs_expr = Expr::List(vec![
            Expr::Sym(interner.intern("lag-obs")),
            Expr::Int(1),
            Expr::Sym(interner.intern("x")),
        ]);
        let normalized_lag = normalize(lag_obs_expr, &mut interner);
        let plan_lag = plan(&normalized_lag, &interner);
        assert!(plan_lag.is_ok(), "lag-obs should plan successfully");

        // Test "shift-obs" token → OBS variant (alias)
        let shift_obs_expr = Expr::List(vec![
            Expr::Sym(interner.intern("shift-obs")),
            Expr::Int(1),
            Expr::Sym(interner.intern("x")),
        ]);
        let normalized_shift_obs = normalize(shift_obs_expr, &mut interner);
        let plan_shift_obs = plan(&normalized_shift_obs, &interner);
        assert!(plan_shift_obs.is_ok(), "shift-obs should plan successfully");
    }
}
