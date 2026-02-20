# BLADE IR Implementation Status

**Last Updated**: 2026-02-20
**Branch**: `reconstruct/tableview-only`
**Current Phase**: Step 3C.3.1 Complete → Ready for Step 3C.4 (Rolling Ops)

---

## Executive Summary

**IR v1 is feature-complete and battle-tested** with 88 passing tests (800+ property cases).

The IR layer now supports:
- ✅ Unary operations (dlog, ret, log, exp, sqrt, abs, inv, **shift**)
- ✅ Binary operations (+, -, *, /, **strict semantics**)
- ✅ Join operations (mapr, asofr, **no implicit alignment**)
- ✅ Let-bindings (sequential scoping, **let\* semantics**)
- ✅ **dlog identity proven** (highest-leverage semantic tripwire)

**Contracts enforced**:
- No index coercion (explicit mapr/asofr required for alignment)
- Arc preservation (I1-I3 invariants, zero-copy)
- Conservative NA policy (no invented values)
- Lag-only shift (no forward-looking by construction)

**Next**: Rolling window operations (mean, std, zscore)

---

## Test Coverage (88 tests, 800+ property cases)

### Test Suites

| Suite | Tests | Purpose | Status |
|-------|-------|---------|--------|
| `ir_equivalence` | 9 | Property tests (600 cases: 300 Date + 300 Timestamp) | ✅ ALL PASS |
| `ir_equivalence_smoke` | 35 | Deterministic smoke tests | ✅ ALL PASS |
| `metamorphic` | 29 | Semantic tripwires (contracts.md) | ✅ ALL PASS |
| `differential_exec` | 15 | AST vs IR oracle comparison | ✅ ALL PASS |

### Metamorphic Properties Proven

**Let\* Scoping Laws**:
- Shadowing: `(let ((x e1) (x e2)) body) == (let ((x e2)) body)`
- Sequential dependency: bindings see prior values

**Join Semantics**:
- Shape invariants: `rows(mapr(x,y)) == rows(y)` (RIGHT OUTER JOIN)
- Column projection: `cols(mapr(x,y)) == cols(x)`
- Arc identity: `Arc::ptr_eq(&result.index, &y.index)` (zero-copy)

**Binary Operation Laws**:
- Additive identity: `x + 0 == x` (exact, including Arc ptr_eq)
- Multiplicative identity: `x * 1 == x`
- Absorption: `x * 0 = 0` (where x valid), `NA` (where x NA)
- LHS tags preservation: Arc pointer equality (I1-I3)
- NA propagation: `mask(x op y) == mask(x) ∧ mask(y)`

**Shift Operation Laws**:
- Identity: `shift(0, x) == x` (exact, Arc ptr_eq)
- Composition: `shift(a, shift(b, x)) == shift(a+b, x)`
- Mask monotonicity: NA positions only grow (conservative)
- Arc preservation: I1-I3 verified

**Time-Series Identity** (🔥 **CRITICAL TRIPWIRE**):
- **dlog identity**: `dlog(x) == log(x / shift(1, x))`
  - Validates: shift sign, div-by-zero, NA propagation, log domain, no off-by-one
  - Hand-crafted smoke test (5 rows, known values)
  - Property test (20 rows, positive domain, 3-layer assertion)

---

## Implementation Architecture

### IR Layer (`src/ir.rs`)

```rust
pub enum Operation {
    Source(Source),           // File, Variable
    Unary(UnaryOp),          // MapNumeric
    Binary(BinaryOp),        // MapNumeric2 (scalar or frame RHS)
    Join(JoinOp),            // MapR, AsofR
}

pub enum UnaryFunc {
    Dlog, Ret, Log, Exp, Sqrt, Abs, Inv,
    Shift { k: usize },      // Lag-only (k ≥ 0)
}

pub enum BinaryFunc {
    Add, Sub, Mul, Div,
}

pub enum ValueRef {
    Scalar(f64),             // Broadcast to all cells
    Frame(NodeId),           // Element-wise (strict compatibility)
}
```

