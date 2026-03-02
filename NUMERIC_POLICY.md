# Numeric Policy - IEEE-754 Standard

**Decision**: BLISP adopts **IEEE-754 standard behavior** for all floating-point edge cases.

**Date**: 2026-02-28
**Rationale**:
- Finance pipelines routinely hit zeros, tiny denominators, log-domain issues
- Hiding infinities behind NaN makes debugging harder
- IEEE-754 is standard across CPU math, Rust f64, and all numeric stacks
- **NA (missingness) ≠ inf/NaN (numeric concepts)** - keep them distinct

---

## Core Rules (IEEE-754)

### Division by Zero

| Operation | Result | Rationale |
|-----------|--------|-----------|
| `1.0 / 0.0` | `+inf` | Positive limit approaching zero |
| `-1.0 / 0.0` | `-inf` | Negative limit approaching zero |
| `0.0 / 0.0` | `NaN` | Indeterminate form |

### Logarithms

| Operation | Result | Rationale |
|-----------|--------|-----------|
| `log(0.0)` | `-inf` | Limit as x → 0⁺ |
| `log(x)` where x < 0 | `NaN` | Undefined in reals |
| `log(NaN)` | `NaN` | Propagates NaN |

### Log-Returns (dlog)

`dlog(x) = log(x[i]) - log(x[i-1])`

| Transition | Result | Rationale |
|------------|--------|-----------|
| `0 → positive` | `+inf` | log(pos/0) = log(pos) - log(0) = val - (-inf) = +inf |
| `positive → 0` | `-inf` | log(0/pos) = log(0) - log(pos) = -inf - val = -inf |
| `0 → 0` | `NaN` | log(0/0) = NaN |
| `negative → any` | `NaN` | log(negative) = NaN |
| `any → negative` | `NaN` | log(negative) = NaN |

### Other Operations

| Operation | Result |
|-----------|--------|
| `sqrt(x)` where x < 0 | `NaN` |
| `inf + inf` | `inf` |
| `inf - inf` | `NaN` |
| `inf * 0` | `NaN` |

---

## NA vs Numeric Edge Cases

**Concept distinction**:
- **NA**: Missing data (never observed, censored, unavailable)
- **NaN**: Indeterminate numeric result (0/0, sqrt(-1), inf-inf)
- **±inf**: Limit behavior (1/0, log(0), overflow)

**Handling**:
- NA is represented as `NaN` in f64 columns (implementation detail)
- But **semantically** they are different:
  - NA from source data propagates as NA
  - NaN from computation (0/0) is a **numeric result**, not missingness
- Operations must preserve this distinction where possible

**Example**:
```lisp
; Source has NA at position 5
(dlog [1.0, 2.0, 3.0, 0.0, NA, 5.0])
      [NA, log(2), log(1.5), -inf, NA, +inf]
       ^                      ^        ^
       prefix                 |        |
                        propagated  log(5/0)=+inf (numeric, not NA!)
```

---

## Test Policy

### Float Comparison

**For exact values** (non-edge):
- Use relative tolerance: `abs(a - b) / max(|a|, |b|) < 1e-10`
- Or ULP-based comparison (4 ULPs max)

**For edge cases**:
```rust
// NaN handling
if a.is_nan() && b.is_nan() {
    // Both NaN → equal (for testing)
    return true;
} else if a.is_nan() || b.is_nan() {
    // One NaN, one not → mismatch
    return false;
}

// Infinity handling
if a.is_infinite() && b.is_infinite() {
    // Must have same sign
    return a.signum() == b.signum();
}

// Finite values
abs(a - b) <= epsilon * max(abs(a), abs(b))
```

**Never use** `==` for floats except when comparing to exact literals (0.0, 1.0, etc.)

---

## Implementation Status

### AST (tests/common/mod.rs)
- ✅ Uses Rust f64 directly → inherits IEEE-754
- ✅ dlog: Returns -inf for log(0), +inf for division by zero
- ✅ No special coercion to NaN

### IR (src/exec.rs)
- ⚠️ **WAS**: Some operations coerced inf → NaN
- ✅ **NOW**: Match AST, preserve IEEE-754

### Operations Affected
1. `dlog_obs_column` - Must preserve -inf/+inf from log(0)
2. `map_numeric` on log/exp - Preserve IEEE-754
3. Division operations - Return ±inf, not NaN

---

## Migration Notes

**Before** (inconsistent):
```rust
// IR was doing this (WRONG):
if result.is_infinite() {
    result = f64::NAN;  // Coercion
}
```

**After** (IEEE-754):
```rust
// IR does this (CORRECT):
result  // Preserve inf as-is
```

**Test updates**:
- Removed `#[ignore]` from 3 differential_exec tests
- Updated comparators to handle inf/NaN correctly
- All AST ≡ IR now under IEEE-754 rules

---

## Why This Matters

### Financial Context

1. **Prices hitting zero**: Rare but real (bankruptcies, data errors)
   - `dlog` with zeros produces `-inf` → clear signal, not hidden as NaN
   - Downstream can filter `is_infinite()` if needed

2. **Tiny denominators**: Common in ratios, spreads
   - Overflow to `±inf` → detectable, debuggable
   - Alternative (coerce to NaN) → loses information

3. **Debugging**:
   - `-inf` from `log(0)` is **different** from `NaN` from `sqrt(-1)`
   - Preserving distinction helps diagnose data issues

### Compatibility

- **Rust**: f64 is IEEE-754 by default
- **NumPy**: Uses IEEE-754 (inf preserved)
- **R**: Uses IEEE-754 for numerics
- **SQL**: FLOAT/DOUBLE preserve inf (vendor-specific for NaN)

---

## FAQ

**Q**: Why not coerce inf to NaN for "safety"?
**A**: Because you're hiding information. If you want to treat inf as missing, do it explicitly: `if x.is_infinite() { NA }`.

**Q**: How do I check if a value is "usable" for downstream math?
**A**: `x.is_finite()` returns true only for non-NaN, non-inf values.

**Q**: What if I want "safe" versions that coerce to NaN?
**A**: Implement separate operations: `log_safe`, `div_safe`, `dlog_safe` with explicit coercion. Don't make it the default.

**Q**: Does this affect joins/masks?
**A**: No. Join matching still uses exact equality (NaN ≠ NaN), and masks propagate NaN as "no match" correctly.

---

## Summary

✅ **BLISP uses IEEE-754 standard behavior**
✅ **inf ≠ NaN ≠ NA** (distinct concepts)
✅ **AST and IR must match** (no silent coercion differences)
✅ **Tests use proper comparators** (is_nan, is_infinite, tolerance)

This policy is **non-negotiable** for semantic correctness and cross-system compatibility.
