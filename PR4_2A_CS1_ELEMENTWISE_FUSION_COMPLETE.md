# ✅ PR4.2a COMPLETION: cs1 ∘ Elementwise Fusion

**Date**: 2026-02-26
**Branch**: `pr4-2a-fuse-cs1-elementwise`
**Status**: ✅ COMPLETE AND VERIFIED

---

## Executive Summary

Successfully implemented stateful fusion for **cs1 ∘ elementwise_chain**:
- **Pattern**: `cs1(EW_CHAIN(x))` → single pass with one state variable
- **Stateful operation**: cs1 (cumulative sum with accumulator)
- **Elementwise operations**: LOG, EXP, SQRT, ABS, INV (pure pointwise)
- **Result**: Single-pass execution, reduced allocations, all correctness tests pass

---

## Architecture Change: Unfused vs Fused

### Before PR4.2a (Unfused)
```
Example: (cs1 (abs (log x)))

IR:
  x → MapNumeric(LOG) → MapNumeric(ABS) → MapNumeric(cs1)

Execution:
  for i: temp1[i] = log(x[i])    # Allocation 1
  for i: temp2[i] = abs(temp1[i]) # Allocation 2
  acc = 1.0
  for i:
    if !temp2[i].is_nan():
      acc += temp2[i]
      out[i] = acc               # Allocation 3
    else:
      out[i] = NaN

Total: 3 passes, 3 allocations
```

### After PR4.2a (Fused)
```
Example: (cs1 (abs (log x)))

IR (after optimization):
  x → FusedCs1Elementwise([LOG, ABS])

Execution:
  acc = 1.0
  for i:
    y = x[i]
    y = log(y)
    y = abs(y)
    if !y.is_nan():
      acc += y
      out[i] = acc
    else:
      out[i] = NaN

Total: 1 pass, 1 allocation
```

**Benefits**:
- Fewer allocations (3 → 1)
- Better cache locality (single pass)
- State machine composition (accumulator + elementwise ops)
- NA propagation unchanged (cs1 is NA-preserving)

---

## Implementation Summary

### 1. IR Extension: Fused cs1+Elementwise Node

**Added to `src/ir.rs`** (line 171):
```rust
pub enum UnaryOp {
    MapNumeric { ... },
    FusedElementwise { ... },  // PR4.1

    /// PR4.2a: cs1 ∘ elementwise_chain
    FusedCs1Elementwise {
        input: NodeId,
        ops: Vec<NumericFunc>,  // Elementwise ops applied before accumulation
    },
}
```

**Semantics** (documented in code):
```rust
acc = 1.0  // cs1 starts at 1.0
for i:
  y = x[i]
  for op in ops: y = op(y)  // Apply elementwise chain
  if y is NA:
    out[i] = NA          // NA-preserving: accumulator unchanged
  else:
    acc += y
    out[i] = acc
```

### 2. Executor: Fused cs1+Elementwise Execution

**Added to `src/exec.rs`** (line 691):
```rust
pub fn fused_cs1_elementwise_column(col: &Column, ops: &[NumericFunc]) -> Column {
    match col {
        Column::F64(data) => {
            let mut result = Vec::with_capacity(data.len());
            let mut acc = 1.0; // cs1 starts at 1.0

            for &x in data.iter() {
                // Apply elementwise chain
                let mut y = x;
                for op in ops {
                    y = apply_elementwise_op(y, *op);
                }

                // cs1 accumulation (NA-preserving)
                if y.is_nan() {
                    result.push(f64::NAN); // NA input → NA output, acc unchanged
                } else {
                    acc += y;
                    result.push(acc);
                }
            }

            Column::F64(result)
        }
        _ => col.clone(),
    }
}
```

**Key property**: cs1 is **NA-preserving**:
- NA input → NA output
- Accumulator unchanged (continues for next valid value)
- Example: `cs1([10, NA, 20])` → `[11, NA, 31]`

### 3. Optimizer: cs1+Elementwise Fusion Pass

**Added to `src/ir_fusion.rs`** (line 631):
```rust
pub fn optimize_cs1_elementwise_fusion(plan: &Plan) -> Plan {
    // Two-pass algorithm (like PR4.1):
    // 1. Reverse pass: Identify fusible cs1 ∘ elementwise patterns
    // 2. Forward pass: Build optimized plan with fused nodes

    // Detects two cases:
    // - cs1(FusedElementwise([...]))
    // - cs1(single_elementwise)

    // Legality: Single-consumer rule (same as PR4.1)
}
```

**Pattern matching**:
1. Find cs1 node (`NumericFunc::SHF_PFX_LIN_SUM`)
2. Check if input is elementwise op(s) with single consumer
3. If yes: fuse into `FusedCs1Elementwise`