**Validation** (plan-time):
- Index type compatibility (no coercion)
- Shape compatibility for binary ops (same nrows, same ncols)
- Arc preservation contracts (I1-I3)

### Planner (`src/planner.rs`)

**Lowering**:
- `(dlog x)` → `Unary(MapNumeric { input, func: Dlog })`
- `(+ x 5.0)` → `Binary(MapNumeric2 { lhs, rhs: Scalar(5.0), func: Add })`
- `(+ x y)` → `Binary(MapNumeric2 { lhs, rhs: Frame(y_id), func: Add })`
  - Validates: same index type, same nrows, same ncols
  - **Error if incompatible**: "Use mapr/asofr for alignment"
- `(shift k x)` → `Unary(MapNumeric { input, func: Shift { k } })`
  - Validates: `k ≥ 0` (integer literal only)
  - **Rejects**: negative k, float k, expression k
- `(mapr x y)` → `Join(MapR { x, y })`
- `(let ((bindings...)) body)` → sequential evaluation (let\* semantics)

### Executor (`src/exec.rs`)

**Primitives**:
- Unary: `map_numeric_preserve_tags` + column kernels
- Binary scalar: `binary_scalar_column` (broadcast)
- Binary frame: `binary_frame_frame` (element-wise, strict compatibility)
- Shift: `shift_column` (memmove-style, prefix NA)
- Joins: `reindex_by` (mapr), `asofr` (right outer asof join)

**Arc Preservation** (runtime verification):
```rust
debug_assert!(Arc::ptr_eq(&result.tags.index, &input.tags.index));
debug_assert!(Arc::ptr_eq(&result.tags.colnames, &input.tags.colnames));
debug_assert_eq!(result.nrows, input.nrows);
```

---

## Design Decisions (Frozen Contracts)

### 1. No Implicit Alignment (Table-First Semantics)

**Binary ops require exact compatibility**:
```lisp
;; ✅ VALID: Same frame, same shape
(+ x x)
(+ x 5.0)  ; scalar broadcast

;; ❌ ERROR: Different frames, different shapes
(+ x y)  ; "Use mapr/asofr for alignment"

;; ✅ VALID: Align first, then operate
(+ (mapr x y) 10.0)
```

**Rationale**:
- No hidden joins (performance predictable)
- No accidental lookahead
- "kdb-ish but honest"

### 2. Shift is Lag-Only (No Forward-Looking)

**Form**: `(shift k x)` where `k ≥ 0`

**Semantics**: `output[i] = input[i-k]` for i ≥ k, NA for i < k

**Rationale**:
- Prevents accidental lookahead **by construction**
- "Future" must be explicit (separate `lead` op if ever needed)
- Enforces temporal causality at language level

### 3. Conservative NA Policy

**Rules**:
- Binary op: if either cell NA → result NA
- Division by zero → NA
- Log of non-positive → NA
- Shift introduces NA in prefix (length k)
- **No invented values** (mask monotone)

**Validation**: dlog identity test catches violations

### 4. Arc Preservation (Zero-Copy Tags)

**Invariants (I1-I3)**:
- I1: `Arc::ptr_eq(&result.tags.index, &input.tags.index)`
- I2: `Arc::ptr_eq(&result.tags.colnames, &input.tags.colnames)`
- I3: `result.nrows == input.nrows`

**Verified**:
- Compile-time (planner schema tracking)
- Runtime (debug_assert in executor)
- Test-time (metamorphic Arc pointer equality tests)

---

## Completed Work (Step 3C.1 - 3C.3.1)

### Step 3A-3B: IR Equivalence + Differential Testing ✅

**Commits**:
- `b74ba5e` Add metamorphic property suite for IR equivalence
- `460f09e` Add differential execution tests (AST vs IR)

**Coverage**: 600 property cases (Date + Timestamp), metamorphic laws, differential oracles

---

### Step 3C.1: Scalars + Constants ✅

**Commits**:
- `230527d` Add binary numeric operations (+ - * /) with strict semantics

**IR**:
- `ValueRef::Scalar(f64)` for constants
- `BinaryOp::MapNumeric2 { lhs, rhs, func }`

