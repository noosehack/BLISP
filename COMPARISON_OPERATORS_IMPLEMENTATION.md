# Comparison Operators IR Extension

**Type**: Canonical IR Extension (NOT alias migration)
**Semantic Requirement**: Match existing builtin behavior exactly

---

## Current State

### BinaryFunc enum (src/ir.rs)
```rust
pub enum BinaryFunc {
    ADD,
    SUB,
    MUL,
    DIV,
    GTR,  // Only one comparison operator
}
```

### Builtin Behavior (verified in src/builtins.rs)

All comparison builtins:
- Return **numeric** 1.0 (true) or 0.0 (false), NOT boolean
- Support scalar × scalar, Column × scalar, Column × Column
- Handle Int/Float type coercion
- Propagate NA (comparison with NA returns NA)
- Use helper functions:
  - `compare_column_scalar(col, scalar, closure)`
  - `compare_columns(col1, col2, closure)`

Example from builtin_lt:
```rust
(Value::Col(col), Value::Float(f)) => {
    let result = compare_column_scalar(col, *f, |a, b| a < b)?;
    Ok(Value::Col(Arc::new(result)))
}
```

---

## Required Extension

### 1. Extend BinaryFunc enum (src/ir.rs)

**Location**: After GTR (around line 300)

```rust
pub enum BinaryFunc {
    /// Addition
    ADD,
    /// Subtraction
    SUB,
    /// Multiplication
    MUL,
    /// Division
    DIV,
    /// Greater than: x > y → 1.0 (true), 0.0 (false), NA (if either is NA)
    GTR,
    /// Less than: x < y → 1.0 (true), 0.0 (false), NA (if either is NA)
    LSS,
    /// Less than or equal: x <= y → 1.0 (true), 0.0 (false), NA (if either is NA)
    LTE,
    /// Greater than or equal: x >= y → 1.0 (true), 0.0 (false), NA (if either is NA)
    GTE,
    /// Equal: x == y → 1.0 (true), 0.0 (false), NA (if either is NA)
    EQL,
    /// Not equal: x != y → 1.0 (true), 0.0 (false), NA (if either is NA)
    NEQ,
}
```

### 2. Add Planner Mappings (src/planner.rs)

**Location**: After ">" mapping (around line 524)

```rust
// Binary operations
"+" => plan_binary(BinaryFunc::ADD, &elements[1..], plan, ctx, interner),
"-" => plan_binary(BinaryFunc::SUB, &elements[1..], plan, ctx, interner),
"*" => plan_binary(BinaryFunc::MUL, &elements[1..], plan, ctx, interner),
"/" => plan_binary(BinaryFunc::DIV, &elements[1..], plan, ctx, interner),
">" => plan_binary(BinaryFunc::GTR, &elements[1..], plan, ctx, interner),

// Comparison operators (canonical IR extension)
"<" => plan_binary(BinaryFunc::LSS, &elements[1..], plan, ctx, interner),
"<=" => plan_binary(BinaryFunc::LTE, &elements[1..], plan, ctx, interner),
">=" => plan_binary(BinaryFunc::GTE, &elements[1..], plan, ctx, interner),
"==" => plan_binary(BinaryFunc::EQL, &elements[1..], plan, ctx, interner),
"!=" => plan_binary(BinaryFunc::NEQ, &elements[1..], plan, ctx, interner),
```

### 3. Extend Executor (src/exec.rs)

**Location**: In apply_binary_scalar and apply_binary_columns functions

#### For Column × Scalar (apply_binary_scalar)

**Current location**: Around line 350, in match statement

```rust
fn apply_binary_scalar(col: &Column, scalar: f64, func: BinaryFunc) -> Column {
    match col {
        Column::F64(data) => {
            let result: Vec<f64> = data.iter().map(|&x| {
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
                            if x > scalar { 1.0 } else { 0.0 }
                        }
                        // ADD THESE:
                        BinaryFunc::LSS => {
                            if x < scalar { 1.0 } else { 0.0 }
                        }
                        BinaryFunc::LTE => {
                            if x <= scalar { 1.0 } else { 0.0 }
                        }
                        BinaryFunc::GTE => {
                            if x >= scalar { 1.0 } else { 0.0 }
                        }
                        BinaryFunc::EQL => {
                            if x == scalar { 1.0 } else { 0.0 }
                        }
                        BinaryFunc::NEQ => {
                            if x != scalar { 1.0 } else { 0.0 }
                        }
                    }
                }
            }).collect();
            Column::F64(result)
        }
        _ => col.clone(),
    }
}
```

