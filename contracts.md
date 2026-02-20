# BLADE IR Semantic Contracts

**Purpose**: Single source of truth for operation semantics. No ambiguity, no implementation discretion.

**Status**: Frozen contracts are non-negotiable. Tests must enforce them. Code must implement them exactly.

---

## 1. Core Invariants (I1-I5) [FROZEN ✅]

### I1: Index Preservation (Unary Ops)
**Contract**: Unary numeric operations preserve row count and index identity.
```rust
Arc::ptr_eq(&output.tags.index, &input.tags.index)
output.nrows == input.nrows
```

### I2: Column Name Preservation (Unary Ops)
**Contract**: Unary numeric operations preserve column names.
```rust
Arc::ptr_eq(&output.tags.colnames, &input.tags.colnames)
```

### I3: Shape Preservation (Unary Ops)
**Contract**: Unary numeric operations preserve shape.
```rust
output.nrows == input.nrows
output.ncols == input.ncols
```

### I4: Join Semantics (mapr)
**Contract**: `mapr(x, y)` aligns x onto y's index.
```
output.index == y.index  (Arc ptr_eq)
output.colnames == x.colnames
output[i,j] = x[lookup(y.index[i]), j] if found, else NA
```

### I5: No Implicit Schema Rebuild
**Contract**: Tags carried by Arc reference through pipelines. Materialization only at boundaries.

---

## 2. Shift Operation [FROZEN ✅]

### Definition
**Form**: `(shift k x)` where `k >= 0` (integer literal only)

**Semantics**: Lag operation (move values down by k rows)
```
output[i] = input[i-k]  for i >= k
output[i] = NA          for i < k
```

### Contracts
- **Lag-only**: `k >= 0` (no forward-looking by construction)
- **Shape preserved**: `output.nrows == input.nrows`
- **Arc preservation**: I1-I3 enforced
- **Mask monotone**: NA positions only grow (conservative)

### Laws (Tested)
- **Identity**: `shift(0, x) == x` (exact, Arc ptr_eq)
- **Composition**: `shift(a, shift(b, x)) == shift(a+b, x)`
- **Mask monotonicity**: `mask(shift(k,x)) ⊇ mask(x)`

---

## 3. Binary Operations [FROZEN ✅]

### Strict Compatibility
**Contract**: Binary ops require exact index and shape compatibility.
```
lhs.index_type == rhs.index_type  (no coercion)
lhs.nrows == rhs.nrows
lhs.ncols == rhs.ncols
```
**Error message**: "Use mapr/asofr for alignment"

### Scalar Broadcast
**Contract**: Scalars broadcast to all cells.
```
(+ x 5.0)  // valid: scalar broadcasts
```

### NA Propagation
**Contract**: NA in either operand → NA in result.
```
mask(x op y) == mask(x) ∧ mask(y)
```

### Arc Preservation
**Contract**: LHS tags preserved (I1-I3).
```rust
Arc::ptr_eq(&output.tags.index, &lhs.tags.index)
Arc::ptr_eq(&output.tags.colnames, &lhs.tags.colnames)
```

### Division by Zero
**Contract**: Division by zero → NA (no error, no inf).

### Laws (Tested)
- **Additive identity**: `x + 0 == x` (exact, Arc ptr_eq)
- **Multiplicative identity**: `x * 1 == x` (exact, Arc ptr_eq)
- **Absorption**: `x * 0 == 0` (where x valid), `NA` (where x NA)

---

## 4. dlog Identity (Temporal Correctness Tripwire) [FROZEN ✅]

### Contract
```
dlog(x) == log(x / shift(1, x))
```

**Validates simultaneously**:
1. Shift sign convention (lag vs lead)
2. Division-by-zero → NA
3. NA propagation correctness
4. Log domain handling
5. No off-by-one errors

**Test strategy**: Hand-crafted smoke test + property test (positive domain, 20 rows)

---

## 5. Rolling Window Operations [FROZEN ✅]

### A) Window Definition

**Contract**: Trailing window, inclusive of current row (kdb/pandas-compatible).

```
rolling_op(w, x)[i] = f( x[max(0, i-w+1) .. i] )
```

**Window boundaries**:
- For row `i`, window spans indices `[i-w+1, i]` (inclusive both ends)
- Window size `w >= 1` (validated at plan time)