**Planner**:
- Parse `Expr::Float` / `Expr::Int` → `ValueRef::Scalar`
- Lower `(+ x 5.0)` → Binary node

**Executor**:
- `binary_scalar_column`: element-wise with broadcast
- `binary_frame_frame`: element-wise strict compatibility

---

### Step 3C.2: Binary Numeric Ops ✅

**Commits**:
- `3505670` Add comprehensive binary operation tests

**Operations**: `+`, `-`, `*`, `/`

**Tests**: 7 smoke tests, 5 metamorphic laws

**Identity Laws Proven**:
- `x + 0 = x`
- `x * 1 = x`
- `x * 0 = 0` (valid), `NA` (NA input)

---

### Step 3C.3: Shift Operation ✅

**Commits**:
- `06c6f80` Add shift unary op (lag-only, contracts-grade)
- `02b6387` Add comprehensive shift operation tests

**Form**: `(shift k x)` where `k ≥ 0`

**Contract**:
- Lag k rows (move values down)
- Shape preserved (nrows/ncols unchanged)
- Arc preservation (I1-I3)
- NA mask monotone

**Tests**: 5 smoke tests, 5 metamorphic laws

**Laws Proven**:
- `shift(0, x) = x` (identity)
- `shift(a, shift(b, x)) = shift(a+b, x)` (composition)
- Mask monotonicity (NA positions only grow)
- Arc preservation (pointer equality)

---

### Step 3C.3.1: dlog Identity (Semantic Tripwire) ✅

**Commits**:
- `79fccd3` Add dlog identity metamorphic test (highest-leverage tripwire)

**Identity**: `dlog(x) == log(x / shift(1, x))`

**Validates Simultaneously**:
1. Shift sign convention (lag vs lead)
2. Division-by-zero → NA
3. NA propagation
4. Log domain handling
5. No off-by-one errors

**Tests**:
- Smoke test: Hand-crafted 5-row sequence
- Metamorphic: 20 rows, positive domain, 3-layer assertion

**Why Critical**: Proves temporal foundation before rolling ops

---

## Next Steps: Step 3C.4 - Rolling Operations

### Contract Design (Must Decide First)

**Operations to add**:
1. `rolling_mean(window, x)`
2. `rolling_std(window, x, ddof=1)`
3. `rolling_zscore(window, x)`

**Contract Questions** (freeze before implementation):

1. **Window definition**:
   - Inclusive or exclusive?
   - Trailing window (past k observations) or centered?
   - **Recommendation**: Trailing, inclusive (row i uses rows [i-k+1..i])

2. **min_periods behavior**:
   - Always require full window (NA if < k observations)?
   - Allow partial windows with `min_periods` parameter?
   - **Recommendation**: `min_periods=window` (require full window, simpler)

3. **NA handling**:
   - Skip NAs (compute mean of non-NA values in window)?
   - Poison (any NA in window → result NA)?
   - **Recommendation**: Skip NAs (more useful, matches pandas default)

4. **Prefix handling**:
   - First k-1 rows: always NA (not enough observations)?
   - **Recommendation**: Yes (conservative, matches shift prefix behavior)

5. **std ddof** (degrees of freedom):
   - ddof=0 (population std) or ddof=1 (sample std)?
   - **Recommendation**: ddof=1 (sample std, matches pandas/numpy default)

6. **zscore zero-std handling**:
   - If window has zero variance: return NA or 0?
   - **Recommendation**: Return NA (conservative, avoids division by zero)

### Implementation Plan

**Commit 1**: IR + Planner
- Add `RollingFunc { window: usize, func: RollingOp }`
- Parse `(rolling-mean k x)`, `(rolling-std k x)`, `(rolling-zscore k x)`
- Validate window > 0

**Commit 2**: Executor (rolling_mean)
- Implement `rolling_mean_column(col, window, min_periods)`
- Sliding window sum + count (skip NAs)
- Prefix NA handling

**Commit 3**: Executor (rolling_std)
- Welford's online algorithm (numerically stable)
- ddof=1 (sample std)
- Skip NAs

