# Table Arithmetic Implementation Guide

## Status: Step-by-step tested, ready to implement

## What We Discovered

1. **clispi `wzscore`** = **blisp `wzs-ft-cols`** (both Ft-measurable) ✅ VERIFIED  
   - Both use window [i-w, i-1] (past-only)
   - Both produce identical output (tested on ES1I.csv)
   - First value at row `window` (e.g., row 25 for window=25)

2. **Column arithmetic exists** but incomplete:
   - `add_columns` ✅ + dispatch in `builtin_add` ✅
   - `mul_columns` ✅ + dispatch in `builtin_mul` ✅
   - `subtract_columns` ✅ but NO dispatch in `builtin_sub` ❌
   - `div_columns` ❌ MISSING + no dispatch ❌

3. **Table arithmetic** completely missing ❌

## Implementation Plan

### Step 1: Add missing imports

```rust
// At top of src/builtins.rs, modify existing import:
use blawktrust::{Column, Table, TableView};
```

### Step 2: Add div_columns helper

Add after `mul_columns` function (~line 2212):

```rust
/// Element-wise division of two columns (col / col)
fn div_columns(a: &Column, b: &Column) -> Result<Column, String> {
    if a.len() != b.len() {
        return Err(format!("Column length mismatch: {} vs {}", a.len(), b.len()));
    }

    match (a, b) {
        (Column::F64(a_data), Column::F64(b_data)) => {
            // IEEE 754: x/0 = inf, 0/0 = NaN, NaN/x = NaN
            // No special casing - let hardware handle it
            let result: Vec<f64> = a_data.iter().zip(b_data.iter())
                .map(|(x, y)| x / y)
                .collect();
            Ok(Column::new_f64(result))
        }
        _ => Err("Column division only supported for F64 columns".to_string()),
    }
}
```

### Step 3: Add Col-Col cases to builtins

**In `builtin_sub` (~line 204):** Add before the final `_ => Err` line:

```rust
        // Col - Col
        (Value::Col(a), Value::Col(b)) => {
            let result = subtract_columns(a, b)?;
            Ok(Value::Col(Arc::new(result)))
        }
```

**In `builtin_div` (~line 310):** Add before the final `_ => Err` line:

```rust
        // Col / Col
        (Value::Col(a), Value::Col(b)) => {
            let result = div_columns(a, b)?;
            Ok(Value::Col(Arc::new(result)))
        }
```

### Step 4: Add table arithmetic helpers

Add at end of file (after all other helpers):

