//! BLADE Frame: Tags + Numeric Payload (P2 Policy)
//!
//! Implements BLISP_BLADE_Blueprint.txt Phase 1:
//! - Tags: index + colnames (Arc-shared, zero-copy)
//! - Frame: tags + numeric columns
//! - map_numeric_preserve_tags: core primitive

use std::sync::Arc;

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
}
