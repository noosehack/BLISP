//! BLADE Frame: Tags + Numeric Payload (P2 Policy)
//!
//! Implements BLISP_BLADE_Blueprint.txt Phase 1 & 2:
//! - Tags: index + colnames (Arc-shared, zero-copy)
//! - Frame: tags + numeric columns
//! - map_numeric_preserve_tags: core primitive (Phase 1)
//! - reindex_by: alignment primitive (Phase 2)

use std::sync::Arc;
use std::collections::HashMap;

/// Index column: Date, Timestamp, or String rownames
#[derive(Debug, Clone)]
pub enum IndexColumn {
    Date(Arc<Vec<i32>>),         // Days since epoch
    Timestamp(Arc<Vec<i64>>),    // Nanoseconds since epoch
    String(Arc<Vec<String>>),    // Generic rownames
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

/// Tags: metadata carried by Arc (P2 policy: index + colnames only)
#[derive(Debug, Clone)]
pub struct Tags {
    pub index_name: String,              // e.g., "DATE", "TIMESTAMP"
    pub index: Arc<IndexColumn>,         // Row identifiers
    pub colnames: Arc<Vec<String>>,      // Numeric column names (in order)
}

impl Tags {
    /// Create new tags
    pub fn new(index_name: String, index: IndexColumn, colnames: Vec<String>) -> Self {
        Self {
            index_name,
            index: Arc::new(index),
            colnames: Arc::new(colnames),
        }
    }

    /// Clone tags (Arc clone = pointer copy only, O(1))
    #[inline]
    pub fn clone_arc(&self) -> Self {
        Self {
            index_name: self.index_name.clone(),
            index: Arc::clone(&self.index),
            colnames: Arc::clone(&self.colnames),
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
    pub tags: Arc<Tags>,        // Shared metadata (zero-copy)
    pub cols: Vec<ColData>,     // Numeric columns (same order as tags.colnames)
    pub nrows: usize,           // Cached
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

        Self { tags, cols: col_data, nrows }
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
    F: Fn(&blawktrust::Column) -> blawktrust::Column,
{
    // Arc clone tags (pointer copy only, O(1))
    let tags_out = Arc::clone(&frame.tags);

    // Transform each numeric column
    let cols_out: Vec<ColData> = frame.cols.iter()
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
pub fn reindex_by(source: &Frame, target_index: &IndexColumn) -> Frame {
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
    let out_tags = Tags {
        index_name: source.tags.index_name.clone(), // Could be improved: take from target
        index: Arc::new(target_index.clone()),      // TODO: Accept Arc directly to avoid clone
        colnames: Arc::clone(&source.tags.colnames), // Arc reused!
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
        assert!(Arc::ptr_eq(&frame.tags.index, &result.tags.index),
            "I1 VIOLATED: Index Arc not preserved");

        // I2: Same colnames Arc
        assert!(Arc::ptr_eq(&frame.tags.colnames, &result.tags.colnames),
            "I2 VIOLATED: Colnames Arc not preserved");

        // I3: Same nrows
        assert_eq!(frame.nrows(), result.nrows(),
            "I3 VIOLATED: Row count changed");
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
        assert!(Arc::ptr_eq(&frame.tags.index, &result.tags.index),
            "I1 VIOLATED: dlog didn't preserve index Arc");

        // I2: Same colnames Arc (pointer equality)
        assert!(Arc::ptr_eq(&frame.tags.colnames, &result.tags.colnames),
            "I2 VIOLATED: dlog didn't preserve colnames Arc");

        // I3: Same nrows
        assert_eq!(frame.nrows(), result.nrows(),
            "I3 VIOLATED: dlog changed row count");

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

        let result = reindex_by(&source, &target_index);

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
        assert!(Arc::ptr_eq(&source.tags.colnames, &result.tags.colnames),
            "reindex_by should preserve colnames Arc");
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

        let result = reindex_by(&source, &target_index);

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

        let result = reindex_by(&source, &target_index);

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

        let result = reindex_by(&source, &target_index);

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
        let once = reindex_by(&x, &y.tags.index);
        // Second application (should be identical)
        let twice = reindex_by(&once, &y.tags.index);

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
        assert!(Arc::ptr_eq(&once.tags.colnames, &twice.tags.colnames),
            "Idempotence: colnames Arc must be preserved");
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
        let result = reindex_by(&x, &index);

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

        let result = reindex_by(&x, &y_index);

        assert_eq!(result.nrows(), y_index.len(),
            "Monotonicity: output rows must equal target rows");
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
        let result = reindex_by(&x, &y_index);

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
                        assert!(val == 100.0 || val == 102.0,
                            "Non-NA value {} not from source x", val);
                    }
                }
            }
            _ => panic!("Expected F64 column"),
        }
    }

    #[test]
    fn property_arc_preservation_numeric_ops() {
        // Property: map_numeric_preserve_tags MUST preserve tag Arcs (I1-I2)

        use blawktrust::builtins::ops::dlog_column;

        let index = IndexColumn::Date(Arc::new(vec![18000, 18001, 18002]));
        let tags = Tags::new("DATE".to_string(), index, vec!["price".to_string()]);
        let col = Arc::new(blawktrust::Column::new_f64(vec![100.0, 101.0, 102.0]));
        let frame = Frame::new(tags, vec![col]);

        // Apply various operations
        let ops: Vec<Box<dyn Fn(&blawktrust::Column) -> blawktrust::Column>> = vec![
            Box::new(|c| dlog_column(c, 1)),
            Box::new(|c| c.clone()), // Identity
        ];

        for (i, op) in ops.iter().enumerate() {
            let result = map_numeric_preserve_tags(&frame, op);

            assert!(Arc::ptr_eq(&frame.tags.index, &result.tags.index),
                "Op {}: I1 violated - index Arc not preserved", i);
            assert!(Arc::ptr_eq(&frame.tags.colnames, &result.tags.colnames),
                "Op {}: I2 violated - colnames Arc not preserved", i);
            assert_eq!(frame.nrows(), result.nrows(),
                "Op {}: I3 violated - row count changed", i);
        }
    }
}
