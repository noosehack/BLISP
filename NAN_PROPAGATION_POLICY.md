# NaN Propagation Policy - FROZEN CONTRACT

**Date:** 2026-02-17
**Status:** ✅ FROZEN - This is the authoritative NaN handling specification

This document defines how `NaN` (Not-a-Number) is handled across all blisp operations.

---

## Core Principle

**NaN represents missing, undefined, or invalid data.**

When a computation involves `NaN`, the result depends on the operation category.
We follow IEEE 754 standards where applicable, with explicit decisions for ambiguous cases.

---

## 1. Arithmetic Operations

**Policy:** **PROPAGATE NaN** (IEEE 754 standard)

Any arithmetic operation involving `NaN` yields `NaN`.

### Rules:

```
NaN + x   => NaN
x + NaN   => NaN
NaN + NaN => NaN

NaN - x   => NaN
NaN * x   => NaN
NaN / x   => NaN
x / NaN   => NaN
0 / 0     => NaN

log(NaN)  => NaN
exp(NaN)  => NaN
abs(NaN)  => NaN
```

### Scalar Examples:

```lisp
(+ NaN 5)      => NaN
(* 10 NaN)     => NaN
(/ NaN 2)      => NaN
(log NaN)      => NaN
```

### Column Examples:

```lisp
Input:  [10, NaN, 30, 40]

(+ col 5)      => [15, NaN, 35, 45]
(* col 2)      => [20, NaN, 60, 80]
(log col)      => [log(10), NaN, log(30), log(40)]
```

**Rationale:**
- IEEE 754 standard behavior
- Preserves information about missing data
- Prevents silent errors
- Universal expectation in numerical computing

---

## 2. Comparison Operations

**Policy:** **Comparisons with NaN return FALSE** (IEEE 754 standard)

All comparisons involving `NaN` return `false`, including equality checks.

### Rules:

```
NaN == NaN    => false    (!)
NaN != NaN    => true     (!)
NaN < x       => false
NaN > x       => false
NaN <= x      => false
NaN >= x      => false
x < NaN       => false
x > NaN       => false
```

### Scalar Examples:

```lisp
(> NaN 5)      => false
(< 10 NaN)     => false
(= NaN NaN)    => false  ; NaN is not equal to itself!
(/= NaN NaN)   => true   ; NaN is not-equal to itself!
```

### Column Examples:

```lisp
Input:  [10, NaN, 30, 40]

(> col 20)     => [false, false, true, true]
                   ^^^^^  ^^^^^
                   10<20  NaN>20 is false

(< col 25)     => [true, false, false, false]
                   ^^^^  ^^^^^
                   10<25 NaN<25 is false
```

**Result Type:** Boolean column with same length as input

**Rationale:**
- IEEE 754 standard behavior
- Predictable: NaN comparisons always fail
- Filtering with `(> col threshold)` naturally excludes NaN values
- Common in SQL, pandas, numpy

**Special Case: Testing for NaN**

Since `(= x NaN)` is always false, we need a special function:

```lisp
(is-nan x)     => true if x is NaN, false otherwise

; Column version:
(is-nan col)   => [false, true, false, false]
```

**Future addition:** `(is-nan x)` builtin for explicit NaN testing.

---

## 3. Aggregations and Window Operations

**Policy:** **PROPAGATE NaN by default** (conservative, correct)

If ANY value in the aggregation window is `NaN`, the result is `NaN`.

### Rules:

```
sum([10, 20, NaN, 40])     => NaN
mean([10, 20, NaN, 40])    => NaN
std([10, 20, NaN, 40])     => NaN
min([10, 20, NaN, 40])     => NaN
max([10, 20, NaN, 40])     => NaN
```

### Window Operation Examples:

```lisp
Input:  [10, 20, NaN, 40, 50]

wstd(col, 3, 0):
  [NaN, NaN, NaN, NaN, NaN]
   ^^^  ^^^  ^^^  ^^^  ^^^
   |    |    |    |    window includes NaN at position 2
   |    |    window includes NaN at position 2
   |    window includes NaN at position 2
   warm-up period

wzs(col, 3, 1):
  [NaN, NaN, NaN, NaN, NaN]
   Similar propagation
```

**Detailed Window Example:**

```lisp
Input:  [10, 20, 30, 40, 50, 60]

wstd(col, 3, 0):
  [NaN, NaN, σ(10,20,30), σ(20,30,40), σ(30,40,50), σ(40,50,60)]
   ^^^^^^^^   valid rolling window

Input:  [10, 20, NaN, 40, 50, 60]

wstd(col, 3, 0):
  [NaN, NaN, NaN, NaN, NaN, σ(40,50,60)]
   ^^^^^^^^   ^^^^^^^^^^   ^^^^^^^^^^^
   warm-up    windows containing NaN   first clean window
```

