# NaN Handling Policy v0.2 - KDB-ISH QUANT ENGINE

**Date:** 2026-02-17
**Version:** 0.2 (supersedes v0.1)
**Status:** ✅ FROZEN

This policy defines NaN handling for a **quantitative finance engine** (kdb-ish behavior).

---

## Core Principle

**NaN represents missing/invalid data. Operations handle it pragmatically for quant workflows.**

Key insight: In finance, a few missing ticks shouldn't destroy all statistics.

---

## 1. Arithmetic Operations

**Policy:** **PROPAGATE NaN** (IEEE 754)

```
NaN + 5     => NaN
10 * NaN    => NaN
log(NaN)    => NaN
[10,NaN,30] + 5  => [15,NaN,35]
```

**Rationale:** Standard IEEE behavior, no ambiguity.

**Code:** Rust f64 default ✅

---

## 2. Comparison Operations

**Policy:** **Return FALSE** (IEEE 754 unordered comparisons)

```
NaN > 5     => false
10 < NaN    => false
NaN == NaN  => false  (!)
```

**Column behavior:**
```
[10,NaN,30,40] > 20  => [false, false, true, true]
                         ^^^^^  ^^^^^
                         10≤20  NaN>20 is false
```

**Rationale:** IEEE standard, natural filtering behavior.

**Code:** Not yet implemented, but Rust f64 default matches ✅

---

## 3. Aggregations and Windows **[CHANGED from v0.1]**

### **Policy:** **SKIP NaN by default** (kdb-ish, quant-friendly)

Output NaN only if **insufficient valid observations** for the statistic.

### Simple Aggregations

```
sum([10, NaN, 30, 40])     => 80          (sum of valid values)
mean([10, NaN, 30, 40])    => 26.67       (mean of [10,30,40])
std([10, NaN, 30])         => std(10,30)  if ddof allows
min([10, NaN, 30])         => 10
max([10, NaN, 30])         => 30
```

**NaN output when:**
```
sum([NaN, NaN])            => NaN   (no valid data)
std([10])                  => NaN   (ddof=1 needs ≥2 points)
std([10, NaN])             => NaN   (only 1 valid, ddof=1)
```

### Window Operations

**Rule:** Compute over valid observations in window. Output NaN if insufficient valid data.

```
Input:  [10, 20, NaN, 40, 50, 60]

wstd(col, 3, 0, ddof=1):
  [NaN, NaN, σ(10,20), σ(20,40), σ(40,50), σ(50,60)]
   ^^^^^^^^   ^^^^^^^^  ^^^^^^^^^^^^^^^^^^^^^^^^^
   warm-up    2 valid   all valid windows
              (10,20)

Indices [0,1]: warm-up (window size < 3)
Index 2: window [10,20,NaN] → valid=[10,20] → σ(10,20) ✅
Index 3: window [20,NaN,40] → valid=[20,40] → σ(20,40) ✅
Index 4: window [NaN,40,50] → valid=[40,50] → σ(40,50) ✅
Index 5: window [40,50,60]  → all valid → σ(40,50,60) ✅
```

**Insufficient valid data:**
```
Input:  [10, NaN, NaN, 40, 50]

wstd(col, 3, 0, ddof=1):
  [NaN, NaN, NaN, σ(40,50), σ(40,50)]
   ^^^^^^^^   ^^^  ^^^^^^^^^^^^^^^^^^
   warm-up    insufficient  valid
              (only 1 valid)
```

### Minimum Valid Observations

**Per statistic:**
- `sum`, `mean`, `min`, `max`: need ≥1 valid
- `std`, `var` (ddof=1): need ≥2 valid
- `wzs` (z-score): need ≥2 valid (for non-zero std)
- `ur` (regression): depends on degrees of freedom

**If insufficient:** output is NaN

---

## 4. Boundary NaN (unchanged)

Lag-based operations produce NaN for unavailable indices:

```
shift([10,20,30], 1)  => [NaN, 10, 20]   (boundary)
diff([10,20,30], 1)   => [NaN, 10, 10]   (boundary)
dlog([10,11,12], 1)   => [NaN, 0.095, 0.087]  (boundary)
```

**Both types coexist:**
```
Input:  [10, NaN, 30, 40]
diff(col, 1):  [NaN, NaN, NaN, 10]
                ^^^  ^^^  ^^^  ^^
                |    |    |    40-30 (valid)
                |    |    30-NaN → skipna? still NaN in diff
                |    NaN-10 → skipna? still NaN in diff
                boundary

```

**Important:** Lag operations (shift/diff/dlog) still propagate data NaN element-wise.
Skipna applies to **aggregations within a window**, not element-wise ops.

---

## 5. Special Cases

### Division by Zero
```
5 / 0     => NaN  (not Inf, not error)
0 / 0     => NaN
```

### Invalid Math
```
log(0)    => NaN
log(-5)   => NaN
sqrt(-4)  => NaN  (no complex numbers)
```

