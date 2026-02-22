# Current BLISP Structures (for mask implementation)

## Dependencies
- **bitvec**: NOT currently in Cargo.toml - needs to be added

## Tags Structure (src/frame.rs, ~line 70)
```rust
#[derive(Debug, Clone)]
pub struct Tags {
    pub index_name: String,              // e.g., "DATE", "TIMESTAMP"
    pub index: Arc<IndexColumn>,         // Row identifiers
    pub colnames: Arc<Vec<String>>,      // Numeric column names (in order)
}

impl Tags {
    pub fn new(index_name: String, index: IndexColumn, colnames: Vec<String>) -> Self {
        Self {
            index_name,
            index: Arc::new(index),
            colnames: Arc::new(colnames),
        }
    }

    #[inline]
    pub fn clone_arc(&self) -> Self {
        Self {
            index_name: self.index_name.clone(),
            index: Arc::clone(&self.index),
            colnames: Arc::clone(&self.colnames),
        }
    }

    pub fn nrows(&self) -> usize {
        self.index.len()
    }
}
```

## Frame Structure (src/frame.rs, ~line 95)
```rust
#[derive(Debug, Clone)]
pub struct Frame {
    pub tags: Arc<Tags>,        // Shared metadata (zero-copy)
    pub cols: Vec<ColData>,     // Numeric columns (same order as tags.colnames)
    pub nrows: usize,           // Cached
}

impl Frame {
    pub fn new(tags: Tags, cols: Vec<Arc<blawktrust::Column>>) -> Self {
        let nrows = tags.nrows();
        let col_data = cols.into_iter().map(ColData::Mat).collect();
        Self {
            tags: Arc::new(tags),
            cols: col_data,
            nrows,
        }
    }

    pub fn with_tags(tags: Arc<Tags>, cols: Vec<Arc<blawktrust::Column>>) -> Self {
        let nrows = tags.nrows();
        let col_data = cols.into_iter().map(ColData::Mat).collect();
        Self { tags, cols: col_data, nrows }
    }
}
```

## IndexColumn (src/frame.rs)
```rust
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
}
```

## Rolling Kernel Signatures (src/exec.rs)

### rolling_mean_column (line 769, strict version)
```rust
fn rolling_mean_column(col: &Column, w: usize) -> Column {
    match col {
        Column::F64(data) => {
            let nrows = data.len();
            let mut result = vec![f64::NAN; nrows];

            if w > nrows {
                return Column::F64(result);
            }

            let mut running_sum = 0.0;
            let mut valid_count = 0usize;

            // Single pass: maintain sliding window [i-w+1 .. i]
            for i in 0..nrows {
                // Add entering value at position i
                if !data[i].is_nan() {
                    running_sum += data[i];
                    valid_count += 1;
                }

                // Remove leaving value at position i-w
                if i >= w {
                    let leaving_idx = i - w;
                    if !data[leaving_idx].is_nan() {
                        running_sum -= data[leaving_idx];
                        valid_count -= 1;
                    }
                }

                // Emit result if window full AND has w valid values (strict)
                if i >= w - 1 && valid_count >= w {
                    result[i] = running_sum / (valid_count as f64);
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}
```

### rolling_mean_partial (line 902, partial version)
```rust
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

                // Emit if window position reached AND min 2 valid (partial)
                if i >= w - 1 && valid_count >= 2 {
                    result[i] = running_sum / (valid_count as f64);
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}
```

### rolling_std_column (similar structure)
```rust
fn rolling_std_column(col: &Column, w: usize) -> Column {
    // Similar structure with:
    // - running_sum and running_sumsq for variance calculation
    // - Strict condition: valid_count >= w
    // - Formula: variance = (sumsq/w) - mean²
}
```

## Key Files
- `src/frame.rs` - Tags, Frame, IndexColumn definitions
- `src/exec.rs` - Rolling kernels (mean, std, partial variants)
- `src/builtins.rs` - Builtin functions (where mask-weekend, with-mask would go)
- `src/ir.rs` - NumericFunc enum (operations)
- `Cargo.toml` - Dependencies

## Current WKD Implementation (src/exec.rs, line 640)
```rust
fn w5_mask_weekends(frame: &Frame) -> Result<Arc<Frame>, String> {
    // Computes weekend_mask: Vec<bool> from index dates
    // Sets weekend positions to f64::NAN
    // Returns new Frame with masked columns
    // NO distinction between masked NA vs source NA currently
}
```

## Branch
- Current: `reconstruct/tableview-only`
- Latest commit: `f789c64` (Fix wzs to use partial rolling)

Ready for your skeleton patches!