#### For Column × Column (apply_binary_columns)

**Current location**: Around line 380, in match statement

```rust
fn apply_binary_columns(lhs: &Column, rhs: &Column, func: BinaryFunc) -> Result<Column, String> {
    match (lhs, rhs) {
        (Column::F64(left_data), Column::F64(right_data)) => {
            if left_data.len() != right_data.len() {
                return Err(format!("Cannot apply binary op: length mismatch ({} vs {})",
                    left_data.len(), right_data.len()));
            }

            let result: Vec<f64> = left_data.iter().zip(right_data.iter()).map(|(&x, &y)| {
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
                            if x > y { 1.0 } else { 0.0 }
                        }
                        // ADD THESE:
                        BinaryFunc::LSS => {
                            if x < y { 1.0 } else { 0.0 }
                        }
                        BinaryFunc::LTE => {
                            if x <= y { 1.0 } else { 0.0 }
                        }
                        BinaryFunc::GTE => {
                            if x >= y { 1.0 } else { 0.0 }
                        }
                        BinaryFunc::EQL => {
                            if x == y { 1.0 } else { 0.0 }
                        }
                        BinaryFunc::NEQ => {
                            if x != y { 1.0 } else { 0.0 }
                        }
                    }
                }
            }).collect();

            Ok(Column::F64(result))
        }
        _ => Err("Binary op requires F64 columns".to_string()),
    }
}
```

---

## Semantic Verification Checklist

Before implementing, verify these match builtin behavior:

✅ **Return type**: Numeric (1.0/0.0), not boolean
✅ **NA propagation**: NA input → NA output
✅ **Type coercion**: Handled at planner level (same as GTR)
✅ **Scalar broadcast**: Column × Scalar supported
✅ **Column pairs**: Column × Column supported
✅ **Float comparison**: Uses standard f64 comparison

---

## Test Cases After Implementation

```lisp
# Simple scalar comparison
(< 5 10)  → 1.0

# Column × Scalar
(< (col df "PRC") 100)  → Column of 1.0/0.0

# Column × Column
(< (col df "PRC") (col df "VOL"))  → Column of 1.0/0.0

# Nested in IR tree (double-fail elimination)
(dlog (< (col df "PRC") 100))  → SUCCESS (no Unknown function)
(shift 1 (>= (col df "VOL") 1000000))  → SUCCESS
(cs1 (== (col df "SECTOR") "TECH"))  → SUCCESS (if string handling added)
```

---

## Implementation Order

1. **ir.rs**: Add 5 BinaryFunc variants
2. **planner.rs**: Add 5 planner mappings
3. **exec.rs**: Add 5 match arms in apply_binary_scalar
4. **exec.rs**: Add 5 match arms in apply_binary_columns
5. **Build**: cargo build
6. **Test**: Verify expressions don't double-fail

---

## Why This is NOT an Alias Migration

| Aspect | Alias (Level 1) | Canonical Extension (This) |
|--------|-----------------|---------------------------|
| **Purpose** | Backward compat for legacy names | Missing IR functionality |
| **Semantic** | Duplicate of existing op | New distinct operation |
| **IR enum** | Reuses existing variant | Adds new variant |
| **Deprecation** | YES (emit warning) | NO (canonical name) |
| **Example** | w5 → wkd, dlog-col → dlog | < → LSS, == → EQL |

---

## Architectural Impact

**Before**:
- IR comparisons: Only `>` (GTR)
- Missing: `<`, `<=`, `>=`, `==`, `!=`
- Expressions like `(dlog (< PRC 100))` → double-fail

**After**:
- IR comparisons: Complete set (GTR, LSS, LTE, GTE, EQL, NEQ)
- All comparison predicates work in IR trees
- No double-fail for nested comparisons
- Never touches legacy fallback in HYBRID mode

---

## Files to Modify

1. `src/ir.rs` - BinaryFunc enum (+6 variants)
2. `src/planner.rs` - Token mappings (+5 match arms)
3. `src/exec.rs` - Executor logic (+10 match arms total, 5 per function)

**Total changes**: ~50 lines across 3 files

---

## Ready to Implement

This is the minimal consistent extension that:
- Matches existing builtin semantics exactly
- Follows established GTR pattern
- Completes IR comparison operator coverage
- Eliminates double-fail for comparison predicates

**Next**: Apply these changes and test.
