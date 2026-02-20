//! Ft-measurable rolling moments with memoization
//!
//! Single-pass computation of mean/std/skew/kurt using shared kernel.

use crate::runtime::Runtime;
use crate::value::Value;
use blawktrust::builtins::{rolling_moments_past_only_f64, MomentsMask, RollingMomentsOutput};
use blawktrust::{Column, Table, TableView};
use std::collections::HashMap;
use std::sync::Arc;

/// Cache key for moments computation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MomentsCacheKey {
    table_ptr: usize, // Pointer address as cache key
    window: usize,
    min_periods: usize,
    mask_bits: u8,
}

/// Memoization cache for moments computation
pub struct MomentsCache {
    cache: HashMap<MomentsCacheKey, Arc<RollingMomentsOutput>>,
}

impl MomentsCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn get_or_compute<F>(
        &mut self,
        table: &TableView,
        window: usize,
        min_periods: usize,
        mask: MomentsMask,
        compute_fn: F,
    ) -> Arc<RollingMomentsOutput>
    where
        F: FnOnce() -> RollingMomentsOutput,
    {
        let key = MomentsCacheKey {
            table_ptr: table as *const _ as usize,
            window,
            min_periods,
            mask_bits: 0, // TODO: Fix to use actual mask bits
        };

        self.cache
            .entry(key)
            .or_insert_with(|| Arc::new(compute_fn()))
            .clone()
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

/// (ft-wmoments-cols table window [:min-periods m] [:moments '(mean std skew kurt count)])
///
/// Compute multiple rolling moments in a single pass.
/// Uses past-only window [i-window, i-1] for Ft-measurable statistics.
///
/// Arguments:
/// - table: Input table
/// - window: Window size
/// - :min-periods (optional): Minimum valid observations (default: window)
/// - :moments (optional): List of moment names (default: all)
///   Valid names: mean, std, skew, kurt, count
///
/// Returns table with columns named "{input_col}_{moment}"
///
/// Example:
///   (ft-wmoments-cols returns 25 :moments '(mean std skew))
///   ; Returns table with columns: col1_mean, col1_std, col1_skew, col2_mean, ...
pub fn builtin_ft_wmoments_cols(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    // Parse arguments
    if args.len() < 2 {
        return Err(format!(
            "ft-wmoments-cols expects at least 2 arguments (table window), got {}",
            args.len()
        ));
    }

    let table = match &args[0] {
        Value::TableView(tv) => tv,
        _ => {
            return Err(format!(
                "ft-wmoments-cols expects TableView, got {}",
                args[0].type_name()
            ))
        }
    };

    let window = args[1].as_int()? as usize;
    if window == 0 {
        return Err("ft-wmoments-cols: window must be > 0".to_string());
    }

    // Parse optional keyword arguments
    let mut min_periods = window;
    let mut mask = MomentsMask::all(); // Default: compute all moments

    let mut i = 2;
    while i < args.len() {
        match &args[i] {
            Value::Sym(sym_id) => {
                let key = rt.interner.resolve(*sym_id);
                if key == ":min-periods" || key == "min-periods" {
                    if i + 1 >= args.len() {
                        return Err("ft-wmoments-cols: :min-periods requires a value".to_string());
                    }
                    min_periods = args[i + 1].as_int()? as usize;
                    i += 2;
                } else if key == ":moments" || key == "moments" {
                    if i + 1 >= args.len() {
                        return Err("ft-wmoments-cols: :moments requires a value".to_string());
                    }
                    // Parse moments list
                    mask = parse_moments_list(&args[i + 1], rt)?;
                    i += 2;
                } else {
                    return Err(format!("ft-wmoments-cols: unknown keyword: {}", key));
                }
            }
            _ => {
                return Err(format!(
                    "ft-wmoments-cols: expected keyword, got {}",
                    args[i].type_name()
                ))
            }
        }
    }

    // Compute moments for all numeric columns
    let mut output_cols: Vec<(String, Column)> = Vec::new();

    for (i, col_name) in table.table.names.iter().enumerate() {
        let col = &table.table.columns[i];

        if let Column::F64(data) = col {
            // blawktrust uses kdb-style embedded nulls (NaN for F64), no validity bitmap
            let output =
                rolling_moments_past_only_f64(data, window, Some(min_periods), mask, None);

            // Add requested moment columns
            if let Some(mean_vec) = output.mean {
                output_cols.push((format!("{}_mean", col_name), Column::new_f64(mean_vec)));
            }
            if let Some(std_vec) = output.std {
                output_cols.push((format!("{}_std", col_name), Column::new_f64(std_vec)));
            }
            if let Some(skew_vec) = output.skew {
                output_cols.push((format!("{}_skew", col_name), Column::new_f64(skew_vec)));
            }
            if let Some(kurt_vec) = output.kurt {
                output_cols.push((format!("{}_kurt", col_name), Column::new_f64(kurt_vec)));
            }
            if let Some(count_vec) = output.count {
                output_cols.push((format!("{}_count", col_name), Column::new_f64(count_vec)));
            }
        }
    }

    if output_cols.is_empty() {
        return Err("ft-wmoments-cols: no numeric columns found".to_string());
    }

    // Build output table
    let col_names: Vec<String> = output_cols.iter().map(|(name, _)| name.clone()).collect();
    let columns: Vec<Column> = output_cols.into_iter().map(|(_, col)| col).collect();

    let out_table = Table::new(col_names, columns);
    Ok(Value::TableView(Arc::new(TableView::new(out_table))))
}

/// Parse moments list from Lisp value
fn parse_moments_list(val: &Value, rt: &Runtime) -> Result<MomentsMask, String> {
    match val {
        Value::List(items) => {
            let mut names = Vec::new();
            for item in items {
                match item {
                    Value::Sym(sym_id) => {
                        let name = rt.interner.resolve(*sym_id);
                        names.push(name);
                    }
                    _ => {
                        return Err(format!(
                            "ft-wmoments-cols: moments list must contain symbols, got {}",
                            item.type_name()
                        ))
                    }
                }
            }
            let name_strs: Vec<&str> = names.iter().map(|s| s.as_ref()).collect();
            Ok(MomentsMask::from_names(&name_strs))
        }
        _ => Err(format!(
            "ft-wmoments-cols: :moments must be a list, got {}",
            val.type_name()
        )),
    }
}

/// Wrapper: ft-wmean-cols
pub fn builtin_ft_wmean_cols(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    // Rewrite as ft-wmoments-cols with moments=(mean)
    let moments_list = Value::List(vec![Value::Sym(rt.interner.intern("mean"))]);
    let mut new_args = args.to_vec();
    new_args.push(Value::Sym(rt.interner.intern(":moments")));
    new_args.push(moments_list);

    builtin_ft_wmoments_cols(rt, &new_args)
}

/// Wrapper: ft-wstd-cols
pub fn builtin_ft_wstd_cols(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    let moments_list = Value::List(vec![Value::Sym(rt.interner.intern("std"))]);
    let mut new_args = args.to_vec();
    new_args.push(Value::Sym(rt.interner.intern(":moments")));
    new_args.push(moments_list);

    builtin_ft_wmoments_cols(rt, &new_args)
}

/// Wrapper: ft-wskew-cols
pub fn builtin_ft_wskew_cols(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    let moments_list = Value::List(vec![Value::Sym(rt.interner.intern("skew"))]);
    let mut new_args = args.to_vec();
    new_args.push(Value::Sym(rt.interner.intern(":moments")));
    new_args.push(moments_list);

    builtin_ft_wmoments_cols(rt, &new_args)
}

/// Wrapper: ft-wkurt-cols
pub fn builtin_ft_wkurt_cols(rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    let moments_list = Value::List(vec![Value::Sym(rt.interner.intern("kurt"))]);
    let mut new_args = args.to_vec();
    new_args.push(Value::Sym(rt.interner.intern(":moments")));
    new_args.push(moments_list);

    builtin_ft_wmoments_cols(rt, &new_args)
}
