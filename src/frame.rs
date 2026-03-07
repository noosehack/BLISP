//! BLADE Frame: Tags + Numeric Payload (P2 Policy)
//!
//! Implements BLISP_BLADE_Blueprint.txt Phase 1 & 2:
//! - Tags: index + colnames (Arc-shared, zero-copy)
//! - Frame: tags + numeric columns
//! - map_numeric_preserve_tags: core primitive (Phase 1)
//! - reindex_by: alignment primitive (Phase 2)

use std::collections::HashMap;
use std::sync::Arc;
use rayon::prelude::*;

/// Index column: Date, Timestamp, or String rownames
#[derive(Debug, Clone)]
pub enum IndexColumn {
    Date(Arc<Vec<i32>>),      // Days since epoch
    Timestamp(Arc<Vec<i64>>), // Nanoseconds since epoch
    String(Arc<Vec<String>>), // Generic rownames
}

impl IndexColumn {
    #[inline]
    pub fn len(&self) -> usize {
        match self {
            IndexColumn::Date(v) => v.len(),
            IndexColumn::Timestamp(v) => v.len(),
            IndexColumn::String(v) => v.len(),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get index value as a hashable key
    pub fn get(&self, idx: usize) -> Option<IndexKey> {
        match self {
            IndexColumn::Date(v) => v.get(idx).map(|&d| IndexKey::Date(d)),
            IndexColumn::Timestamp(v) => v.get(idx).map(|&t| IndexKey::Timestamp(t)),
            IndexColumn::String(v) => v.get(idx).map(|s| IndexKey::String(s.clone())),
        }
    }
}

/// Tags: metadata carried by Arc (P2 policy: index + colnames + masks)
#[derive(Debug, Clone)]
pub struct Tags {
    pub index_name: String,                   // e.g., "DATE", "TIMESTAMP"
    pub index: Arc<IndexColumn>,              // Row identifiers
    pub colnames: Arc<Vec<String>>,           // Numeric column names (in order)
    pub masks: crate::mask::MaskSet,          // Named row masks (weekend, holiday, etc.)
    pub active_mask: crate::mask::ActiveMask, // Currently active mask (compiled)
}

impl Tags {
    /// Create new tags
    pub fn new(index_name: String, index: IndexColumn, colnames: Vec<String>) -> Self {
        let nrows = index.len();
        Self {
            index_name,
            index: Arc::new(index),
            colnames: Arc::new(colnames),
            masks: crate::mask::MaskSet::new(),
            active_mask: crate::mask::ActiveMask::empty(nrows),
        }
    }

    /// Clone tags (Arc clone = pointer copy only, O(1))
    #[inline]
    pub fn clone_arc(&self) -> Self {
        Self {
            index_name: self.index_name.clone(),
            index: Arc::clone(&self.index),
            colnames: Arc::clone(&self.colnames),
            masks: self.masks.clone(),
            active_mask: self.active_mask.clone(),
        }
    }

    /// Number of rows
    #[inline]
    pub fn nrows(&self) -> usize {
        self.index.len()
    }

    /// Number of columns
    #[inline]
    pub fn ncols(&self) -> usize {
        self.colnames.len()
    }
}

/// Column data: materialized or lazy (future)
#[derive(Debug, Clone)]
pub enum ColData {
    Mat(Arc<blawktrust::Column>),
    // Future: Expr(ExprId) for lazy evaluation
}

/// Frame: tags + numeric payload (BLADE core structure)
#[derive(Debug, Clone)]
pub struct Frame {
    pub tags: Arc<Tags>,    // Shared metadata (zero-copy)
    pub cols: Vec<ColData>, // Numeric columns (same order as tags.colnames)
    pub nrows: usize,       // Cached
}

impl Frame {
    /// Create new frame
    pub fn new(tags: Tags, cols: Vec<Arc<blawktrust::Column>>) -> Self {
        let nrows = tags.nrows();
        let col_data = cols.into_iter().map(ColData::Mat).collect();

        Self {
            tags: Arc::new(tags),
            cols: col_data,
            nrows,
        }
    }

    /// Create from Arc<Tags> (reuse existing tags)
    pub fn with_tags(tags: Arc<Tags>, cols: Vec<Arc<blawktrust::Column>>) -> Self {
        let nrows = tags.nrows();
        let col_data = cols.into_iter().map(ColData::Mat).collect();

        Self {
            tags,
            cols: col_data,
            nrows,
        }
    }

    /// Get materialized column by index
    pub fn get_col(&self, idx: usize) -> Option<&Arc<blawktrust::Column>> {
        match self.cols.get(idx)? {
            ColData::Mat(col) => Some(col),
        }
    }

    /// Number of rows
    #[inline]
    pub fn nrows(&self) -> usize {
        self.nrows
    }

    /// Number of columns
    #[inline]
    pub fn ncols(&self) -> usize {
        self.cols.len()
    }
}

/// CORE PRIMITIVE: map_numeric_preserve_tags
///
/// Apply function to all numeric columns, preserve tags.
/// This is the SINGLE SOURCE OF TRUTH for unary numeric operations.
///
/// Invariants enforced (Blueprint I1-I3):
/// - I1: output.tags.index == input.tags.index (same Arc)
/// - I2: output.tags.colnames == input.tags.colnames (same Arc)
/// - I3: output.nrows == input.nrows
pub fn map_numeric_preserve_tags<F>(frame: &Frame, f: F) -> Frame
where
    F: Fn(&blawktrust::Column) -> blawktrust::Column + Sync,
{
    // Arc clone tags (pointer copy only, O(1))
    let tags_out = Arc::clone(&frame.tags);

    // Transform columns in parallel across CPU cores (like Adyton's mthr)
    let cols_out: Vec<ColData> = frame
        .cols
        .par_iter()
        .map(|col_data| match col_data {
            ColData::Mat(col) => ColData::Mat(Arc::new(f(col))),
        })
        .collect();

    Frame {
        tags: tags_out,
        cols: cols_out,
        nrows: frame.nrows,
    }
}

/// Hashable index key for lookups
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IndexKey {
    Date(i32),
    Timestamp(i64),
    String(String),
}

/// PHASE 2: Reindex source frame onto target index (Blueprint Phase 2.1)
///
/// This is the alignment primitive for mapr/joins.
///
/// Algorithm:
/// 1. Build hashmap: source.index -> row_id
/// 2. For each target_index value:
///    - Lookup in hashmap
///    - If found: copy numeric row from source
///    - If missing: write NA row
///
/// Invariants:
/// - Output index = target_index (Arc reused)
/// - Output colnames = source.colnames (Arc reused)
/// - Output nrows = target_index.len()
///
/// Semantics: RIGHT OUTER JOIN
///   Result always has ALL rows from target_index
///   Matching rows from source preserved
///   Missing rows -> NA
pub fn reindex_by(source: &Frame, target_index: Arc<IndexColumn>) -> Frame {
    // Build hashmap: source index value -> row index
    let mut index_map: HashMap<IndexKey, usize> = HashMap::new();
    for i in 0..source.tags.index.len() {
        if let Some(key) = source.tags.index.as_ref().get(i) {
            index_map.insert(key, i);
        }
    }

    let target_nrows = target_index.len();
    let ncols = source.ncols();

    // Allocate output columns (pre-sized)
    let mut out_cols: Vec<Vec<f64>> = (0..ncols)
        .map(|_| Vec::with_capacity(target_nrows))
        .collect();

    // For each target row, lookup or NA
    for target_row in 0..target_nrows {
        if let Some(target_key) = target_index.get(target_row) {
            match index_map.get(&target_key) {
                Some(&source_row) => {
                    // Found: copy numeric values from source
                    for col_idx in 0..ncols {
                        let val = match source.get_col(col_idx) {
                            Some(col) => match &**col {
                                blawktrust::Column::F64(data) => {
                                    if source_row < data.len() {
                                        data[source_row]
                                    } else {
                                        f64::NAN
                                    }
                                }
                                _ => f64::NAN,
                            },
                            None => f64::NAN,
                        };
                        out_cols[col_idx].push(val);
                    }
                }
                None => {
                    // Missing: write NA row
                    for col_idx in 0..ncols {
                        out_cols[col_idx].push(f64::NAN);
                    }
                }
            }
        } else {
            // Invalid target index: write NA row
            for col_idx in 0..ncols {
                out_cols[col_idx].push(f64::NAN);
            }
        }
    }

    // Build output frame
    let out_numeric: Vec<Arc<blawktrust::Column>> = out_cols
        .into_iter()
        .map(|vec| Arc::new(blawktrust::Column::new_f64(vec)))
        .collect();

    // Create new Tags with target_index and source colnames (both Arc-reused)
    // Phase G: Reindex masks from source onto target_index
    // Policy: Named masks reindexed, new rows default to false (unmasked)
    let reindexed_masks =
        crate::mask::reindex_maskset(&source.tags.masks, &source.tags.index, &target_index);

    let reindexed_active_mask = crate::mask::reindex_active_mask(
        &source.tags.active_mask,
        &reindexed_masks,
        &source.tags.index,
        &target_index,
    );

    let out_tags = Tags {
        index_name: source.tags.index_name.clone(),
        index: target_index,                         // Arc reused!
        colnames: Arc::clone(&source.tags.colnames), // Arc reused!
        masks: reindexed_masks,
        active_mask: reindexed_active_mask,
    };

    Frame::new(out_tags, out_numeric)
}

/// Check if index is sorted (monotone nondecreasing)
fn is_sorted(index: &IndexColumn) -> bool {
    match index {
        IndexColumn::Date(v) => v.windows(2).all(|w| w[0] <= w[1]),
        IndexColumn::Timestamp(v) => v.windows(2).all(|w| w[0] <= w[1]),
        IndexColumn::String(v) => v.windows(2).all(|w| w[0] <= w[1]),
    }
}

/// Compare two index keys (for asof: ≤ comparison)
fn index_key_le(a: &IndexKey, b: &IndexKey) -> bool {
    match (a, b) {
        (IndexKey::Date(x), IndexKey::Date(y)) => x <= y,
        (IndexKey::Timestamp(x), IndexKey::Timestamp(y)) => x <= y,
        (IndexKey::String(x), IndexKey::String(y)) => x <= y,
        _ => false, // Type mismatch
    }
}

/// PHASE 2 (Extended): Asof join primitive (RIGHT OUTER ASOF JOIN) - PUBLIC API
///
/// asofr(x, y) = For each t in y.index, pick t' = max{x.index ≤ t}
///
/// Algorithm:
/// - Fast path (sorted indices): Two-pointer merge O(nx + ny)
/// - Fallback (unsorted): Sort + merge or hashmap
///
/// Invariants (per docs/contracts.md):
/// - Output index = y.index (Arc reused)
/// - Output colnames = x.colnames (Arc reused)
/// - Output nrows = y.nrows
/// - At-or-before only (no forward-looking bias)
/// - Duplicates in x: last wins
/// - Missing → NA row
///
/// Semantics: RIGHT OUTER ASOF JOIN
///   For each y timestamp, find the most recent x value at-or-before
///   Never uses future x values (Ft-measurable)
pub fn asofr(x: &Frame, y: &Frame) -> Frame {
    let x_sorted = is_sorted(&x.tags.index);
    let y_sorted = is_sorted(&y.tags.index);

    if x_sorted && y_sorted {
        // Fast path: Two-pointer merge O(nx + ny)
        asofr_sorted(x, y)
    } else {
        // Fallback: Use hashmap approach (conservative)
        // TODO: Implement sorted-view optimization for unsorted case
        asofr_fallback(x, y)
    }
}

/// Fast path: asof join with sorted indices (two-pointer merge)
fn asofr_sorted(x: &Frame, y: &Frame) -> Frame {
    let y_nrows = y.tags.index.len();
    let x_nrows = x.tags.index.len();
    let ncols = x.ncols();

    // Allocate output columns
    let mut out_cols: Vec<Vec<f64>> = (0..ncols).map(|_| Vec::with_capacity(y_nrows)).collect();

    // Two-pointer merge
    let mut x_ptr = -1_isize; // Last valid x index (-1 = none found yet)

    for y_row in 0..y_nrows {
        let y_key = y.tags.index.get(y_row);

        if let Some(y_key) = y_key {
            // Advance x_ptr while next x value <= current y value
            // Start from x_ptr+1 (or 0 if x_ptr=-1)
            let start_check = if x_ptr < 0 { 0 } else { (x_ptr + 1) as usize };

            for check_idx in start_check..x_nrows {
                if let Some(x_key) = x.tags.index.get(check_idx) {
                    if index_key_le(&x_key, &y_key) {
                        x_ptr = check_idx as isize; // Update to this x
                    } else {
                        break; // x values beyond this are > y_key
                    }
                } else {
                    break;
                }
            }

            // Emit row: use x_ptr if valid, else NA
            if x_ptr >= 0 {
                let source_row = x_ptr as usize;
                for col_idx in 0..ncols {
                    let val = match x.get_col(col_idx) {
                        Some(col) => match &**col {
                            blawktrust::Column::F64(data) => {
                                if source_row < data.len() {
                                    data[source_row]
                                } else {
                                    f64::NAN
                                }
                            }
                            _ => f64::NAN,
                        },
                        None => f64::NAN,
                    };
                    out_cols[col_idx].push(val);
                }
            } else {
                // No x value at-or-before y → NA row
                for col_idx in 0..ncols {
                    out_cols[col_idx].push(f64::NAN);
                }
            }
        } else {
            // Invalid y index → NA row
            for col_idx in 0..ncols {
                out_cols[col_idx].push(f64::NAN);
            }
        }
    }

    // Build output frame
    let out_numeric: Vec<Arc<blawktrust::Column>> = out_cols
        .into_iter()
        .map(|vec| Arc::new(blawktrust::Column::new_f64(vec)))
        .collect();

    // Phase D: For asofr, result has Y's index, so inherit Y's masks
    let out_tags = Tags {
        index_name: y.tags.index_name.clone(),
        index: Arc::clone(&y.tags.index),        // Arc reused!
        colnames: Arc::clone(&x.tags.colnames),  // Arc reused!
        masks: y.tags.masks.clone(),             // Inherit Y's masks (result has Y's index)
        active_mask: y.tags.active_mask.clone(), // Inherit Y's active mask
    };

    Frame::new(out_tags, out_numeric)
}

/// Fallback: asof join with unsorted indices
/// TODO: Optimize with sorted views
fn asofr_fallback(x: &Frame, y: &Frame) -> Frame {
    // For now: collect all (x_index, x_row) pairs, find best match per y
    let y_nrows = y.tags.index.len();
    let ncols = x.ncols();

    // Build list of (x_key, x_row)
    let mut x_entries: Vec<(IndexKey, usize)> = Vec::new();
    for i in 0..x.tags.index.len() {
        if let Some(key) = x.tags.index.get(i) {
            x_entries.push((key, i));
        }
    }

    // Allocate output
    let mut out_cols: Vec<Vec<f64>> = (0..ncols).map(|_| Vec::with_capacity(y_nrows)).collect();

    // For each y row, find best x (linear scan - O(n²) but correct)
    for y_row in 0..y_nrows {
        let y_key = y.tags.index.get(y_row);

        if let Some(y_key) = y_key {
            // Find max { x_key : x_key <= y_key }
            let mut best_x_row: Option<usize> = None;
            let mut best_x_key: Option<&IndexKey> = None;

            for (x_key, x_row) in &x_entries {
                if index_key_le(x_key, &y_key) {
                    if best_x_key.is_none() || index_key_le(best_x_key.unwrap(), x_key) {
                        // x_key <= y_key and (no best yet or x_key >= best)
                        best_x_key = Some(x_key);
                        best_x_row = Some(*x_row);
                    }
                }
            }

            // Emit row
            if let Some(source_row) = best_x_row {
                for col_idx in 0..ncols {
                    let val = match x.get_col(col_idx) {
                        Some(col) => match &**col {
                            blawktrust::Column::F64(data) => {
                                if source_row < data.len() {
                                    data[source_row]
                                } else {
                                    f64::NAN
                                }
                            }
                            _ => f64::NAN,
                        },
                        None => f64::NAN,
                    };
                    out_cols[col_idx].push(val);
                }
            } else {
                // No x at-or-before y → NA
                for col_idx in 0..ncols {
                    out_cols[col_idx].push(f64::NAN);
                }
            }
        } else {
            // Invalid y → NA
            for col_idx in 0..ncols {
                out_cols[col_idx].push(f64::NAN);
            }
        }
    }

    // Build output
    let out_numeric: Vec<Arc<blawktrust::Column>> = out_cols
        .into_iter()
        .map(|vec| Arc::new(blawktrust::Column::new_f64(vec)))
        .collect();

    // Phase D: For asofr_fallback, result has Y's index, so inherit Y's masks
    let out_tags = Tags {
        index_name: y.tags.index_name.clone(),
        index: Arc::clone(&y.tags.index),
        colnames: Arc::clone(&x.tags.colnames),
        masks: y.tags.masks.clone(), // Inherit Y's masks (result has Y's index)
        active_mask: y.tags.active_mask.clone(), // Inherit Y's active mask
    };

    Frame::new(out_tags, out_numeric)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_column_len() {
        let dates = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002]));
        assert_eq!(dates.len(), 3);
        assert!(!dates.is_empty());
    }

    #[test]
    fn test_tags_creation() {
        let index = IndexColumn::Date(Arc::new(vec![18000, 18001]));
        let colnames = vec!["price".to_string(), "volume".to_string()];
        let tags = Tags::new("DATE".to_string(), index, colnames);

        assert_eq!(tags.index_name, "DATE");
        assert_eq!(tags.nrows(), 2);
        assert_eq!(tags.ncols(), 2);
    }

    #[test]
    fn test_tags_arc_clone() {
        let index = IndexColumn::Date(Arc::new(vec![18000]));
        let tags = Tags::new("DATE".to_string(), index, vec!["col1".to_string()]);
        let cloned = tags.clone_arc();

        // Arc pointers should be equal (shared)
        assert!(Arc::ptr_eq(&tags.index, &cloned.index));
        assert!(Arc::ptr_eq(&tags.colnames, &cloned.colnames));
    }

    #[test]
    fn test_frame_creation() {
        let index = IndexColumn::Date(Arc::new(vec![18000, 18001]));
        let tags = Tags::new("DATE".to_string(), index, vec!["px".to_string()]);
        let col = Arc::new(blawktrust::Column::new_f64(vec![100.0, 101.0]));
        let frame = Frame::new(tags, vec![col]);

        assert_eq!(frame.nrows(), 2);
        assert_eq!(frame.ncols(), 1);
    }

    #[test]
    fn test_map_numeric_preserve_tags_invariants() {
        // Create test frame
        let index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002]));
        let tags = Tags::new("DATE".to_string(), index, vec!["price".to_string()]);
        let col = Arc::new(blawktrust::Column::new_f64(vec![100.0, 101.0, 102.0]));
        let frame = Frame::new(tags, vec![col]);

        // Apply identity transformation
        let result = map_numeric_preserve_tags(&frame, |c| c.clone());

        // I1: Same index Arc
        assert!(
            Arc::ptr_eq(&frame.tags.index, &result.tags.index),
            "I1 VIOLATED: Index Arc not preserved"
        );

        // I2: Same colnames Arc
        assert!(
            Arc::ptr_eq(&frame.tags.colnames, &result.tags.colnames),
            "I2 VIOLATED: Colnames Arc not preserved"
        );

        // I3: Same nrows
        assert_eq!(
            frame.nrows(),
            result.nrows(),
            "I3 VIOLATED: Row count changed"
        );
    }

    #[test]
    fn test_i1_i2_i3_with_dlog() {
        use blawktrust::builtins::ops::dlog_column;

        // Create test frame
        let index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002]));
        let tags = Tags::new("DATE".to_string(), index, vec!["price".to_string()]);
        let col = Arc::new(blawktrust::Column::new_f64(vec![100.0, 101.0, 102.0]));
        let frame = Frame::new(tags, vec![col]);

        // Apply dlog using map_numeric_preserve_tags
        let result = map_numeric_preserve_tags(&frame, |c| dlog_column(c, 1));

        // I1: Same index Arc (pointer equality)
        assert!(
            Arc::ptr_eq(&frame.tags.index, &result.tags.index),
            "I1 VIOLATED: dlog didn't preserve index Arc"
        );

        // I2: Same colnames Arc (pointer equality)
        assert!(
            Arc::ptr_eq(&frame.tags.colnames, &result.tags.colnames),
            "I2 VIOLATED: dlog didn't preserve colnames Arc"
        );

        // I3: Same nrows
        assert_eq!(
            frame.nrows(),
            result.nrows(),
            "I3 VIOLATED: dlog changed row count"
        );

        // Verify operation actually worked (first value should be NA for lag=1)
        let out_col = result.get_col(0).unwrap();
        match &**out_col {
            blawktrust::Column::F64(data) => {
                assert!(data[0].is_nan(), "First value should be NA for dlog lag=1");
                // Second value should be log(101/100) ≈ 0.00995
                assert!((data[1] - 0.00995).abs() < 0.0001);
            }
            _ => panic!("Expected F64 column"),
        }
    }

    #[test]
    fn test_reindex_by_all_matching() {
        // Source: dates [18000, 18001, 18002], values [100, 101, 102]
        let source_index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002]));
        let source_tags = Tags::new("DATE".to_string(), source_index, vec!["price".to_string()]);
        let source_col = Arc::new(blawktrust::Column::new_f64(vec![100.0, 101.0, 102.0]));
        let source = Frame::new(source_tags, vec![source_col]);

        // Target: same dates (all should match)
        let target_index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002]));

        let result = reindex_by(&source, Arc::new(target_index));

        // Check output
        assert_eq!(result.nrows(), 3);
        assert_eq!(result.ncols(), 1);

        // Verify values (all should match)
        let out_col = result.get_col(0).unwrap();
        match &**out_col {
            blawktrust::Column::F64(data) => {
                assert_eq!(data[0], 100.0);
                assert_eq!(data[1], 101.0);
                assert_eq!(data[2], 102.0);
            }
            _ => panic!("Expected F64 column"),
        }

        // Verify colnames Arc preserved
        assert!(
            Arc::ptr_eq(&source.tags.colnames, &result.tags.colnames),
            "reindex_by should preserve colnames Arc"
        );
    }

    #[test]
    fn test_reindex_by_with_missing_rows() {
        // Source: dates [18000, 18002], values [100, 102] (missing 18001)
        let source_index = IndexColumn::Date(Arc::new(vec![18000, 18002]));
        let source_tags = Tags::new("DATE".to_string(), source_index, vec!["price".to_string()]);
        let source_col = Arc::new(blawktrust::Column::new_f64(vec![100.0, 102.0]));
        let source = Frame::new(source_tags, vec![source_col]);

        // Target: dates [18000, 18001, 18002] (18001 missing from source)
        let target_index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002]));

        let result = reindex_by(&source, Arc::new(target_index));

        // Check output
        assert_eq!(result.nrows(), 3, "Should have 3 rows (target size)");

        // Verify values: [100, NA, 102]
        let out_col = result.get_col(0).unwrap();
        match &**out_col {
            blawktrust::Column::F64(data) => {
                assert_eq!(data[0], 100.0, "First row should match");
                assert!(data[1].is_nan(), "Missing row should be NA");
                assert_eq!(data[2], 102.0, "Third row should match");
            }
            _ => panic!("Expected F64 column"),
        }
    }

    #[test]
    fn test_reindex_by_reordering() {
        // Source: dates [18002, 18000, 18001], values [102, 100, 101] (out of order)
        let source_index = IndexColumn::Date(Arc::new(vec![18002, 18000, 18001]));
        let source_tags = Tags::new("DATE".to_string(), source_index, vec!["price".to_string()]);
        let source_col = Arc::new(blawktrust::Column::new_f64(vec![102.0, 100.0, 101.0]));
        let source = Frame::new(source_tags, vec![source_col]);

        // Target: dates [18000, 18001, 18002] (canonical order)
        let target_index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002]));

        let result = reindex_by(&source, Arc::new(target_index));

        // Verify values reordered to match target: [100, 101, 102]
        let out_col = result.get_col(0).unwrap();
        match &**out_col {
            blawktrust::Column::F64(data) => {
                assert_eq!(data[0], 100.0, "Should be reordered to target[0] = 18000");
                assert_eq!(data[1], 101.0, "Should be reordered to target[1] = 18001");
                assert_eq!(data[2], 102.0, "Should be reordered to target[2] = 18002");
            }
            _ => panic!("Expected F64 column"),
        }
    }

    #[test]
    fn test_reindex_by_semantics_right_outer_join() {
        // This test verifies the RIGHT OUTER JOIN semantics:
        // - Result has ALL rows from target
        // - Matching rows from source preserved
        // - Missing rows -> NA

        // Source: [A, C] with values [1, 3]
        let source_index = IndexColumn::String(Arc::new(vec!["A".to_string(), "C".to_string()]));
        let source_tags = Tags::new("ID".to_string(), source_index, vec!["value".to_string()]);
        let source_col = Arc::new(blawktrust::Column::new_f64(vec![1.0, 3.0]));
        let source = Frame::new(source_tags, vec![source_col]);

        // Target: [A, B, C, D]
        let target_index = IndexColumn::String(Arc::new(vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
            "D".to_string(),
        ]));

        let result = reindex_by(&source, Arc::new(target_index));

        // Result should be: [1, NA, 3, NA]
        assert_eq!(result.nrows(), 4, "RIGHT OUTER JOIN: all target rows");

        let out_col = result.get_col(0).unwrap();
        match &**out_col {
            blawktrust::Column::F64(data) => {
                assert_eq!(data[0], 1.0, "A matches");
                assert!(data[1].is_nan(), "B missing -> NA");
                assert_eq!(data[2], 3.0, "C matches");
                assert!(data[3].is_nan(), "D missing -> NA");
            }
            _ => panic!("Expected F64 column"),
        }
    }

    // ==================== PROPERTY TESTS (Semantic Tripwires) ====================
    // These catch regressions in alignment semantics (per docs/contracts.md)

    #[test]
    fn property_mapr_idempotence() {
        // Property: mapr(mapr(x,y), y) == mapr(x,y)
        // Rationale: Reindexing twice onto same target is idempotent

        let x_index = IndexColumn::Date(Arc::new(vec![18000, 18002]));
        let x_tags = Tags::new("DATE".to_string(), x_index, vec!["price".to_string()]);
        let x_col = Arc::new(blawktrust::Column::new_f64(vec![100.0, 102.0]));
        let x = Frame::new(x_tags, vec![x_col]);

        let y_index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002]));
        let y_tags = Tags::new("DATE".to_string(), y_index, vec!["dummy".to_string()]);
        let y_col = Arc::new(blawktrust::Column::new_f64(vec![1.0, 2.0, 3.0]));
        let y = Frame::new(y_tags, vec![y_col]);

        // First application
        let once = reindex_by(&x, Arc::clone(&y.tags.index));
        // Second application (should be identical)
        let twice = reindex_by(&once, Arc::clone(&y.tags.index));

        // Numeric equality
        assert_eq!(once.nrows(), twice.nrows(), "Idempotence: row count");
        assert_eq!(once.ncols(), twice.ncols(), "Idempotence: col count");

        for col_idx in 0..once.ncols() {
            let once_col = once.get_col(col_idx).unwrap();
            let twice_col = twice.get_col(col_idx).unwrap();
            match (&**once_col, &**twice_col) {
                (blawktrust::Column::F64(d1), blawktrust::Column::F64(d2)) => {
                    for i in 0..d1.len() {
                        let v1 = d1[i];
                        let v2 = d2[i];
                        if v1.is_nan() && v2.is_nan() {
                            continue; // Both NA = equal
                        }
                        assert_eq!(v1, v2, "Idempotence: value at row {}", i);
                    }
                }
                _ => panic!("Expected F64 columns"),
            }
        }

        // Arc preservation (twice should still share y's colnames from once)
        assert!(
            Arc::ptr_eq(&once.tags.colnames, &twice.tags.colnames),
            "Idempotence: colnames Arc must be preserved"
        );
    }

    #[test]
    fn property_mapr_identity_when_indices_match() {
        // Property: If x.index == y.index, then mapr(x,y) numerically equals x
        // (Arcs may differ since we create new Tags, but values identical)

        let index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002]));
        let x_tags = Tags::new("DATE".to_string(), index.clone(), vec!["price".to_string()]);
        let x_col = Arc::new(blawktrust::Column::new_f64(vec![100.0, 101.0, 102.0]));
        let x = Frame::new(x_tags, vec![x_col]);

        // y has SAME index values
        let result = reindex_by(&x, Arc::new(index));

        // Numeric equality
        assert_eq!(result.nrows(), x.nrows(), "Identity: row count");
        let result_col = result.get_col(0).unwrap();
        let x_col_ref = x.get_col(0).unwrap();
        match (&**result_col, &**x_col_ref) {
            (blawktrust::Column::F64(r), blawktrust::Column::F64(orig)) => {
                for i in 0..r.len() {
                    assert_eq!(r[i], orig[i], "Identity: value at row {}", i);
                }
            }
            _ => panic!("Expected F64 columns"),
        }
    }

    #[test]
    fn property_mapr_monotonicity() {
        // Property: mapr(x, y).nrows == y.nrows ALWAYS (regardless of x)
        // This is the RIGHT OUTER JOIN guarantee

        // Small x
        let x_index = IndexColumn::Date(Arc::new(vec![18000]));
        let x_tags = Tags::new("DATE".to_string(), x_index, vec!["price".to_string()]);
        let x_col = Arc::new(blawktrust::Column::new_f64(vec![100.0]));
        let x = Frame::new(x_tags, vec![x_col]);

        // Large y
        let y_index = IndexColumn::Date(Arc::new(vec![17999, 18000, 18001, 18002, 18003]));
        let y_nrows = y_index.len();

        let result = reindex_by(&x, Arc::new(y_index));

        assert_eq!(
            result.nrows(),
            y_nrows,
            "Monotonicity: output rows must equal target rows"
        );
    }

    #[test]
    fn property_no_forward_looking_bias() {
        // Property: mapr NEVER invents non-NA data
        // All non-NA values in result exist in source

        let x_index = IndexColumn::Date(Arc::new(vec![18000, 18002]));
        let x_tags = Tags::new("DATE".to_string(), x_index, vec!["price".to_string()]);
        let x_col = Arc::new(blawktrust::Column::new_f64(vec![100.0, 102.0]));
        let x = Frame::new(x_tags, vec![x_col]);

        let y_index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002, 18003]));
        let result = reindex_by(&x, Arc::new(y_index));

        // Check: non-NA values must be from x
        let result_col = result.get_col(0).unwrap();
        match &**result_col {
            blawktrust::Column::F64(data) => {
                assert_eq!(data[0], 100.0, "Row 0: from x");
                assert!(data[1].is_nan(), "Row 1: missing in x → NA");
                assert_eq!(data[2], 102.0, "Row 2: from x");
                assert!(data[3].is_nan(), "Row 3: missing in x → NA");

                // NO invented data
                for &val in data.iter() {
                    if !val.is_nan() {
                        assert!(
                            val == 100.0 || val == 102.0,
                            "Non-NA value {} not from source x",
                            val
                        );
                    }
                }
            }
            _ => panic!("Expected F64 column"),
        }
    }

    #[test]
    #[allow(clippy::type_complexity)] // Test uses collection of function pointers
    fn property_arc_preservation_numeric_ops() {
        // Property: map_numeric_preserve_tags MUST preserve tag Arcs (I1-I2)

        use blawktrust::builtins::ops::dlog_column;

        let index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002]));
        let tags = Tags::new("DATE".to_string(), index, vec!["price".to_string()]);
        let col = Arc::new(blawktrust::Column::new_f64(vec![100.0, 101.0, 102.0]));
        let frame = Frame::new(tags, vec![col]);

        // Apply various operations
        let ops: Vec<Box<dyn Fn(&blawktrust::Column) -> blawktrust::Column + Sync>> = vec![
            Box::new(|c| dlog_column(c, 1)),
            Box::new(|c| c.clone()), // Identity
        ];

        for (i, op) in ops.iter().enumerate() {
            let result = map_numeric_preserve_tags(&frame, |c| op(c));

            assert!(
                Arc::ptr_eq(&frame.tags.index, &result.tags.index),
                "Op {}: I1 violated - index Arc not preserved",
                i
            );
            assert!(
                Arc::ptr_eq(&frame.tags.colnames, &result.tags.colnames),
                "Op {}: I2 violated - colnames Arc not preserved",
                i
            );
            assert_eq!(
                frame.nrows(),
                result.nrows(),
                "Op {}: I3 violated - row count changed",
                i
            );
        }
    }

    // ==================== ASOFR PROPERTY TESTS (Semantic Tripwires) ====================

    #[test]
    fn property_asofr_identity_when_indices_match() {
        // Property: If x.index == y.index, asofr(x,y) == mapr(x,y) == x

        let index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002]));
        let x_tags = Tags::new("DATE".to_string(), index.clone(), vec!["price".to_string()]);
        let x_col = Arc::new(blawktrust::Column::new_f64(vec![100.0, 101.0, 102.0]));
        let x = Frame::new(x_tags, vec![x_col]);

        let y_tags = Tags::new("DATE".to_string(), index, vec!["dummy".to_string()]);
        let y_col = Arc::new(blawktrust::Column::new_f64(vec![1.0, 2.0, 3.0]));
        let y = Frame::new(y_tags, vec![y_col]);

        let asof_result = asofr(&x, &y);
        let mapr_result = reindex_by(&x, Arc::clone(&y.tags.index));

        // Should be numerically identical
        assert_eq!(asof_result.nrows(), mapr_result.nrows());
        let asof_col = asof_result.get_col(0).unwrap();
        let mapr_col = mapr_result.get_col(0).unwrap();

        match (&**asof_col, &**mapr_col) {
            (blawktrust::Column::F64(a), blawktrust::Column::F64(m)) => {
                for i in 0..a.len() {
                    assert_eq!(
                        a[i], m[i],
                        "Row {}: asofr should equal mapr when indices match",
                        i
                    );
                }
            }
            _ => panic!("Expected F64 columns"),
        }
    }

    #[test]
    fn property_asofr_no_forward_looking_bias() {
        // Property (STRONG): asofr NEVER uses future x values
        // Construct x with a "future spike" - it must never appear at earlier y times

        // x: [18000: 100, 18005: 999]  ← future spike at 18005
        let x_index = IndexColumn::Date(Arc::new(vec![18000, 18005]));
        let x_tags = Tags::new("DATE".to_string(), x_index, vec!["price".to_string()]);
        let x_col = Arc::new(blawktrust::Column::new_f64(vec![100.0, 999.0]));
        let x = Frame::new(x_tags, vec![x_col]);

        // y: [18000, 18001, 18002, 18003, 18004, 18005, 18006]
        let y_index = IndexColumn::Date(Arc::new(vec![
            18000, 18001, 18002, 18003, 18004, 18005, 18006,
        ]));
        let y_tags = Tags::new("DATE".to_string(), y_index, vec!["dummy".to_string()]);
        let y_col = Arc::new(blawktrust::Column::new_f64(vec![1.0; 7]));
        let y = Frame::new(y_tags, vec![y_col]);

        let result = asofr(&x, &y);

        let result_col = result.get_col(0).unwrap();
        match &**result_col {
            blawktrust::Column::F64(data) => {
                assert_eq!(data[0], 100.0, "t=18000: exact match");
                assert_eq!(data[1], 100.0, "t=18001: carry 100 (NOT 999!)");
                assert_eq!(data[2], 100.0, "t=18002: carry 100 (NOT 999!)");
                assert_eq!(data[3], 100.0, "t=18003: carry 100 (NOT 999!)");
                assert_eq!(data[4], 100.0, "t=18004: carry 100 (NOT 999!)");
                assert_eq!(data[5], 999.0, "t=18005: now 999 is valid");
                assert_eq!(data[6], 999.0, "t=18006: carry 999");

                // CRITICAL: 999 never appears before 18005
                for i in 1..5 {
                    assert_ne!(
                        data[i], 999.0,
                        "BIAS VIOLATION: Future value leaked to row {}",
                        i
                    );
                }
            }
            _ => panic!("Expected F64 column"),
        }
    }

    #[test]
    fn property_asofr_monotonicity() {
        // Property: For sorted y, selected x pointer is monotone nondecreasing

        // x: [18000: 100, 18002: 102, 18005: 105]
        let x_index = IndexColumn::Date(Arc::new(vec![18000, 18002, 18005]));
        let x_tags = Tags::new("DATE".to_string(), x_index, vec!["price".to_string()]);
        let x_col = Arc::new(blawktrust::Column::new_f64(vec![100.0, 102.0, 105.0]));
        let x = Frame::new(x_tags, vec![x_col]);

        // y: sorted [18000, 18001, 18002, 18003, 18004, 18005]
        let y_index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002, 18003, 18004, 18005]));
        let y_tags = Tags::new("DATE".to_string(), y_index, vec!["dummy".to_string()]);
        let y_col = Arc::new(blawktrust::Column::new_f64(vec![1.0; 6]));
        let y = Frame::new(y_tags, vec![y_col]);

        let result = asofr(&x, &y);

        let result_col = result.get_col(0).unwrap();
        match &**result_col {
            blawktrust::Column::F64(data) => {
                // Expected: [100, 100, 102, 102, 102, 105]
                // Monotone nondecreasing
                for i in 1..data.len() {
                    if !data[i - 1].is_nan() && !data[i].is_nan() {
                        assert!(
                            data[i - 1] <= data[i],
                            "Monotonicity violated: data[{}]={} > data[{}]={}",
                            i - 1,
                            data[i - 1],
                            i,
                            data[i]
                        );
                    }
                }
            }
            _ => panic!("Expected F64 column"),
        }
    }

    #[test]
    fn property_asofr_idempotence() {
        // Property: asofr(asofr(x,y), y) == asofr(x,y)

        let x_index = IndexColumn::Date(Arc::new(vec![18000, 18002]));
        let x_tags = Tags::new("DATE".to_string(), x_index, vec!["price".to_string()]);
        let x_col = Arc::new(blawktrust::Column::new_f64(vec![100.0, 102.0]));
        let x = Frame::new(x_tags, vec![x_col]);

        let y_index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002, 18003]));
        let y_tags = Tags::new("DATE".to_string(), y_index, vec!["dummy".to_string()]);
        let y_col = Arc::new(blawktrust::Column::new_f64(vec![1.0; 4]));
        let y = Frame::new(y_tags, vec![y_col]);

        let once = asofr(&x, &y);
        let twice = asofr(&once, &y);

        // Numeric equality
        assert_eq!(once.nrows(), twice.nrows());
        let once_col = once.get_col(0).unwrap();
        let twice_col = twice.get_col(0).unwrap();

        match (&**once_col, &**twice_col) {
            (blawktrust::Column::F64(d1), blawktrust::Column::F64(d2)) => {
                for i in 0..d1.len() {
                    if d1[i].is_nan() && d2[i].is_nan() {
                        continue;
                    }
                    assert_eq!(d1[i], d2[i], "Idempotence: row {}", i);
                }
            }
            _ => panic!("Expected F64 columns"),
        }
    }

    #[test]
    fn property_asofr_equivalence_to_naive() {
        // Property: Optimized asofr equals naive O(n²) scan
        // This catches edge cases in the two-pointer merge

        let x_index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18003, 18003, 18005]));
        let x_tags = Tags::new("DATE".to_string(), x_index, vec!["price".to_string()]);
        let x_col = Arc::new(blawktrust::Column::new_f64(vec![
            100.0, 101.0, 103.0, 103.5, 105.0,
        ]));
        let x = Frame::new(x_tags, vec![x_col]);

        let y_index = IndexColumn::Date(Arc::new(vec![17999, 18000, 18002, 18003, 18004, 18006]));
        let y_tags = Tags::new("DATE".to_string(), y_index, vec!["dummy".to_string()]);
        let y_col = Arc::new(blawktrust::Column::new_f64(vec![1.0; 6]));
        let y = Frame::new(y_tags, vec![y_col]);

        // Optimized version (uses two-pointer merge since both sorted)
        let optimized = asofr(&x, &y);

        // Naive version (uses fallback)
        let naive = asofr_fallback(&x, &y);

        // Should be identical
        assert_eq!(optimized.nrows(), naive.nrows());
        let opt_col = optimized.get_col(0).unwrap();
        let naive_col = naive.get_col(0).unwrap();

        match (&**opt_col, &**naive_col) {
            (blawktrust::Column::F64(o), blawktrust::Column::F64(n)) => {
                for i in 0..o.len() {
                    if o[i].is_nan() && n[i].is_nan() {
                        continue;
                    }
                    assert_eq!(o[i], n[i], "Row {}: optimized != naive (edge case)", i);
                }
            }
            _ => panic!("Expected F64 columns"),
        }
    }

    #[test]
    fn test_asofr_basic_carry_forward() {
        // Functional test: basic carry-forward behavior

        // x: [18000: 100, 18003: 103]
        let x_index = IndexColumn::Date(Arc::new(vec![18000, 18003]));
        let x_tags = Tags::new("DATE".to_string(), x_index, vec!["price".to_string()]);
        let x_col = Arc::new(blawktrust::Column::new_f64(vec![100.0, 103.0]));
        let x = Frame::new(x_tags, vec![x_col]);

        // y: [18000, 18001, 18002, 18003, 18004]
        let y_index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002, 18003, 18004]));
        let y_tags = Tags::new("DATE".to_string(), y_index, vec!["dummy".to_string()]);
        let y_col = Arc::new(blawktrust::Column::new_f64(vec![1.0; 5]));
        let y = Frame::new(y_tags, vec![y_col]);

        let result = asofr(&x, &y);

        let result_col = result.get_col(0).unwrap();
        match &**result_col {
            blawktrust::Column::F64(data) => {
                // Expected: [100, 100, 100, 103, 103]
                assert_eq!(data[0], 100.0, "18000: exact");
                assert_eq!(data[1], 100.0, "18001: carry 100");
                assert_eq!(data[2], 100.0, "18002: carry 100");
                assert_eq!(data[3], 103.0, "18003: exact");
                assert_eq!(data[4], 103.0, "18004: carry 103");
            }
            _ => panic!("Expected F64 column"),
        }
    }

    #[test]
    fn test_asofr_no_past_values() {
        // Functional test: y has times before all x values → NA

        // x: [18005: 105, 18006: 106]
        let x_index = IndexColumn::Date(Arc::new(vec![18005, 18006]));
        let x_tags = Tags::new("DATE".to_string(), x_index, vec!["price".to_string()]);
        let x_col = Arc::new(blawktrust::Column::new_f64(vec![105.0, 106.0]));
        let x = Frame::new(x_tags, vec![x_col]);

        // y: [18000, 18001, 18002, 18005]
        let y_index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002, 18005]));
        let y_tags = Tags::new("DATE".to_string(), y_index, vec!["dummy".to_string()]);
        let y_col = Arc::new(blawktrust::Column::new_f64(vec![1.0; 4]));
        let y = Frame::new(y_tags, vec![y_col]);

        let result = asofr(&x, &y);

        let result_col = result.get_col(0).unwrap();
        match &**result_col {
            blawktrust::Column::F64(data) => {
                // Expected: [NA, NA, NA, 105]
                assert!(data[0].is_nan(), "18000: no past x → NA");
                assert!(data[1].is_nan(), "18001: no past x → NA");
                assert!(data[2].is_nan(), "18002: no past x → NA");
                assert_eq!(data[3], 105.0, "18005: exact match");
            }
            _ => panic!("Expected F64 column"),
        }
    }
}