**Rationale:**
- **Conservative:** Missing data means uncertain statistics
- **Correct:** Can't compute true std/mean with missing values
- **Transparent:** NaN output clearly indicates data quality issues
- **Safe:** Prevents misleading results from partial data

**Alternative (Future):** Could add `skipna` variants later:

```lisp
; Future:
(wstd-skipna col 3 0)  ; Ignore NaN values in window
(sum-skipna col)       ; Sum of non-NaN values
```

But default behavior is propagate.

---

## 4. Lag-Based Operations

**Policy:** **PROPAGATE NaN through lag references**

Already covered in BOUNDARY_SEMANTICS.md, but restated here for completeness.

### Rules:

```lisp
Input:  [10, NaN, 30, 40]

shift(col, 1):
  [NaN, 10, NaN, 30]
   ^^^  ^^  ^^^  ^^
   |    |   |     shift of 30 (valid)
   |    |   shift of NaN (propagates)
   |    shift of 10 (valid)
   boundary NaN

diff(col, 1):
  [NaN, NaN, NaN, 10]
   ^^^  ^^^  ^^^  ^^
   |    |    |    40 - 30 (valid)
   |    |    30 - NaN (propagates)
   |    NaN - 10 (propagates)
   boundary NaN

dlog(col, 1):
  [NaN, NaN, NaN, log(40/30)]
   Similar propagation pattern
```

**Key Point:** Both boundary NaN and data NaN propagate.

---

## 5. Special Functions

### Division by Zero

**Policy:** **Return NaN** (IEEE 754 standard)

```lisp
(/ 5 0)      => NaN  ; Not infinity, not error
(/ 0 0)      => NaN
(/ NaN 0)    => NaN
```

**Rationale:** Consistent with propagation policy, doesn't crash.

### Logarithm of Non-Positive

**Policy:** **Return NaN**

```lisp
(log 0)      => NaN
(log -5)     => NaN  ; No complex numbers
(dlog col lag) where col[i] <= 0 => NaN at position i
```

### Square Root of Negative

**Policy:** **Return NaN** (when implemented)

```lisp
(sqrt -4)    => NaN  ; No complex numbers
```

---

## 6. Creation and Input

### How NaN is Created

**From CSV:**
```csv
px;vol
100;1000
NA;1200      # Parsed as NaN
NaN;800      # Parsed as NaN
101;NA       # Parsed as NaN
```

**Programmatically:**
```lisp
(/ 0 0)                    ; Arithmetic that produces NaN
(log 0)                    ; Math function on invalid input
(make-col 10 NaN 30)       ; Explicit NaN in constructor (future)
```

**From Operations:**
```lisp
(shift col 1)              ; Boundary NaN
(dlog col 1)               ; Boundary NaN or invalid computation
```

---

## 7. Output and Display

### Printing NaN

**Text representation:** `NaN` (uppercase)

```lisp
(print NaN)                ; Output: NaN
(print (make-col 10 NaN 30))   ; Output: Col[10, NaN, 30]
```

### CSV Output

**Policy:** Write `NaN` as string literal

```csv
TIMESTAMP;px;returns
2020-01-01;100.0;NaN
2020-01-02;102.0;0.0198
2020-01-03;NaN;NaN
```

**Alternative formats (configurable future):**
- Empty string: `100.0;;0.0198`
- NULL: `100.0;NULL;0.0198`
- Custom sentinel: `100.0;-99999;0.0198`

Default is `NaN` string.

---

## 8. Implementation Requirements

### All operations MUST:

1. ✅ Follow the propagation rules for their category
2. ✅ Document any deviation from this policy
3. ✅ Use `f64::NAN` as the canonical NaN representation
4. ✅ Handle NaN in tests

### Testing Requirements

Every operation MUST have tests for:

- NaN as input
- NaN from computation
- NaN propagation through chains
- Mixed NaN and valid data

**Example test patterns:**

```rust
#[test]
fn test_add_nan_propagation() {
    // NaN + number => NaN
    assert!(builtin_add(&[NaN, 5]).unwrap().is_nan());

    // number + NaN => NaN
    assert!(builtin_add(&[10, NaN]).unwrap().is_nan());

    // NaN + NaN => NaN
    assert!(builtin_add(&[NaN, NaN]).unwrap().is_nan());
}

#[test]
fn test_comparison_nan_false() {
    // NaN > x => false
    assert_eq!(builtin_gt(&[NaN, 5]).unwrap(), false);

    // x > NaN => false
    assert_eq!(builtin_gt(&[10, NaN]).unwrap(), false);
}

#[test]
fn test_window_nan_propagation() {
    let col = make_col(&[10.0, NaN, 30.0, 40.0]);
    let result = wstd(&col, 3, 0);

    // Windows containing NaN should yield NaN
    assert!(result[2].is_nan());  // window [10, NaN, 30]
    assert!(result[3].is_nan());  // window [NaN, 30, 40]
}
```