---

## 6. Strict Variants (future)

For cases requiring propagate behavior, add `-strict` suffix:

```lisp
; Future:
(sum-strict [10 NaN 30])     => NaN   (propagate)
(wstd-strict col 20 0)       => NaN if any window NaN
```

**Default remains skipna** for all aggregations/windows.

---

## 7. Implementation Checklist

**For all window operations (`wstd`, `wzs`, `wq`, `ur`, etc.):**

1. ✅ Count valid (non-NaN) observations in window
2. ✅ Check if sufficient for statistic (e.g., ≥2 for std with ddof=1)
3. ✅ If insufficient → output NaN
4. ✅ If sufficient → compute over valid subset
5. ✅ Test with: all valid, some NaN, all NaN, boundary cases

**Pseudo-code:**
```rust
fn wstd(col, window, lag, ddof) {
    for i in range {
        let window_data = col[i-window..i];
        let valid: Vec<f64> = window_data.iter()
            .filter(|x| !x.is_nan())
            .copied()
            .collect();

        if valid.len() < 2 {  // ddof=1 needs ≥2
            output[i] = NaN;
        } else {
            output[i] = std(&valid, ddof);
        }
    }
}
```

---

## 8. Rationale: Why Skip NaN by Default?

**✅ Quant-friendly:**
- Real-world data has missing ticks
- One bad tick shouldn't destroy 20-day statistics
- Matches kdb+/q behavior
- Matches pandas default (`skipna=True`)

**✅ Practical:**
- `wstd([10,20,NaN,40,50], 3)` → meaningful statistics
- Alternative (propagate) destroys all downstream calculations

**✅ Industry standard:**
- pandas: `df.rolling(20).std()` skips NaN by default
- kdb+/q: aggregate functions skip nulls
- SQL: `AVG`, `SUM` skip NULL

**❌ v0.1 (propagate) problems:**
- Too conservative for finance
- One missing point kills entire analysis
- Not how quant systems work

---

## 9. Comparison: v0.1 vs v0.2

| Operation | v0.1 | v0.2 (this) |
|-----------|------|-------------|
| `sum([10,NaN,30])` | NaN | 40 |
| `wstd([10,NaN,30],3)` | NaN | σ(10,30) if ddof allows |
| `+ - * /` | Propagate | Propagate (unchanged) |
| `> < =` | False | False (unchanged) |
| Boundary NaN | NaN | NaN (unchanged) |

**Key change:** Aggregations/windows skip NaN by default.

---

## 10. Test Requirements

**Every window operation MUST test:**

1. All valid data (baseline)
2. Some NaN in window (skipna behavior)
3. Insufficient valid data (output NaN)
4. All NaN (output NaN)
5. Boundary warm-up (output NaN)

**Example test pattern:**
```rust
#[test]
fn test_wstd_skipna() {
    let col = make_col(&[10.0, 20.0, f64::NAN, 40.0, 50.0]);
    let result = wstd(&col, 3, 0, ddof=1);

    // Index 2: window [10,20,NaN] → valid=[10,20] → σ(10,20)
    assert!(!result[2].is_nan());

    // Index 3: window [20,NaN,40] → valid=[20,40] → σ(20,40)
    assert!(!result[3].is_nan());
}

#[test]
fn test_wstd_insufficient_valid() {
    let col = make_col(&[10.0, f64::NAN, f64::NAN, 40.0]);
    let result = wstd(&col, 3, 0, ddof=1);

    // Index 2: window [10,NaN,NaN] → only 1 valid → NaN
    assert!(result[2].is_nan());
}
```

---

## 11. Migration from v0.1

**Breaking change:** Window aggregations now skip NaN by default.

**If you need v0.1 behavior (propagate):**
```lisp
; Use strict variants (future):
(wstd-strict col 20 0)   ; propagate NaN
(sum-strict col)         ; propagate NaN
```

**Why this change is correct:**
- v0.1 was overly conservative
- Doesn't match quant finance practice
- Doesn't match kdb+/pandas defaults
- Makes the tool unusable for real data with occasional missing ticks

---

## Summary Table

| Operation | NaN Behavior | Example |
|-----------|--------------|---------|
| Arithmetic | Propagate | `NaN + 5 => NaN` |
| Comparisons | False | `NaN > 5 => false` |
| Aggregations | **Skip NaN** ← NEW | `sum([10,NaN,30]) => 40` |
| Windows | **Skip NaN** ← NEW | `wstd([10,NaN,30],3) => σ(10,30)` |
| Boundary | NaN | `shift(col,1)[0] => NaN` |

**Key principle:** Skip NaN in aggregations (kdb-ish), propagate in arithmetic (IEEE).

---

**Version:** 0.2
**Status:** FROZEN ❄️
**Target:** KDB-ish quantitative finance engine
**Date:** 2026-02-17

---

**All future blisp aggregation/window operations MUST skip NaN by default.**