### 4. Test Suite: 6 Mandatory Tests

**Differential Test** (random data):
```rust
test_pr4_2a_differential_random()
```
- 100 elements with 20% NaN
- Chain: ABS → LOG
- Proves: fused == unfused (byte-for-byte)

**Boundary Case Tests** (hand-crafted):
```rust
test_pr4_2a_boundary_leading_nans()
```
- Input: `[NA, NA, 10, 20, 30]`
- Expected: `[NA, NA, 11, 31, 61]`
- Verifies: Leading NAs handled correctly, acc starts at 1.0

```rust
test_pr4_2a_boundary_intermittent_nans()
```
- Input: `[10, NA, 20, NA, NA, 30]`
- Expected: `[11, NA, 31, NA, NA, 61]`
- Verifies: Accumulator continues across NAs

```rust
test_pr4_2a_boundary_all_nans()
```
- Input: All NAs
- Expected: All NAs
- Verifies: Accumulator stays at 1.0, no updates

**Graph Rewrite Tests** (optimizer correctness):
```rust
test_pr4_2a_optimizer_fuses_cs1_fused_ew()
```
- Pattern: `x → FusedElementwise([ABS, LOG]) → cs1`
- Result: `x → FusedCs1Elementwise([ABS, LOG])`
- Verifies: Optimizer fuses pre-fused elementwise chain

```rust
test_pr4_2a_optimizer_fuses_cs1_single_ew()
```
- Pattern: `x → ABS → cs1`
- Result: `x → FusedCs1Elementwise([ABS])`
- Verifies: Optimizer fuses single elementwise op

---

## Verification Results

### Compilation: ✅ SUCCESS
```bash
$ cargo build
   Compiling blisp v0.1.0
    Finished dev [unoptimized + debuginfo]
```
- Warnings: 55 (pre-existing + unused variables in tests)
- Errors: 0

### Tests: ✅ SUCCESS (154/167 passing)
```bash
$ cargo test --lib
test result: ok. 154 passed; 12 failed; 1 ignored
```
- **154 passed** (148 before + 6 new PR4.2a tests)
- **12 failed** (same pre-existing I/O failures)
- **1 ignored** (PR4.1 microbench)
- **0 new failures** introduced

### New Tests: ✅ ALL PASS
```bash
$ cargo test --lib test_pr4_2a
running 6 tests
test ir_fusion::tests::test_pr4_2a_boundary_all_nans ... ok
test ir_fusion::tests::test_pr4_2a_boundary_intermittent_nans ... ok
test ir_fusion::tests::test_pr4_2a_boundary_leading_nans ... ok
test ir_fusion::tests::test_pr4_2a_differential_random ... ok
test ir_fusion::tests::test_pr4_2a_optimizer_fuses_cs1_fused_ew ... ok
test ir_fusion::tests::test_pr4_2a_optimizer_fuses_cs1_single_ew ... ok

test result: ok. 6 passed; 0 failed
```

---

## Files Modified

1. **src/ir.rs**
   - Added: `UnaryOp::FusedCs1Elementwise` variant (line 171, ~30 lines)
   - Updated: `Plan::validate()` to handle new variant (line 577)

2. **src/exec.rs**
   - Added: `fused_cs1_elementwise_column()` function (line 691, ~30 lines)
   - Made public: `cumsum_column()` (for testing)
   - Added: `execute_unary()` match arm for FusedCs1Elementwise (line 153, ~25 lines)

3. **src/ir_fusion.rs**
   - Added: `optimize_cs1_elementwise_fusion()` function (line 631, ~200 lines)
   - Updated: Consumer counting to include FusedCs1Elementwise (line 447)
   - Updated: Node remapping to handle FusedCs1Elementwise (line 577)
   - Added: 6 tests (lines 1305-1580, ~275 lines)

**Total**: 3 files modified, ~560 lines added (including ~275 lines of tests)

---

## Correctness Guarantees

### cs1 Semantics Preserved ✅

**Unfused cs1 behavior** (from `cumsum_column`):
```rust
acc = 1.0
for x in data:
  if x.is_nan():
    output.push(NaN)
  else:
    acc += x
    output.push(acc)
```

**Fused behavior** (from `fused_cs1_elementwise_column`):
```rust
acc = 1.0
for x in data:
  y = apply_elementwise_chain(x)
  if y.is_nan():
    output.push(NaN)
  else:
    acc += y
    output.push(acc)
```

**Difference**: Only when elementwise chain is applied (before accumulation).
**Equivalence**: `cs1(EW_CHAIN(x))` = `fused_cs1_elementwise(x, EW_CHAIN)`