**Examples**:
```
w=3, i=5: uses rows [3,4,5]
w=3, i=1: uses rows [0,1] (partial window, only 2 values)
w=1, i=any: uses row [i] only (identity-like)
```

---

### B) min_periods Policy

**Contract**: **Strict** (conservative NA philosophy).

```
if count(valid values in window) < w:
    result[i] = NA
else:
    result[i] = f(valid values in window)
```

**Rationale**:
- Simplest reasoning, strongest tripwires, easiest fusion legality
- Conservative NA policy consistent with shift/dlog/binary ops
- No API surface for `min_periods` parameter (can add later if needed)

**Implications**:
- Prefix `i < w-1` always NA (not enough observations)
- Partial windows at boundaries → NA
- Rolling ops only produce valid results for `i >= w-1`

---

### C) NA Handling Inside Window

**Contract**: **Skip NA** (compute statistic over valid values only).

```
For window [i-w+1 .. i]:
  1. Extract non-NA values
  2. If count(non-NA) < w: result = NA (per min_periods strict)
  3. Else: compute f(non-NA values)
```

**Rationale**:
- Makes rolling ops usable on real data (avoids poison-window explosion)
- Still conservative: requires full window of valid values
- Mask monotone: `mask(rolling_op(w,x)) ⊇ mask(x)` (can only add NAs, never remove)

**Example**:
```
w=3, x = [1.0, NA, 3.0, 4.0, 5.0]
rolling_mean(3, x):
  [0]: window [1.0] → count=1 < 3 → NA
  [1]: window [1.0, NA] → count=1 < 3 → NA
  [2]: window [1.0, NA, 3.0] → count=2 < 3 → NA
  [3]: window [NA, 3.0, 4.0] → count=2 < 3 → NA
  [4]: window [3.0, 4.0, 5.0] → count=3 == 3 → mean(3,4,5) = 4.0
```

---

### D) Prefix Behavior

**Contract**: Under strict `min_periods = w`, prefix `i < w-1` is **always NA**.

```
rolling_op(w, x)[i] = NA  for all i < w-1
```

**Rationale**:
- Not enough observations to form full window
- Conservative, predictable, testable
- Matches shift prefix behavior (lag k introduces k NAs at start)

---

### E) Standard Deviation Definition

**Contract**: **Population standard deviation** (ddof=0).

```
std(values) = sqrt( sum((x - mean)^2) / n )
```
where `n = count(values)`.

**Rationale**:
- Simpler than sample std (ddof=1)
- More stable (no n-1 in denominator)
- Sufficient for finance use cases (zscore normalization)
- Can add sample std later if needed

**Zero variance contract**:
```
If all values in window are identical (or window has 1 value):
    std = 0.0  (not NA)
```

---

### F) zscore Definition

**Contract**: Standardization using rolling mean and std.

```
zscore(w, x)[i] = (x[i] - rolling_mean(w,x)[i]) / rolling_std(w,x)[i]
```

**Division-by-zero handling**:
```
If rolling_std(w,x)[i] == 0.0:
    zscore(w,x)[i] = NA
```

**Rationale**:
- Conservative (avoids fake signal from flat series)
- Consistent with binary division-by-zero → NA
- For finance: NA is safer than 0 or inf

**NA propagation**:
```
If rolling_mean[i] is NA OR rolling_std[i] is NA OR x[i] is NA:
    zscore[i] = NA
```

---

### G) Arc Preservation (Rolling Ops are Unary)

**Contract**: Rolling operations preserve I1-I3 invariants.

```rust
Arc::ptr_eq(&output.tags.index, &input.tags.index)
Arc::ptr_eq(&output.tags.colnames, &input.tags.colnames)
output.nrows == input.nrows
```

---

### H) Feature Engineering Family (ft_*) [FROZEN ✅]

**Contract**: All `ft_*` operations use "yesterday's distribution" semantics (no self-reference).

**Definition**: For any rolling statistic R, the feature variant is defined as:
```
ft_R(w, x) := shift(1, R(w, x))
```

**Rationale**:
- **No information leakage**: x[i] is compared to pure historical window (x[i-w..i-1])
- **Explicit**: No ambiguity about window boundaries
- **Compositional**: Reuses proven primitives (shift + rolling)
- **Fusion-ready**: Can optimize later without changing semantics
- **Inherits all invariants**: Arc preservation, NA policy, mask monotonicity

