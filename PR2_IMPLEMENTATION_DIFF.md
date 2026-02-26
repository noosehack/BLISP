# PR2 Implementation Diff: dlog_column

**Date**: 2026-02-26
**Purpose**: Compare local vs blawktrust dlog_column before unification

---

## LOCAL Implementation (exec.rs:1092-1121)

```rust
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
```

**Characteristics**:
- **NA Policy**: Skip NAs, compute dlog between valid values
- **Lag Semantics**: NA-skipping (looks back to last valid)
- **Edge Case**: First valid value → NA
- **Negative Values**: prev ≤ 0 or x ≤ 0 → NA
- **Type Handling**: Non-F64 columns → clone unchanged

---

## BLAWKTRUST Implementation

**Location**: `/home/ubuntu/blawktrust/src/builtins/ops.rs`

[Reading implementation...]