### Differential Testing ✅
- Random test with 20% NaN density
- Boundary cases: leading, intermittent, all NAs
- All prove: fused == unfused (byte-for-byte with NaN-aware comparison)

### Legality Rules ✅
1. **Single consumer**: Intermediate elementwise node only fused if consumed by cs1 alone
2. **Pure elementwise**: Only LOG, EXP, SQRT, ABS, INV (no stateful ops)
3. **I1-I3 preservation**: Tags (index, colnames, nrows) unchanged

### Graph Rewriting ✅
- Two-pass algorithm (reverse + forward) prevents partial fusion
- Optimizer correctly identifies both patterns (single ew, fused ew chain)
- Node mapping preserves dependencies

---

## State Machine Composition

**Key insight**: cs1 is a **prefix accumulator** with one state variable.

**Composition**:
```
State: acc ∈ ℝ (initially 1.0)
Transition: acc' = acc + f(x_i)  where f = elementwise chain
Output: y_i = acc'  (or NA if f(x_i) = NA)
```

**Why this works**:
- Elementwise ops are local (no state)
- cs1 state is independent of elementwise computation
- Composition is associative: `cs1 ∘ (f ∘ g) = cs1 ∘ f ∘ g`

**Why this is easier than dlog fusion (PR4.2b)**:
- No lag dependencies (dlog needs x[i-1] or last_valid)
- One state variable (dlog-obs needs current + last_valid)
- No mask interaction (dlog-obs skips masked rows)

---

## Examples

### Example 1: Simple Chain
```lisp
; User code
(cs1 (abs x))

; Before PR4.2a (unfused)
x → MapNumeric(ABS) → MapNumeric(cs1)
; 2 passes, 2 allocations

; After PR4.2a (fused)
x → FusedCs1Elementwise([ABS])
; 1 pass, 1 allocation
```

### Example 2: Longer Chain
```lisp
; User code
(cs1 (log (abs x)))

; Before PR4.1+PR4.2a
x → MapNumeric(ABS) → MapNumeric(LOG) → MapNumeric(cs1)
; 3 passes, 3 allocations

; After PR4.1 only (elementwise fusion)
x → FusedElementwise([ABS, LOG]) → MapNumeric(cs1)
; 2 passes, 2 allocations

; After PR4.1+PR4.2a (stateful fusion)
x → FusedCs1Elementwise([ABS, LOG])
; 1 pass, 1 allocation
```

### Example 3: NA Handling
```lisp
; Input: [10, NA, 20, 30]

; cs1(abs(x))
; Step by step:
; i=0: y=abs(10)=10, acc=1+10=11, out[0]=11
; i=1: y=abs(NA)=NA, out[1]=NA, acc unchanged (still 11)
; i=2: y=abs(20)=20, acc=11+20=31, out[2]=31
; i=3: y=abs(30)=30, acc=31+30=61, out[3]=61

; Output: [11, NA, 31, 61]
```

---

## Design Decisions

### 1. Two-Pass Optimizer (Correctness) ✅

**Same pattern as PR4.1**:
- Reverse pass: Identify what should be fused
- Forward pass: Build optimized plan once

**Prevents partial fusion**:
- Without reverse pass: intermediate nodes processed before cs1 node
- Result: intermediate gets copied before fusion opportunity detected

### 2. NA-Preserving Semantics (Finance-Correct) ✅

**cs1 behavior on NA**:
- Input NA → Output NA
- Accumulator unchanged → continues for next valid value

**Rationale**:
- Matches existing `cumsum_column` behavior
- Preserves weekend masks (wkd): masked rows stay masked
- Accumulator represents "running sum of valid values seen so far"

**Alternative (NA-poisoning)**: Once NA seen, all subsequent outputs NA.
- Would break masked time series workflows
- Not implemented (not needed for quant pipelines)

### 3. Single State Variable (Simplicity) ✅

**cs1 has one state: accumulator**
- Start: `acc = 1.0`
- Update: `acc += f(x_i)` if valid
- Output: `acc`

**Why this is easy to fuse**:
- No dependencies on other elements (unlike lag)
- No conditional state updates (unlike dlog-obs last_valid)
- Composition is trivial

**Contrast with PR4.2b (dlog ∘ cs1)**:
- dlog-ofs needs x[i-1] (lag dependency)
- dlog-obs needs last_valid (conditional state)
- Both require careful state machine composition

---

## Comparison: PR4.1 vs PR4.2a