```rust
// ============================================================================
// Table Arithmetic Helpers  
// ============================================================================

fn add_tables(a: &TableView, b: &TableView) -> Result<TableView, String> {
    let a_rows = if !a.table.columns.is_empty() { a.table.columns[0].len() } else { 0 };
    let b_rows = if !b.table.columns.is_empty() { b.table.columns[0].len() } else { 0 };
    if a_rows != b_rows {
        return Err(format!("Table row count mismatch: {} vs {}", a_rows, b_rows));
    }
    if a.table.columns.len() != b.table.columns.len() {
        return Err(format!("Table column count mismatch: {} vs {}", a.table.columns.len(), b.table.columns.len()));
    }
    let mut result_cols = Vec::new();
    for (i, (col_a, col_b)) in a.table.columns.iter().zip(b.table.columns.iter()).enumerate() {
        let result_col = add_columns(col_a, col_b)
            .map_err(|e| format!("Column {} ({}): {}", i, a.table.names.get(i).unwrap_or(&"?".to_string()), e))?;
        result_cols.push(result_col);
    }
    Ok(TableView {
        table: Arc::new(Table {
            names: a.table.names.clone(),
            columns: result_cols,
        }),
    })
}

fn subtract_tables(a: &TableView, b: &TableView) -> Result<TableView, String> {
    let a_rows = if !a.table.columns.is_empty() { a.table.columns[0].len() } else { 0 };
    let b_rows = if !b.table.columns.is_empty() { b.table.columns[0].len() } else { 0 };
    if a_rows != b_rows {
        return Err(format!("Table row count mismatch: {} vs {}", a_rows, b_rows));
    }
    if a.table.columns.len() != b.table.columns.len() {
        return Err(format!("Table column count mismatch: {} vs {}", a.table.columns.len(), b.table.columns.len()));
    }
    let mut result_cols = Vec::new();
    for (i, (col_a, col_b)) in a.table.columns.iter().zip(b.table.columns.iter()).enumerate() {
        let result_col = subtract_columns(col_a, col_b)
            .map_err(|e| format!("Column {} ({}): {}", i, a.table.names.get(i).unwrap_or(&"?".to_string()), e))?;
        result_cols.push(result_col);
    }
    Ok(TableView {
        table: Arc::new(Table {
            names: a.table.names.clone(),
            columns: result_cols,
        }),
    })
}

fn mul_tables(a: &TableView, b: &TableView) -> Result<TableView, String> {
    let a_rows = if !a.table.columns.is_empty() { a.table.columns[0].len() } else { 0 };
    let b_rows = if !b.table.columns.is_empty() { b.table.columns[0].len() } else { 0 };
    if a_rows != b_rows {
        return Err(format!("Table row count mismatch: {} vs {}", a_rows, b_rows));
    }
    if a.table.columns.len() != b.table.columns.len() {
        return Err(format!("Table column count mismatch: {} vs {}", a.table.columns.len(), b.table.columns.len()));
    }
    let mut result_cols = Vec::new();
    for (i, (col_a, col_b)) in a.table.columns.iter().zip(b.table.columns.iter()).enumerate() {
        let result_col = mul_columns(col_a, col_b)
            .map_err(|e| format!("Column {} ({}): {}", i, a.table.names.get(i).unwrap_or(&"?".to_string()), e))?;
        result_cols.push(result_col);
    }
    Ok(TableView {
        table: Arc::new(Table {
            names: a.table.names.clone(),
            columns: result_cols,
        }),
    })
}

fn div_tables(a: &TableView, b: &TableView) -> Result<TableView, String> {
    let a_rows = if !a.table.columns.is_empty() { a.table.columns[0].len() } else { 0 };
    let b_rows = if !b.table.columns.is_empty() { b.table.columns[0].len() } else { 0 };
    if a_rows != b_rows {
        return Err(format!("Table row count mismatch: {} vs {}", a_rows, b_rows));
    }
    if a.table.columns.len() != b.table.columns.len() {
        return Err(format!("Table column count mismatch: {} vs {}", a.table.columns.len(), b.table.columns.len()));
    }
    let mut result_cols = Vec::new();
    for (i, (col_a, col_b)) in a.table.columns.iter().zip(b.table.columns.iter()).enumerate() {
        let result_col = div_columns(col_a, col_b)
            .map_err(|e| format!("Column {} ({}): {}", i, a.table.names.get(i).unwrap_or(&"?".to_string()), e))?;
        result_cols.push(result_col);
    }
    Ok(TableView {
        table: Arc::new(Table {
            names: a.table.names.clone(),
            columns: result_cols,
        }),
    })
}
```

### Step 5: Add TableView-TableView dispatch to builtins

**Add to each of the four builtins before final `_ => Err` line:**

```rust
// In builtin_add:
        // TableView + TableView
        (Value::TableView(a), Value::TableView(b)) => {
            let result = add_tables(a, b)?;
            Ok(Value::TableView(Arc::new(result)))
        }

// In builtin_sub:
        // TableView - TableView
        (Value::TableView(a), Value::TableView(b)) => {
            let result = subtract_tables(a, b)?;
            Ok(Value::TableView(Arc::new(result)))
        }

// In builtin_mul:
        // TableView * TableView
        (Value::TableView(a), Value::TableView(b)) => {
            let result = mul_tables(a, b)?;
            Ok(Value::TableView(Arc::new(result)))
        }

// In builtin_div:
        // TableView / TableView
        (Value::TableView(a), Value::TableView(b)) => {
            let result = div_tables(a, b)?;
            Ok(Value::TableView(Arc::new(result)))
        }
```

## Testing

After implementation, test with:

```bash
cd /home/ubuntu && /home/ubuntu/blisp/target/release/blisp -e '
(let* ((data (file "/home/ubuntu/ES1I.csv"))
       (w5d (WKD data))
       (ret (dlog-cols w5d 1))
       (mean (ft-wmean-cols ret 25))
       (std (ft-wstd-cols ret 25))
       (diff (- ret mean))
       (zscore (/ diff std)))
  (save "/tmp/test_table_zscore.csv" zscore))
' && head -30 /tmp/test_table_zscore.csv
```

Expected: First 25 rows NaN, then z-score values matching `wzs-ft-cols` output.

## Next Steps (Optional)

After table arithmetic works, optionally add `ft-wzs-cols` optimized builtin that uses single-pass kernel for better performance.

