# Boundary Semantics - FROZEN CONTRACT

**Date:** 2026-02-17
**Status:** ✅ FROZEN - Do not change without major version bump

This document defines the boundary behavior for all lag-based operations in blisp.
All future operations (wstd, wzs, ur, etc.) MUST follow these rules.

---

## Core Principle

**Unavailable elements are represented as `NaN` (not-a-number).**

When an operation requires data that doesn't exist (before start or after end of series),
the output at that position is `NaN`.

---

## General Lag Semantics

### For Positive Lag (lag > 0) - Looking Backward

```
Input:   [x₀, x₁, x₂, x₃, x₄]
lag = 1

Output:  [NaN, x₀, x₁, x₂, x₃]
         ^^^  └─────────────┘
         |    shifted right by 1
         unavailable
```

**Rule:**
- Indices `[0 .. lag-1]` are `NaN`
- For `i >= lag`: `out[i] = f(x[i], x[i-lag])`

**Examples:**

```
shift(x, 1):  [NaN, x₀, x₁, x₂, x₃]
shift(x, 2):  [NaN, NaN, x₀, x₁, x₂]
diff(x, 1):   [NaN, x₁-x₀, x₂-x₁, x₃-x₂, x₄-x₃]
dlog(x, 1):   [NaN, log(x₁/x₀), log(x₂/x₁), ...]
```

### For Negative Lag (lag < 0) - Looking Forward (LEAD)

```
Input:   [x₀, x₁, x₂, x₃, x₄]
lag = -1

Output:  [x₁, x₂, x₃, x₄, NaN]
         └─────────────┘  ^^^
         shifted left      unavailable
```

**Rule:**
- Indices `[n+lag .. n-1]` are `NaN`
  (where `n` = length, and `lag` is negative, so `n+lag < n`)
- For `i < n+lag`: `out[i] = f(x[i], x[i-lag])`

**Examples:**

```
shift(x, -1):  [x₁, x₂, x₃, x₄, NaN]
shift(x, -2):  [x₂, x₃, x₄, NaN, NaN]
diff(x, -1):   [x₁-x₀, x₂-x₁, x₃-x₂, x₄-x₃, NaN]
```

**Note:** Negative lag not yet implemented, but semantics are frozen.

### For |lag| >= n (lag exceeds series length)

**Rule:** Output is all `NaN`

```
Input:   [x₀, x₁, x₂]  (length = 3)
lag = 5

Output:  [NaN, NaN, NaN]
```

**Reasoning:** No valid reference point exists for any element.

### For lag = 0

**Rule:** Output equals input (identity operation)

```
Input:   [x₀, x₁, x₂, x₃, x₄]
lag = 0

shift(x, 0):  [x₀, x₁, x₂, x₃, x₄]
diff(x, 0):   [0, 0, 0, 0, 0]
dlog(x, 0):   [0, 0, 0, 0, 0]
```

---

## Windowed Operations Contract

For operations with a window size `w` and lag `l`:

**General form:** `op(x, window, lag)`

### Window Boundary Semantics

**Rule:** Indices `[0 .. window-1]` are `NaN` (insufficient history)

```
Input:   [x₀, x₁, x₂, x₃, x₄, x₅, x₆]
wstd(x, 3, 0):  [NaN, NaN, std(x₀,x₁,x₂), std(x₁,x₂,x₃), ...]
                 ^^^^^^^^   └─────────────────────────────┘
                 warm-up    valid rolling window
```

**Reasoning:** Need at least `window` elements to compute statistics.

### Combined Window + Lag

When both window and lag are present:

```
wstd(x, window=3, lag=1):
  - First (window-1) elements: NaN (insufficient history)
  - Next (lag) elements: May be NaN depending on lag offset
  - Remaining: valid rolling statistic with lag offset
```

**Example:**

```
Input:  [x₀, x₁, x₂, x₃, x₄, x₅]
wzs(x, 3, 1):  [NaN, NaN, NaN, z₃, z₄, z₅]
                ^^^^^^^^^^^ ^^^  └────┘
                warm-up     lag   valid
```

Where `zᵢ = (xᵢ₋₁ - μ(xᵢ₋₃:xᵢ₋₁)) / σ(xᵢ₋₃:xᵢ₋₁)`

---

## Specific Operations

### shift(col, lag)

**Definition:** Shift values by `lag` positions.

**Positive lag (backward shift):**
```
shift([10, 20, 30, 40], 1)  => [NaN, 10, 20, 30]
shift([10, 20, 30, 40], 2)  => [NaN, NaN, 10, 20]
```

**Negative lag (forward shift - FUTURE):**
```
shift([10, 20, 30, 40], -1) => [20, 30, 40, NaN]
shift([10, 20, 30, 40], -2) => [30, 40, NaN, NaN]
```

---

### diff(col, lag)

**Definition:** `out[i] = x[i] - x[i-lag]`

**Positive lag:**
```
diff([10, 12, 15, 13], 1)  => [NaN, 2, 3, -2]
                                ^^^  └─────┘
                                |    x[i]-x[i-1]
                                unavailable

diff([10, 12, 15, 13], 2)  => [NaN, NaN, 5, 1]
                                ^^^^^^^^  └──┘
                                |         x[i]-x[i-2]
                                unavailable
```

**Negative lag (FUTURE):**
```
diff([10, 12, 15, 13], -1) => [2, 3, -2, NaN]
                                └─────┘  ^^^
                                x[i+1]-x[i] unavailable
```

---

### dlog(col, lag)

**Definition:** `out[i] = log(x[i]) - log(x[i-lag])` = `log(x[i] / x[i-lag])`