---

## 9. Summary Table

| Category | Operation | NaN Behavior | Example |
|----------|-----------|--------------|---------|
| **Arithmetic** | `+` `-` `*` `/` | Propagate | `NaN + 5 => NaN` |
| **Math** | `log` `exp` `abs` | Propagate | `log(NaN) => NaN` |
| **Comparison** | `>` `<` `>=` `<=` `=` | Return false | `NaN > 5 => false` |
| **Lag** | `shift` `diff` `dlog` | Propagate | `diff([10,NaN,30],1) => [NaN,NaN,NaN]` |
| **Window** | `wstd` `wzs` `ur` | Propagate | `wstd([10,NaN,30],3) => NaN` |
| **Aggregation** | `sum` `mean` `std` | Propagate | `sum([10,NaN,30]) => NaN` |
| **Special** | `/0` `log(0)` | Return NaN | `5/0 => NaN` |

---

## 10. Rationale Summary

### Why Propagate for Arithmetic/Windows?

**✅ Correct:** Missing data = uncertain result
**✅ Safe:** No silent errors from partial calculations
**✅ Transparent:** NaN output shows data quality issues
**✅ Standard:** Matches IEEE 754, numpy default, conservative approach

### Why False for Comparisons?

**✅ Standard:** IEEE 754 behavior
**✅ Practical:** Filters naturally exclude NaN
**✅ Predictable:** Consistent with all numerical systems
**✅ SQL-compatible:** Matches SQL NULL comparison semantics

### Why Not Skip NaN by Default?

**❌ Wrong:** `std([10, NaN, 30])` is mathematically undefined
**❌ Misleading:** Gives false confidence in uncertain data
**❌ Dangerous:** Hidden data quality problems
**❌ Inconsistent:** Different window sizes give incomparable results

Can add `skipna` variants later for specific use cases, but default is propagate.

---

## 11. Future Extensions

### Possible Future Additions (NOT in frozen contract):

**Explicit NaN handling:**
```lisp
(is-nan x)           ; Test for NaN
(drop-nan col)       ; Remove NaN values
(fill-nan col value) ; Replace NaN with value
```

**Skip-NaN variants:**
```lisp
(sum-skipna col)     ; Sum ignoring NaN
(mean-skipna col)    ; Mean ignoring NaN
(wstd-skipna col w l); Window std ignoring NaN
```

**Interpolation:**
```lisp
(locf col)           ; Last observation carried forward
(interp-linear col)  ; Linear interpolation
```

These are NOT part of the frozen contract. Default behavior is PROPAGATE.

---

## 12. Exceptions and Edge Cases

### Empty Windows After NaN Removal

**If implementing skipna variants:**

```lisp
(mean-skipna [NaN, NaN, NaN])  => NaN
; Can't compute mean of zero values
```

### All-NaN Columns

```lisp
(wstd [NaN, NaN, NaN] 3 0)  => [NaN, NaN, NaN]
; Consistent with propagation
```

### Infinity

**Not currently supported.** If added later:

```lisp
Inf + Inf   => Inf
Inf - Inf   => NaN
Inf * 0     => NaN
Inf / Inf   => NaN
```

But for now, only NaN is supported (not Inf).

---

## 13. Contract Enforcement

**This document is the authoritative specification for NaN handling.**

Any deviation from these rules is a BUG, unless:
1. Explicitly documented as an extension
2. Marked as a `skipna` or alternative variant
3. Part of a new major version

**Version:** 1.0
**Status:** FROZEN ❄️
**Last Updated:** 2026-02-17

---

## Quick Reference Card

```
ARITHMETIC:   NaN OP x     => NaN     (propagate)
COMPARISONS:  NaN > x      => false   (IEEE 754)
WINDOWS:      window[NaN]  => NaN     (propagate)
LAG:          lag(NaN)     => NaN     (propagate)
SPECIAL:      log(0)       => NaN     (undefined)
              x / 0        => NaN     (undefined)
TEST:         (is-nan x)   => bool    (future)
```

**Default philosophy: Conservative propagation preserves data quality information.**

---

**All blisp operations MUST comply with this NaN propagation policy.**