**Commit 4**: Executor (rolling_zscore)
- Compose mean + std
- Zero-std → NA

**Commit 5**: Tests
- Smoke tests (hand-crafted sequences)
- Metamorphic properties:
  - `rolling_mean(1, x) == x` (window=1 is identity)
  - `rolling_zscore` has mean ≈ 0, std ≈ 1 (when valid)
  - Mask monotonicity (rolling can only add NAs)
  - Composition with shift
- Differential tests

### Metamorphic Properties to Add

1. **Window=1 identities**:
   - `rolling_mean(1, x) == x`
   - `rolling_std(1, x) == 0` (all rows with 1 observation → zero variance)

2. **Prefix behavior**:
   - First `window-1` rows always NA

3. **Mask monotonicity**:
   - `mask(rolling_op(k, x)) ⊇ mask(x)` (can only add NAs, never remove)

4. **Zscore properties** (when valid):
   - `mean(rolling_zscore(k, x)[k:]) ≈ 0` (standardized)
   - `std(rolling_zscore(k, x)[k:]) ≈ 1` (unit variance)

5. **NA handling**:
   - Any NA in window propagates (if poison policy)
   - OR: result valid if enough non-NA values (if skip policy)

### Files to Modify

- `src/ir.rs`: Add `RollingFunc` enum
- `src/planner.rs`: Parse rolling ops
- `src/exec.rs`: Implement kernels
- `tests/ir_equivalence_smoke.rs`: Add smoke tests
- `tests/metamorphic.rs`: Add rolling metamorphic laws
- `tests/common/mod.rs`: Add `rolling_*_column` to direct_eval

---

## Key Files Reference

### Core Implementation
- `src/ir.rs` - IR node definitions, validation
- `src/planner.rs` - AST → IR lowering
- `src/exec.rs` - IR execution (primitives)
- `src/frame.rs` - Frame structure, operations

### Test Infrastructure
- `tests/common/mod.rs` - Test utilities, direct_eval, frame generators
- `tests/ir_equivalence.rs` - Property tests (600 cases)
- `tests/ir_equivalence_smoke.rs` - Deterministic smoke tests (35)
- `tests/metamorphic.rs` - Semantic tripwires (29)
- `tests/differential_exec.rs` - AST vs IR oracle (15)

### Documentation
- `BLADE_IR_STATUS.md` (this file) - Current status
- `contracts.md` (if exists) - Semantic contracts
- `README.md` - Project overview

---

## Test Invocation

```bash
# All IR tests (88 tests, ~1 second)
cargo test --test ir_equivalence --test ir_equivalence_smoke \
           --test metamorphic --test differential_exec

# Individual suites
cargo test --test ir_equivalence        # 9 tests (600 property cases)
cargo test --test ir_equivalence_smoke  # 35 tests
cargo test --test metamorphic           # 29 tests
cargo test --test differential_exec     # 15 tests

# Specific test
cargo test meta_dlog_identity_positive_domain
cargo test smoke_shift_zero_identity
```

---

## Current Commits (Step 3C)

```
79fccd3 Add dlog identity metamorphic test (highest-leverage tripwire)
02b6387 Add comprehensive shift operation tests
06c6f80 Add shift unary op (lag-only, contracts-grade)
3505670 Add comprehensive binary operation tests
230527d Add binary numeric operations (+ - * /) with strict semantics
460f09e Add differential execution tests (AST vs IR)
b74ba5e Add metamorphic property suite for IR equivalence
```

---

## Ready State

**IR v1 is production-ready for**:
- Time series transformations (dlog, ret, shift)
- Binary arithmetic (strict semantics)
- Join operations (explicit alignment)
- Let-bindings (compositional pipelines)

**Proven via**:
- 88 tests, 800+ property cases
- Metamorphic laws (contracts.md)
- dlog identity (temporal correctness)
- Differential oracles (AST vs IR)

**Next**: Rolling window operations (mean, std, zscore) to complete time-series foundation.

**Status**: ✅ **Ready for Step 3C.4**

---

*Document maintained by: Claude Sonnet 4.5*
*Last test run: 2026-02-20, all tests passing*