**Concrete semantics**:
```
ft_mean(w,x)[i]   = rolling_mean(w,x)[i-1] = mean(x[i-w .. i-1])
ft_std(w,x)[i]    = rolling_std(w,x)[i-1]  = std(x[i-w .. i-1])
ft_zscore(w,x)[i] = (x[i] - ft_mean(w,x)[i]) / ft_std(w,x)[i]
```

**Implementation**: Planner rewrite (not new IR primitives).
```
(ft-mean w x)   → (shift 1 (rolling-mean w x))
(ft-std w x)    → (shift 1 (rolling-std w x))
(ft-zscore w x) → (/ (- x (ft-mean w x)) (ft-std w x))
```

**Use cases**:
- Backtesting (no forward-looking by construction)
- ML feature engineering (out-of-sample comparison)
- Signal generation (today's value vs historical distribution)

**Standard vs Feature**:
- **Standard rolling**: Descriptive statistics (x[i] in its own window)
- **Feature ft_**: Predictive features (x[i] vs yesterday's distribution)

**Test requirement**: Metamorphic identity must hold:
```
ft_R(w,x) == shift(1, R(w,x))
```

---

## 6. Rolling Operations: Metamorphic Laws (Test Requirements)

### rolling_mean Laws

**L1: Window=1 is identity**:
```
rolling_mean(1, x)[i] == x[i]  for all i
```
(Except NA stays NA; Arc ptr_eq holds)

**L2: Constant series invariant**:
```
If x[i] = c (constant, no NA) for all i in window:
    rolling_mean(w,x)[i] = c  for all i >= w-1
```

**L3: Shift commutation** (strong tripwire):
```
shift(k, rolling_mean(w,x)) == rolling_mean(w, shift(k,x))  for k >= 0
```
Holds under trailing windows with strict min_periods.

**L4: Mask monotonicity**:
```
mask(rolling_mean(w,x)) ⊇ mask(x)
```
(Rolling can only add NAs at prefix, never remove them)

---

### rolling_std Laws

**L1: Constant series**:
```
If x[i] = c (constant, no NA) for all i in window:
    rolling_std(w,x)[i] = 0.0  for all i >= w-1
```

**L2: Non-negativity**:
```
rolling_std(w,x)[i] >= 0.0  for all valid (non-NA) results
```

**L3: Scale equivariance**:
```
rolling_std(w, x*c) == rolling_std(w,x) * |c|  for scalar c != 0
```
(Where both sides are valid; NA propagates correctly)

**L4: Window=1**:
```
rolling_std(1, x)[i] = 0.0  for all i
```
(Single observation → zero variance)

---

### rolling_zscore Laws

**L1: Constant series**:
```
If x[i] = c (constant, no NA):
    rolling_zscore(w,x)[i] = NA  for all i
```
(Because std=0 → division by zero → NA)

**L2: Window=1**:
```
rolling_zscore(1, x)[i] = NA  for all i
```
(Because mean=x, std=0 → division by zero → NA)

**L3: Derived form identity** (if zscore implemented as primitive):
```
rolling_zscore(w,x) == (x - rolling_mean(w,x)) / rolling_std(w,x)
```
(Validates that primitive matches derived semantics)

**L4: Standardization properties** (when valid):
```
mean(rolling_zscore(w,x)[w-1:]) ≈ 0.0  (within floating-point error)
std(rolling_zscore(w,x)[w-1:]) ≈ 1.0   (within floating-point error)
```
(Skipping NA values in the zscore output)

---

## Appendix: Contract Philosophy

**Conservative NA Policy**: When in doubt, return NA. No invented values. Mask monotone.

**Explicit Alignment**: No implicit joins. Use `mapr`/`asofr` for index alignment.

**Arc Preservation**: Zero-copy tags. Unary ops preserve I1-I3 by construction.

**Temporal Causality**: Lag-only primitives. "Future" must be explicit (no accidental lookahead).

**Metamorphic Testing**: Laws prove correctness. dlog identity, shift commutation, constant series invariants.

**Single Source of Truth**: This document. No implementation discretion. Tests enforce contracts exactly.

---

*Contract frozen: 2026-02-20*
*Next action: Implement rolling_mean (Step 3C.4.1)*
