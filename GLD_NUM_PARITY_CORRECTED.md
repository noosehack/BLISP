# GLD_NUM Parity - CORRECTED Gap Analysis

**Date**: 2026-02-21 (corrected after code inspection)

---

## The GLD_NUM Expression

```lisp
(let* ((s (-> (stdin) (WKD) (dlog) (x- 1) (cs1) (wzs 25 1) (> -1) (shift 2))))
  (-> (file "GC1C.csv") (mapr s) (dlog) (ur 250 5) (* s) (cs1)))
```

---

## Operation-by-Operation Breakdown

### PART 1: Signal `s`

| Op | Actual Meaning | IR Status | Type |
|----|----------------|-----------|------|
| `stdin` | Read CSV from stdin | ❌ Missing | Source |
| `WKD` | Macro → `locf` (fill forward) | ❌ Missing | Unary (shape-preserving) |
| `dlog` | Daily log returns | ✅ IN IR | Unary |
| `x- 1` | Pairwise spread: col[i] - col[1] | ❌ Missing | **Schema transform** |
| `cs1` | **Cumulative sum** (starts at 1.0) | ❌ Missing | Unary (shape-preserving) |
| `wzs 25 1` | Macro → `rolling-zscore` | ✅ IN IR (rewrite) | Unary |
| `> -1` | **Comparison mask**: x > -1 → 1.0/0.0 | ❌ Missing | Binary comparison |
| `shift 2` | Lag 2 periods | ✅ IN IR | Unary |

### PART 2: Output

| Op | Actual Meaning | IR Status | Type |
|----|----------------|-----------|------|
| `file "GC1C.csv"` | Load CSV | ✅ IN IR | Source |
| `mapr s` | Align onto signal index | ✅ IN IR | Join |
| `dlog` | Log returns | ✅ IN IR | Unary |
| `ur 250 5` | Unit ratio: val/(100*√252*rolling_std) | ❌ Missing | Derived |
| `* s` | Multiply by signal | ✅ IN IR | Binary |
| `cs1` | **Cumulative sum** | ❌ Missing | Unary |

---

## Corrected Understanding

### ✅ Shape-Preserving Operations (I1-I3 preserved)

**Already in IR:**
- `dlog`, `shift`, `mapr`, `*`, `rolling-zscore`, `let*`

**Need to add (shape-preserving):**
- `stdin` - Source node
- `locf` (WKD) - Fill forward NA
- `cs1` - Cumulative sum (scan)
- `ecs1` - Exponential cumsum: exp(cumsum(dlog))
- `>` - Comparison → numeric mask (1.0/0.0)

### 🔄 Schema-Transforming Operations (I2' - new colnames)

**Need to add:**
- `x-` (xminus) - Pairwise spread (reduces ncols by 1)

### 🎯 Derived Forms (rewrite to existing ops)

**Already works:**
- `wzs` → `rolling-zscore` (just macro mapping)

**Can derive:**
- `ur` → `x / (100 * sqrt(252) * rolling-std(w, x))`
- `ecs1` → `exp(cs1(dlog(x)))` (after cs1 exists)

---

## Key Semantic Corrections

### A) `>` is Comparison, NOT Filtering

**WRONG understanding**: Row filtering (breaks I1-I3)
**CORRECT understanding**: Element-wise comparison → mask

**Semantics**:
```lisp
(> x -1)  ; Returns frame same shape as x
          ; Values: 1.0 where x > -1
          ;         0.0 where x <= -1
          ;         NA where x is NA
```

**Usage in GLD_NUM**:
```lisp
(shift 2 (> (wzs ...) -1))  ; Mask, then lag
; Multiply later: (* value mask) filters via arithmetic
```

**Shape preserving**: ✅ I1-I3 hold
**Row filtering**: Separate operation (not needed for GLD_NUM!)

---

### B) `cs1` is Cumulative Sum, NOT Cross-Sectional Z-Score

**From code** (`clispi_dev/blawk.cpp`):
```cpp
// cs1 - Cumulative sum (starts at 1.0)
```

**Semantics**:
```
cs1(x)[0] = 1.0
cs1(x)[i] = cs1(x)[i-1] + x[i]
```

**Purpose**: Running total (common in index reconstruction)

**ecs1**: Exponential cumsum = `exp(cumsum(dlog(prices)))` (price index from returns)

**If you need cross-sectional zscore**: Name it `xzs` or `csz`, NOT cs1!

---

## Revised Implementation Priority

### Phase 1: Quick Wins (hours)

**1. stdin** (30 min)
```rust
// src/ir.rs
pub enum Source {
    File { path: String },
    Stdin,  // NEW
    Variable { name: SymbolId },
}

// src/exec.rs
Source::Stdin => {
    let stdin = std::io::stdin();
    io::read_csv_from_reader(BufReader::new(stdin.lock()), &rt.interner)?
}
```

**2. wzs macro** (15 min)
```lisp
; stdlib/compat_clispi.cl
(defmacro wzs (w l x) `(rolling-zscore ,w ,x))  ; Map to IR
```

**3. `>` comparison** (1 hour)
```rust
// src/ir.rs
pub enum BinaryFunc {
    Add, Sub, Mul, Div,
    Gt,  // NEW: greater than (returns 1.0/0.0/NA mask)
}