**Positive lag:**
```
dlog([100, 101, 102], 1)  => [NaN, log(101/100), log(102/101)]
                              ^^^  └──────────────────────────┘
                              |    log returns
                              unavailable
```

**Special cases:**
- If `x[i] <= 0` or `x[i-lag] <= 0`: output is `NaN` (log undefined)
- If `x[i-lag]` is `NaN`: output is `NaN` (propagates)

---

### wstd(col, window, lag) - FUTURE

**Definition:** Rolling standard deviation with window size and lag.

```
Input:  [x₀, x₁, x₂, x₃, x₄, x₅, x₆, x₇]
wstd(x, 3, 1):  [NaN, NaN, NaN, σ₃, σ₄, σ₅, σ₆, σ₇]

Where:
  σ₃ = std(x₀, x₁, x₂)  (lagged by 1, so computes over [i-3..i-1])
  σ₄ = std(x₁, x₂, x₃)
  ...
```

**Boundary:**
- First `(window + lag - 1)` elements are `NaN`
- Remainder: valid rolling standard deviation

---

### wzs(col, window, lag) - FUTURE

**Definition:** Rolling z-score normalization.

```
out[i] = (x[i-lag] - mean(x[i-window..i])) / std(x[i-window..i])
```

**Boundary:**
- First `(window - 1)` elements: `NaN` (insufficient window)
- If `std = 0`: output is `NaN` (avoid division by zero)

---

### ur(col, window, lag) - FUTURE

**Definition:** Univariate regression (rolling beta).

Computes rolling regression: `y[i] ~ x[i-lag]` over `[i-window..i]`

**Boundary:**
- First `(window + lag - 1)` elements: `NaN`
- If insufficient valid data in window: `NaN`

---

## NaN Propagation Rules

**All operations propagate NaN:**

1. If input contains `NaN`, output at that position is `NaN`
2. If computation requires a `NaN` value, result is `NaN`
3. Operations never "skip" or "fill" NaN values automatically

**Example:**
```
Input:  [10, NaN, 30, 40]
diff(x, 1):  [NaN, NaN, NaN, 10]
              ^^^  ^^^  ^^^  └─┘
              |    |    |    40-30 (valid)
              |    |    30-NaN (propagates)
              |    NaN-10 (propagates)
              unavailable (boundary)
```

---

## Empty or Single-Element Series

**Rule:** Follow general semantics naturally.

```
shift([], lag)     => []
shift([x], 1)      => [NaN]
diff([x], 1)       => [NaN]
dlog([x], 1)       => [NaN]
wstd([x], 3, 1)    => [NaN]
```

---

## Implementation Requirements

### All lag-based operations MUST:

1. ✅ Return `NaN` for indices where reference data is unavailable
2. ✅ Use `f64::NAN` for NaN representation
3. ✅ Propagate `NaN` through computations
4. ✅ Handle edge cases (empty, single element, |lag| >= n)
5. ✅ Document deviation if different from this spec

### Testing Requirements

All operations MUST have tests covering:

- `lag = 1` (most common case)
- `lag > 1` (multi-period)
- `lag = 0` (identity)
- `lag = -1` (lead, when supported)
- `|lag| >= n` (exceeds series)
- Input with `NaN` values (propagation)
- Empty series
- Single-element series

---

## Current Implementation Status

### ✅ Implemented and Frozen:

- `shift(col, lag)` - lag > 0 only
  - Boundary: `[0..lag-1]` = NaN ✅
  - Test: ✅

- `diff(col, lag)` - lag > 0 only
  - Boundary: `[0..lag-1]` = NaN ✅
  - Test: ✅

- `dlog(col, lag)` - lag > 0 only (blawktrust kernel)
  - Boundary: `[0..lag-1]` = NaN ✅
  - Test: ✅ (in blawktrust)

### 🔲 Not Yet Implemented:

- Negative lag (lead operations)
- `wstd`, `wzs`, `wq`, `ur` (windowed operations)
- `locf` (last observation carried forward)

When implementing these, they MUST follow the boundary contract above.

---

## Verification Test Suite

```lisp
; Test lag=1
(assert-nan-at 0 (shift (make-col 10 20 30) 1))
(assert-nan-at 0 (diff (make-col 10 20 30) 1))
(assert-nan-at 0 (dlog (make-col 10 20 30) 1))

; Test lag=2
(assert-nan-at 0 (shift (make-col 10 20 30 40) 2))
(assert-nan-at 1 (shift (make-col 10 20 30 40) 2))

; Test lag >= n
(assert-all-nan (shift (make-col 10 20 30) 5))

; Test NaN propagation
(assert-nan-at 1 (diff (make-col 10 NaN 30 40) 1))
(assert-nan-at 2 (diff (make-col 10 NaN 30 40) 1))
```

---

## Rationale

### Why NaN instead of NULL/nil?

1. **Standard:** IEEE 754 NaN is the standard for missing numeric data
2. **Performance:** No need for separate validity bitmaps in simple cases
3. **Compatibility:** Works with all numeric operations
4. **Propagation:** Automatic NaN propagation prevents silent errors

### Why freeze this now?

1. **Consistency:** All future operations (wzs, ur, etc.) will follow same rules
2. **Testing:** Clear contract enables comprehensive test suites
3. **Documentation:** Users know exactly what to expect
4. **API Stability:** No breaking changes to boundary behavior

---

## Contract Enforcement

**This document is the authoritative specification.**

Any deviation from these rules is a BUG and must be fixed, unless:
1. A new major version is released, OR
2. The deviation is explicitly documented as an extension

---

**Version:** 1.0
**Status:** FROZEN ❄️
**Last Updated:** 2026-02-17

---

**All future blisp operations MUST comply with this boundary contract.**
