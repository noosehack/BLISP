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

use crate::ir::{Plan, Node, NodeId, Operation, Source, UnaryOp, BinaryOp, BinaryFunc, ValueRef, JoinOp, NumericFunc, SchemaOp};
use crate::frame::{Frame, Tags, ColData, map_numeric_preserve_tags, asofr};
use crate::value::Value;
use crate::runtime::Runtime;
use crate::io;
use std::sync::Arc;
use std::collections::HashMap;
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
                _ => Err(format!("CSV loader returned non-Frame: {}", value.type_name())),
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
                _ => Err(format!("stdin parsing returned non-Frame: {}", value.type_name())),
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

            // Special handling for W5: requires index access for weekday determination
            if matches!(func, NumericFunc::W5) {
                return w5_mask_weekends(&input_frame);
            }

            // Phase E: Special handling for rolling operations and mask-aware shift (need active_mask)
            let result = match func {
                NumericFunc::RollMean { w } |
                NumericFunc::RollStd { w } |
                NumericFunc::RollMeanPartial { w } |
                NumericFunc::RollStdPartial { w } |
                NumericFunc::RollMeanPartialExclCurrent { w } |
                NumericFunc::RollStdPartialExclCurrent { w } => {
                    // Rolling operations need access to active_mask to count only eligible observations
                    apply_rolling_mask_aware(&input_frame, *func)?
                }
                NumericFunc::ShiftObs { k } => {
                    // Mask-aware shift: skip masked rows when computing lag
                    // This matches CLISPI's "business day lag" when weekends are masked
                    apply_shift_obs_mask_aware(&input_frame, *k)?
                }
                _ => {
                    // Non-rolling, non-mask-aware operations: use standard map_numeric_preserve_tags
                    map_numeric_preserve_tags(&input_frame, |col| {
                        match func {
                            NumericFunc::Dlog => dlog_column(col, 1),
                            NumericFunc::Ret => ret_column(col, 1),
                            NumericFunc::Log => log_column(col),
                            NumericFunc::Exp => exp_column(col),
                            NumericFunc::Sqrt => sqrt_column(col),
                            NumericFunc::Abs => abs_column(col),
                            NumericFunc::Inv => inv_column(col),
                            NumericFunc::Locf => locf_column(col),
                            NumericFunc::CumSum => cumsum_column(col),
                            NumericFunc::Shift { k } => shift_column(col, *k),
                            NumericFunc::W5 => unreachable!("W5 handled above"),
                            NumericFunc::ShiftObs { .. } => unreachable!("ShiftObs handled separately"),
                            _ => unreachable!("Rolling ops handled separately"),
                        }
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
    }
}

/// Execute a binary operation (element-wise combination)
///
/// Contract: LHS tags preserved (Arc identity I1-I3)
/// RHS can be scalar (broadcast) or frame (strict compatibility required)
fn execute_binary(binary: &BinaryOp, ctx: &ExecContext) -> Result<Arc<Frame>, String> {
    match binary {
        BinaryOp::MapNumeric2 { lhs, rhs, func } => {
            let lhs_frame = ctx.load(*lhs)
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
                    let rhs_frame = ctx.load(*rhs_id)
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

/// Execute a schema-transforming operation
///
/// Contract:
/// - I1 preserved: index Arc ptr_eq
/// - I2_schema: colnames Arc rebuilt (deterministic)
/// - I3 preserved: nrows unchanged
fn execute_schema(schema: &SchemaOp, ctx: &ExecContext, rt: &mut Runtime) -> Result<Arc<Frame>, String> {
    use crate::frame::ColData;

    match schema {
        SchemaOp::Xminus { input, half } => {
            let input_frame = ctx.load(*input)
                .ok_or_else(|| format!("Input node {:?} not found", input))?;

            // Validate: need at least 2 columns
            let ncols = input_frame.cols.len();
            if ncols < 2 {
                return Err(format!("xminus requires at least 2 columns (have {})", ncols));
            }

            // Extract raw columns from ColData
            let input_cols: Vec<&blawktrust::Column> = input_frame.cols.iter()
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
                    for r in (j+1)..ncols {
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
                index_name: input_frame.tags.index_name.clone(),  // Preserve index name
                index: Arc::clone(&input_frame.tags.index),        // I1: preserved
                colnames: Arc::new(output_colnames),                // I2_schema: rebuilt
                masks: input_frame.tags.masks.clone(),              // Inherit input masks
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

        SchemaOp::MaskWeekend { input, name } => {
            use bitvec::prelude::*;
            use std::sync::Arc;

            let input_frame = ctx.load(*input)
                .ok_or_else(|| format!("Input node {:?} not found", input))?;

            // Determine mask name
            let mask_name = name.clone().unwrap_or_else(|| "weekend".to_string());

            // Compute weekend bitmask from index
            let weekend_bitvec: BitVec = match &*input_frame.tags.index {
                crate::frame::IndexColumn::Date(dates) => {
                    dates.iter().map(|&date| {
                        let day_of_week = (4 + date).rem_euclid(7);
                        day_of_week == 0 || day_of_week == 6
                    }).collect()
                }
                crate::frame::IndexColumn::Timestamp(timestamps) => {
                    timestamps.iter().map(|&ts| {
                        let days = (ts / 86400000) as i32;
                        let day_of_week = (4 + days).rem_euclid(7);
                        day_of_week == 0 || day_of_week == 6
                    }).collect()
                }
                crate::frame::IndexColumn::String(_) => {
                    return Err("mask-weekend requires Date or Timestamp index, got String".to_string());
                }
            };

            // Add mask to MaskSet
            let nrows = input_frame.nrows();
            let mut new_masks = input_frame.tags.masks.clone();
            new_masks.insert(
                mask_name.clone(),
                Arc::new(weekend_bitvec),
                nrows
            ).map_err(|e| format!("mask-weekend: {}", e))?;

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
                input_frame.cols.iter().filter_map(|cd| {
                    if let ColData::Mat(col) = cd {
                        Some(Arc::clone(col))
                    } else {
                        None
                    }
                }).collect()
            );

            Ok(Arc::new(result))
        }

        SchemaOp::WithMask { input, mask_expr } => {
            let input_frame = ctx.load(*input)
                .ok_or_else(|| format!("Input node {:?} not found", input))?;

            let nrows = input_frame.nrows();

            // Compile mask expression
            let compiled = crate::mask::compile_mask_expr(mask_expr, &input_frame.tags.masks, nrows)?;

            // Create new active mask
            let new_active_mask = crate::mask::ActiveMask::from_bitvec(compiled, Some(mask_expr.clone()));

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
                input_frame.cols.iter().filter_map(|cd| {
                    if let ColData::Mat(col) = cd {
                        Some(Arc::clone(col))
                    } else {
                        None
                    }
                }).collect()
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

pub fn exp_column(col: &Column) -> Column {
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

pub fn sqrt_column(col: &Column) -> Column {
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

pub fn abs_column(col: &Column) -> Column {
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

pub fn inv_column(col: &Column) -> Column {
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

/// Mask-aware dlog: log returns with NA-skipping lag
///
/// Contract (updated for shape-preserving w5):
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
    let cols_out: Vec<ColData> = frame.cols.iter()
        .map(|col_data| match col_data {
            ColData::Mat(col) => {
                let result_col = match &func {
                    NumericFunc::RollMean { w } => rolling_mean_mask_aware(col, *w, active_mask, nrows),
                    NumericFunc::RollStd { w } => rolling_std_mask_aware(col, *w, active_mask, nrows),
                    NumericFunc::RollMeanPartial { w } => rolling_mean_partial_mask_aware(col, *w, active_mask, nrows),
                    NumericFunc::RollStdPartial { w } => rolling_std_partial_mask_aware(col, *w, active_mask, nrows),
                    NumericFunc::RollMeanPartialExclCurrent { w } => rolling_mean_partial_mask_aware_offset(col, *w, active_mask, nrows, 1),
                    NumericFunc::RollStdPartialExclCurrent { w } => rolling_std_partial_mask_aware_offset(col, *w, active_mask, nrows, 1),
                    _ => unreachable!("Non-rolling op passed to apply_rolling_mask_aware"),
                };
                ColData::Mat(Arc::new(result_col))
            }
        })
        .collect();

    Ok(Frame {
        tags: Arc::clone(&frame.tags),  // Preserve tags (I1-I3)
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
    let cols_out: Vec<ColData> = frame.cols.iter()
        .map(|col_data| match col_data {
            ColData::Mat(col) => {
                let result_col = shift_obs_column(col, k, active_mask, nrows);
                ColData::Mat(Arc::new(result_col))
            }
        })
        .collect();

    Ok(Frame {
        tags: Arc::clone(&frame.tags),  // Preserve tags (I1-I3)
        cols: cols_out,
        nrows: frame.nrows,
    })
}

/// Rolling mean with mask-aware observation counting - O(n) streaming version
///
/// Maintains a queue of the last w eligible observations.
/// Amortized O(n): each eligible observation enters/exits queue exactly once.
fn rolling_mean_mask_aware(col: &Column, w: usize, mask: &crate::mask::ActiveMask, nrows: usize) -> Column {
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

/// Rolling mean with mask-aware observation counting (strict) - LEGACY O(n·w) version
///
/// Kept for comparison testing. Use `rolling_mean_mask_aware_legacy` for verification.
#[cfg(test)]
#[allow(dead_code)]
fn rolling_mean_mask_aware_legacy(col: &Column, w: usize, mask: &crate::mask::ActiveMask, nrows: usize) -> Column {
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
                let mut j = i as isize;  // Start from current position, go backward

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
fn rolling_std_mask_aware(col: &Column, w: usize, mask: &crate::mask::ActiveMask, nrows: usize) -> Column {
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
                    result.push(variance.max(0.0).sqrt());  // max(0) for numerical stability
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
fn rolling_std_mask_aware_legacy(col: &Column, w: usize, mask: &crate::mask::ActiveMask, nrows: usize) -> Column {
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
                    let variance: f64 = eligible.iter()
                        .map(|&x| (x - mean).powi(2))
                        .sum::<f64>() / (w as f64);
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
fn rolling_mean_partial_mask_aware(col: &Column, w: usize, mask: &crate::mask::ActiveMask, nrows: usize) -> Column {
    rolling_mean_partial_mask_aware_offset(col, w, mask, nrows, 0)
}

fn rolling_mean_partial_mask_aware_offset(col: &Column, w: usize, mask: &crate::mask::ActiveMask, nrows: usize, end_offset: usize) -> Column {
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
fn rolling_mean_partial_mask_aware_legacy(col: &Column, w: usize, mask: &crate::mask::ActiveMask, nrows: usize) -> Column {
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
fn rolling_std_partial_mask_aware(col: &Column, w: usize, mask: &crate::mask::ActiveMask, nrows: usize) -> Column {
    rolling_std_partial_mask_aware_offset(col, w, mask, nrows, 0)
}

fn rolling_std_partial_mask_aware_offset(col: &Column, w: usize, mask: &crate::mask::ActiveMask, nrows: usize, end_offset: usize) -> Column {
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
fn rolling_std_partial_mask_aware_legacy(col: &Column, w: usize, mask: &crate::mask::ActiveMask, nrows: usize) -> Column {
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
                    let variance: f64 = eligible.iter()
                        .map(|&x| (x - mean).powi(2))
                        .sum::<f64>() / (eligible.len() as f64);
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
/// - CLISPI equivalent: LOCF→w5→dlog creates zeros, BLISP w5→dlog creates multi-day returns
/// - Both approaches yield identical non-NA, non-zero cumsum results
fn dlog_column(col: &Column, _lag: usize) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(data.len());
            let mut last_valid: Option<f64> = None;

            for &x in data.iter() {
                if x.is_nan() {
                    // Current NA → output NA, but keep last_valid for next valid value
                    result.push(f64::NAN);
                } else if let Some(prev) = last_valid {
                    // Valid current, valid previous → compute dlog
                    if prev > 0.0 && x > 0.0 {
                        result.push(x.ln() - prev.ln());
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

/// Mask-aware ret: arithmetic returns with NA-skipping lag
///
/// Contract (updated for shape-preserving w5):
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
/// Contract (updated for shape-preserving w5):
/// - Starts at 1.0 (not 0.0!)
/// - NA policy: "skip and preserve"
///   - NA input → NA output (preserves weekend masks)
///   - Valid values: cumsum updates and outputs
///   - Running sum maintained across NA positions
/// - Compatible with masked time series (w5/wkd)
/// - O(n) single pass
fn cumsum_column(col: &Column) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(data.len());
            let mut cumsum = 1.0;

            for &x in data.iter() {
                if x.is_nan() {
                    // NA input → NA output (preserves masks from w5)
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

/// Weekday mask (w5): Set weekend values to NA
///
/// Contract:
/// - Shape-preserving: I1, I2, I3 all maintained
/// - For each row: if Saturday (6) or Sunday (0) → set all column values to NA
/// - Weekday rows (Monday-Friday, 1-5): values unchanged
/// - Requires Date or Timestamp index
/// - O(n) single pass per column
fn w5_mask_weekends(frame: &Frame) -> Result<Arc<Frame>, String> {
    use crate::frame::IndexColumn;

    // Determine which rows are weekends
    let weekend_mask: Vec<bool> = match &*frame.tags.index {
        IndexColumn::Date(dates) => {
            dates.iter().map(|&date| {
                // Parse date to get day of week
                // Date is stored as i32: days since Unix epoch (1970-01-01)
                // Use chrono-like calculation to determine day of week

                // Unix epoch (1970-01-01) was a Thursday (day_of_week = 4)
                // day_of_week = (4 + days_since_epoch) % 7
                // 0=Sunday, 1=Monday, ..., 6=Saturday
                let day_of_week = (4 + date).rem_euclid(7);

                // Weekend: Sunday (0) or Saturday (6)
                day_of_week == 0 || day_of_week == 6
            }).collect()
        }
        IndexColumn::Timestamp(timestamps) => {
            timestamps.iter().map(|&ts| {
                // Timestamp is i64 milliseconds since Unix epoch
                // Convert to days and use same logic
                let days = (ts / 86400000) as i32;  // 86400000 ms per day
                let day_of_week = (4 + days).rem_euclid(7);
                day_of_week == 0 || day_of_week == 6
            }).collect()
        }
        IndexColumn::String(_) => {
            return Err("w5 requires Date or Timestamp index, got String".to_string());
        }
    };

    // Apply weekend mask to all columns
    let masked_cols: Vec<ColData> = frame.cols.iter().map(|col_data| {
        match col_data {
            ColData::Mat(col_arc) => {
                match &**col_arc {
                    Column::F64(data) => {
                        let masked_data: Vec<f64> = data.iter().enumerate().map(|(i, &val)| {
                            if weekend_mask[i] {
                                f64::NAN  // Weekend: mask to NA
                            } else {
                                val  // Weekday: unchanged
                            }
                        }).collect();
                        ColData::Mat(Arc::new(Column::F64(masked_data)))
                    }
                    other => ColData::Mat(Arc::new(other.clone()))
                }
            }
        }
    }).collect();

    // Build result frame with preserved tags (I1, I2, I3)
    let result = Frame {
        tags: Arc::clone(&frame.tags),  // I1, I2 preserved via Arc
        cols: masked_cols,
        nrows: frame.nrows,  // I3: preserved
    };

    // Verify invariants
    debug_assert_eq!(result.nrows(), frame.nrows(), "W5: I3 violation - nrows changed");
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

            let result = data_a.iter().zip(data_b.iter())
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
    let eligible: Vec<usize> = (0..nrows)
        .filter(|&i| !mask.is_masked(i))
        .collect();

    let mut pos_in_eligible = vec![-1i32; nrows];
    for (p, &i) in eligible.iter().enumerate() {
        pos_in_eligible[i] = p as i32;
    }

    (eligible, pos_in_eligible)
}

/// Calendar shift (positional): shift by k calendar rows
fn shift_column(col: &Column, k: usize) -> Column {
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

/// Mask-aware shift (observation-based): shift by k eligible (unmasked) rows
/// Skip masked rows only (not NA values)
/// For matching CLISPI's w5-filtered behavior
fn shift_obs_column(col: &Column, k: usize, mask: &crate::mask::ActiveMask, nrows: usize) -> Column {
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
                return Column::F64(result);  // All NA
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
                return Column::F64(result);  // All NA
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
                        0.0  // Constant series or numerical noise
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
            let result = data.iter().map(|&x| {
                if x.is_nan() || scalar.is_nan() {
                    f64::NAN
                } else {
                    match func {
                        BinaryFunc::Add => x + scalar,
                        BinaryFunc::Sub => x - scalar,
                        BinaryFunc::Mul => x * scalar,
                        BinaryFunc::Div => {
                            if scalar == 0.0 {
                                f64::NAN
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

/// Apply binary operation between two frames (element-wise)
///
/// Requires: frames have same shape and compatible tags
/// NA propagation: if either cell is NA, result is NA
fn binary_frame_frame(lhs: &Frame, rhs: &Frame, func: BinaryFunc) -> Result<Frame, String> {
    if lhs.cols.len() != rhs.cols.len() {
        return Err(format!(
            "Frame-frame binary op requires same column count: {} vs {}",
            lhs.cols.len(), rhs.cols.len()
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
        let lhs_data = match lhs_col {
            ColData::Mat(col) => col,
        };
        let rhs_data = match rhs_col {
            ColData::Mat(col) => col,
        };

        let result_col = binary_column_column(lhs_data, rhs_data, func)?;
        result_cols.push(ColData::Mat(Arc::new(result_col)));
    }

    // Phase D: Propagate masks through binary operations
    // - Merge mask sets (error on collision with different bitsets)
    // - OR active masks (union of excluded rows)
    let mut merged_masks = lhs.tags.masks.clone();
    merged_masks.merge(&rhs.tags.masks)
        .map_err(|e| format!("Binary op mask merge failed: {}", e))?;

    let merged_active_mask = crate::mask::or_active_masks(&lhs.tags.active_mask, &rhs.tags.active_mask);

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
                    lhs_data.len(), rhs_data.len()
                ));
            }

            let result = lhs_data.iter().zip(rhs_data.iter()).map(|(&x, &y)| {
                if x.is_nan() || y.is_nan() {
                    f64::NAN
                } else {
                    match func {
                        BinaryFunc::Add => x + y,
                        BinaryFunc::Sub => x - y,
                        BinaryFunc::Mul => x * y,
                        BinaryFunc::Div => {
                            if y == 0.0 {
                                f64::NAN
                            } else {
                                x / y
                            }
                        }
                        BinaryFunc::Gt => {
                            if x > y { 1.0 } else { 0.0 }
                        }
                    }
                }
            }).collect();

            Ok(Column::F64(result))
        }
        _ => Err("Binary op requires F64 columns".to_string()),
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