// Kernel
fn gt_mask(lhs: &Column, rhs_scalar: f64) -> Column {
    lhs.iter().map(|&x| {
        if x.is_nan() { NA }
        else if x > rhs_scalar { 1.0 }
        else { 0.0 }
    }).collect()
}
```

**Tests**: NA propagation, mask arithmetic `(* x (> x 0))`

---

### Phase 2: GLD_NUM Blockers (medium)

**4. locf** (1-2 hours)
```rust
pub enum NumericFunc {
    // ...
    Locf,  // Last observation carried forward
}

fn locf_column(col: &Column) -> Column {
    let mut last_valid = NA;
    col.iter().map(|&x| {
        if !x.is_nan() { last_valid = x; }
        last_valid
    }).collect()
}
```

**Tests**: Idempotence `locf(locf(x)) == locf(x)`, NA prefix

**5. cs1** (cumsum) (2 hours)
```rust
pub enum NumericFunc {
    // ...
    CumSum,  // Cumulative sum (starts at 1.0 or 0.0?)
}

fn cumsum_column(col: &Column) -> Column {
    let mut running = 1.0;  // or 0.0 - check clispi semantics!
    col.iter().map(|&x| {
        if !x.is_nan() { running += x; }
        running
    }).collect()
}
```

**Tests**: Deterministic sequence, NA handling

**6. x-** (pairwise spread) (2-3 hours)
```rust
// Schema-transforming op (breaks I2)
pub enum Operation {
    // ...
    PairwiseSpread { input: NodeId, half: usize },  // NEW
}

// Executor: ncols_out = ncols_in - 1
// colnames_out = new generated names (col2-col1, col3-col1, ...)
```

**Tests**: Schema correctness, symmetry

---

### Phase 3: Derived Forms (easy)

**7. ur** (30 min - rewrite)
```lisp
(defmacro ur (w step x)
  `(/ ,x (* 100.0 (* (sqrt 252.0) (rolling-std ,w ,x)))))
```

**8. ecs1** (15 min - rewrite)
```lisp
(defmacro ecs1 (x) `(exp (cs1 (dlog ,x))))
```

---

## GLD_NUM Coverage After Phase 1+2

| Operation | Status After Implementation |
|-----------|------------------------------|
| stdin | ✅ Phase 1 |
| WKD (locf) | ✅ Phase 2 |
| dlog | ✅ Already in IR |
| x- | ✅ Phase 2 |
| cs1 | ✅ Phase 2 |
| wzs | ✅ Phase 1 (macro) |
| > | ✅ Phase 1 |
| shift | ✅ Already in IR |
| file | ✅ Already in IR |
| mapr | ✅ Already in IR |
| ur | ✅ Phase 3 (derived) |
| * | ✅ Already in IR |

**Result**: Full GLD_NUM runs on IR executor! 🎉

---

## Estimated Work

| Phase | Time | Impact |
|-------|------|--------|
| Phase 1 (stdin, wzs, >) | 2 hours | Unlocks many pipelines |
| Phase 2 (locf, cs1, x-) | 6 hours | GLD_NUM works |
| Phase 3 (ur, ecs1 rewrites) | 1 hour | Full parity |
| **Total** | **~9 hours** | **GLD_NUM on IR** |

Previous estimate (13 hours) was inflated by:
- ❌ Row filtering (not needed - `>` is mask!)
- ❌ Cross-sectional ops (cs1 is cumsum!)
- ❌ Complex regression (ur is just division!)

---

## Testing Strategy

### Metamorphic Laws to Add

**Comparison**:
- `(> x c) * (> x c) == (> x c)` (idempotence)
- `(> x c1) * (> x c2) == (> x (max c1 c2))` (conjunction)

**Cumsum**:
- `cs1(0) == 1.0` (or 0.0 - verify)
- `cs1(x + y) == cs1(x) + cs1(y) - 1.0` (linearity-ish)

**Locf**:
- `locf(locf(x)) == locf(x)` (idempotence)
- Leading NA preserved until first valid

---

## Migration to IR-Default

**Current**: Hybrid mode (try IR, fallback to legacy)

**After Phase 1+2**:
```bash
# Default: IR executor (fast!)
./blisp -e '...'

# Explicit legacy (if needed)
./blisp --legacy -e '...'

# Help: show supported IR operations
./blisp --help-ir
```

**In planner**: Define authoritative list
```rust
const SUPPORTED_IR_OPS: &[&str] = &[
    "stdin", "file", "dlog", "ret", "log", "exp", "sqrt", "abs", "inv",
    "shift", "rolling-mean", "rolling-std", "rolling-zscore",
    "+", "-", "*", "/", ">",  // NEW
    "mapr", "asofr", "let*",
    "locf", "cs1",  // NEW
];
```

---

## Summary of Corrections

**What I got wrong**:
1. ❌ `>` is NOT row filtering → ✅ It's comparison (mask)
2. ❌ cs1 is NOT cross-sectional zscore → ✅ It's cumulative sum
3. ❌ ur is NOT rolling regression → ✅ It's unit ratio (return/vol)

**Impact**:
- Simpler implementation (no I1-I3 breaking ops!)
- Faster development (~9 hours vs 13)
- GLD_NUM actually works with just shape-preserving ops!

---

*Corrected analysis: 2026-02-21*
*Ready to implement Phase 1 (quick wins)*