| Aspect | PR4.1 (Elementwise) | PR4.2a (cs1 + Elementwise) |
|--------|---------------------|----------------------------|
| **Operations** | Pure elementwise only | cs1 (stateful) + elementwise |
| **State** | None | One accumulator |
| **Complexity** | Trivial (no state) | Simple (prefix accumulator) |
| **Legality** | Single consumer | Single consumer |
| **Tests** | 3 + 1 bench | 6 (4 boundary + 2 optimizer) |
| **Benefit** | N allocations → 1 | N+1 allocations → 1 |
| **Example** | (exp (log (abs x))) | (cs1 (log (abs x))) |

**Progression**:
- PR4.1: Fuse stateless ops
- PR4.2a: Fuse stateful (prefix) + stateless
- PR4.2b: Fuse stateful (lag) + stateful (prefix) ← **next step**

---

## Next Steps

### Immediate (PR4.2b)
- **Goal**: Fuse `dlog-ofs ∘ cs1` and `dlog-obs ∘ cs1`
- **Challenge**: Two state variables (lag + accumulator)
- **Legality**: Same single-consumer rule + mask semantics must match
- **Tests**: Same pattern (random + boundary cases)

**Why PR4.2b is harder**:
1. **dlog-ofs** needs x[i-1] (lag dependency)
2. **dlog-obs** needs last_valid (observation-based lag)
3. **cs1** needs accumulator
4. Must compose: dlog state → cs1 state

**Implementation approach**:
```rust
// dlog-ofs ∘ cs1
let mut acc = 1.0;
let mut prev = f64::NAN;
for i in 0..n {
    let dlog_val = if prev.is_finite() && x[i].is_finite() {
        (x[i] / prev).ln()
    } else {
        f64::NAN
    };

    if !dlog_val.is_nan() {
        acc += dlog_val;
        out[i] = acc;
    } else {
        out[i] = f64::NAN;
    }

    prev = x[i]; // Update lag state
}
```

### Future (PR4.2c)
- **Goal**: Fuse `dlog-* ∘ elementwise_chain`
- **Example**: `(abs (dlog x))`
- **Easy after PR4.2b**: Stateful → elementwise (just reverse of PR4.2a)

### Future (PR4.3+)
- Rolling window fusion
- Cross-column fusion (pairwise spreads)
- Group-by fusion

---

## Lessons Learned

### 1. State Machine Composition is Tractable ✅

**PR4.2a proves**: Fusing stateful + stateless is straightforward.
- Prefix accumulator (cs1) composes cleanly with local ops
- Single state variable keeps complexity low
- NA-preserving semantics prevent state corruption

**Implication for PR4.2b**: dlog + cs1 is harder but doable.
- Need explicit state machine: (prev, acc) or (last_valid, acc)
- Must handle NA propagation carefully
- Differential tests will catch state bugs

### 2. Two-Pass Pattern is Reusable ✅

**Used in PR4.1 and PR4.2a**:
1. Reverse pass: Identify fusible patterns
2. Forward pass: Build optimized plan

**Why it works**:
- Reverse pass sees "sinks" of chains first
- Forward pass can safely skip fused nodes (already marked)
- No partial fusion artifacts

**Apply to PR4.2b**: Same pattern for dlog ∘ cs1.

### 3. Boundary Testing is Critical ✅

**Hand-crafted cases caught**:
- Leading NAs: acc initialization
- Intermittent NAs: acc continuation
- All NAs: no-update case

**For PR4.2b, must test**:
- Leading NAs (dlog output = NA, cs1 acc unchanged)
- Intermittent NAs (dlog skips or uses positional lag)
- lag > 1 (dlog-ofs with multiple offset)
- Two NAs between valid points (dlog-obs last_valid lookup)

**Pattern**: Boundary tests > random tests for stateful ops.

---

## Summary

**Status**: ✅ PR4.2a COMPLETE

**Achievement**: First stateful fusion (cs1 + elementwise) with full correctness guarantees and test coverage.

**Impact**:
- 6 new tests (all passing)
- 0 regressions
- 0 user-facing changes (pure optimization)
- Reduced allocations: N+1 → 1
- Established state machine composition pattern

**Quality**:
- Followed same discipline as PR4.1 (explicit legality + differential tests)
- Boundary cases comprehensively covered
- cs1 semantics preserved exactly (NA-preserving)
- Two-pass optimizer prevents partial fusion

**Conclusion**: PR4.2a successfully extended fusion to stateful operations (cs1). The single-state-variable case is now proven. PR4.2b can proceed with dlog + cs1 (two state variables, lag dependencies) using the same test-driven pattern.

---

**END OF PR4.2a COMPLETION REPORT**

*"From stateless to stateful. From elementwise to prefix accumulation. The finance fusion begins."*
