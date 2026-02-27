# BLISP 3-Layer Operation Dispatch Model

**Date:** 2026-02-27
**Repository:** /home/ubuntu/blisp
**Branch:** reconstruct/tableview-only

---

## Executive Summary

BLISP uses a **3-layer dispatch architecture** for operation execution:

1. **Layer 1: Macros/Normalization** - Surface syntax transformation (e.g., `->` threading)
2. **Layer 2: Legacy Builtins** - Direct AST evaluation for general Lisp (eval.rs → builtins.rs)
3. **Layer 3: IR Planner** - High-performance Frame pipeline (planner.rs → ir.rs → exec.rs)

The default **HYBRID mode** tries IR first (for Frame ops), then falls back to legacy (for general Lisp).

---

## Layer Definitions

### Layer 1: Macros / Surface Syntax (User Input)

**Location:** `src/normalize.rs` (lines 1-145)

- **Purpose:** Transform surface syntax into canonical forms before evaluation
- **Key transformation:** Thread-first macro `(->)` expansion
- **Entry point:** `normalize::normalize()` called from `main.rs:545`

**Example:**
```lisp
(-> data (dlog) (shift 1))
;; Expands to:
(shift 1 (dlog data))
```

**Code Flow:**
```
User input → normalize.rs:40 normalize()
          → normalize.rs:45 normalize_expr()
          → normalize.rs:53-57 check for "->" macro
          → normalize.rs:85 normalize_thread_first()
          → Returns canonical form
```

---

### Layer 2: Legacy Builtin Dispatch (AST Evaluator)

**Location:** `src/eval.rs` + `src/builtins.rs`

- **Purpose:** Direct AST evaluation for general Lisp (defparameter, if, let*, macros, builtin functions)
- **Entry point:** `Runtime::eval()` at `eval.rs:11`
- **Builtin check:** `eval.rs:82` checks `is_builtin()`
- **Builtin call:** `eval.rs:90` calls `call_builtin()`
- **Registration:** `builtins.rs:119-222` in `register_builtins()`

**Code Flow:**
```
eval.rs:11  Runtime::eval()
  ↓
eval.rs:36  eval_list()
  ↓
eval.rs:47-59  Check for macro expansion
  ↓
eval.rs:66-79  Check for special forms (quote, progn, if, let*, etc.)
  ↓
eval.rs:82  is_builtin() check
  ↓
eval.rs:84-87  Evaluate arguments
  ↓
eval.rs:90  call_builtin(sym, args)
  ↓
builtins.rs:XXX  fn builtin_XXX()
```

**Registered Builtins (examples):**
- `builtins.rs:121`: `+` → `builtin_add`
- `builtins.rs:186`: `wkd` → `builtin_wkd`
- `builtins.rs:187`: `w5` → `builtin_wkd` (alias)
- `builtins.rs:133`: `dlog-col` → `builtin_dlog`
- `builtins.rs:162`: `dlog-cols` → `builtin_dlog_cols`

---

### Layer 3: IR Planner Path (Frame Pipeline)

**Location:** `src/planner.rs` → `src/ir.rs` → `src/exec.rs`

- **Purpose:** High-performance Frame operations with schema validation and fusion optimization
- **Entry point:** `main.rs:543` calls `try_ir_eval()`
- **Pipeline:** normalize → plan → execute

**Code Flow:**
```
main.rs:543  try_ir_eval(rt, expr)
  ↓
main.rs:545  normalize::normalize()  [Layer 1]
  ↓
main.rs:548  planner::plan()
  ↓
planner.rs:42  plan(expr, interner)
  ↓
planner.rs:57  plan_expr() - recursive planning
  ↓
planner.rs:86  match func_name:
               - "file" → Source::File
               - "stdin" → Source::Stdin
               - "dlog" → NumericFunc::SHF_PTW_OBS_NLN_DLOG
               - "wkd" → NumericFunc::MSK_WKE
               - "shift" → NumericFunc::SHF_PTW_LIN_SHF
               - etc.
  ↓
planner.rs:42-50  Create IR Plan (DAG of nodes)
  ↓
main.rs:551  exec::execute(plan, rt)
  ↓
exec.rs:48  execute() - iterate over nodes
  ↓
exec.rs:65  execute_node()
  ↓
exec.rs:70-76  match node.op:
               - Operation::Source → execute_source()
               - Operation::Unary → execute_unary()
               - Operation::Binary → execute_binary()
               - Operation::Join → execute_join()
               - Operation::Schema → execute_schema()
  ↓
exec.rs:125-192  execute_unary() - apply kernel via map_numeric_preserve_tags()
  ↓
exec.rs:157-170  match NumericFunc:
                 - SHF_PTW_OBS_NLN_DLOG → dlog_obs_column()
                 - MSK_WKE → wkd_mask_weekends()
                 - SHF_PTW_LIN_SHF → shift_column()
                 - etc.
```

**IR Enums (ir.rs):**
- `Operation::Source(Source::File)` - Load CSV
- `Operation::Source(Source::Stdin)` - Read stdin
- `Operation::Unary(UnaryOp::MapNumeric { func })` - Map function over columns
- `NumericFunc::SHF_PTW_OBS_NLN_DLOG` - Diff log (observation-based)
- `NumericFunc::MSK_WKE` - Weekend mask (wkd)
- `NumericFunc::SHF_PTW_LIN_SHF { k }` - Shift by k

---

## Selection Logic (HYBRID Mode - Default)

**File:** `main.rs:572-594`

```rust
// 🎯 HYBRID mode (DEFAULT):
// Try IR first for Frame operations, fall back to legacy for general Lisp
match try_ir_eval(rt, expr.clone()) {
    Ok(val) => {
        // ✅ IR succeeded (Frame pipeline)
        // Benefits:
        // - O(n) rolling operations (6-102x faster)
        // - Fusion framework ready
        // - Schema validation at plan time
        // - All 116 IR tests enforcing correctness
        result = val;
    }
    Err(e) if e.contains("Cannot plan") ||
              e.contains("not supported") ||
              e.contains("Unknown function") => {
        // IR can't handle this expression → fallback to legacy
        // This is NORMAL for general Lisp (defparameter, if, let*, etc.)
        result = rt.eval(&expr)?;
    }
    Err(e) => {
        // IR failed with real error → propagate
        return Err(e);
    }
}
```

**Selection Conditions:**
- **IR succeeds:** Frame/TableView operations with recognized functions
- **Legacy fallback:** Scalars, special forms, defparameter, macros, unrecognized ops
- **Error propagation:** Real IR errors (schema mismatches, type errors)

---

## Example 1: `(file "tto.csv")`

### A) Dispatch Path
**IR Planner (Layer 3) - IR-ONLY**

### B) Complete Mapping Chain

```
User Input: (file "tto.csv")
    ↓
[Layer 1: Normalization]
    normalize.rs:40 normalize()
    → No macro expansion (not a ->)
    → Returns: (file "tto.csv")
    ↓
[Layer 3: IR Path]
    main.rs:574 try_ir_eval()
    → main.rs:545 normalize::normalize()
    → main.rs:548 planner::plan()
        planner.rs:42 plan()
        → planner.rs:57 plan_expr()
        → planner.rs:88 match "file"
        → planner.rs:98-104 creates Source::File node
    → main.rs:551 exec::execute()
        exec.rs:48 execute()
        → exec.rs:71 execute_source()
        → exec.rs:82-88 Source::File case
        → **KERNEL:** io::load_csv() (exec.rs:84)
```

### C) File+Line Anchors

| Hop | File | Line | Description |
|-----|------|------|-------------|
| Entry | main.rs | 574 | `try_ir_eval()` |
| Normalize | normalize.rs | 40 | `normalize()` |
| Plan | planner.rs | 42 | `plan()` entry |
| Match | planner.rs | 88 | `"file"` case |
| IR Node | planner.rs | 99-104 | `Source::File { path }` creation |
| Execute | exec.rs | 48 | `execute()` entry |
| Dispatch | exec.rs | 71 | `execute_source()` |
| Kernel | exec.rs | 84 | `io::load_csv()` |

**IR Enum:** `Operation::Source(Source::File { path })`
**Legacy Builtin:** **MISSING** - No legacy path
**Status:** IR-only operation (new design)

---

## Example 2: `(w5 <expr>)` / `(wkd <expr>)`

### A) Dispatch Path
**DUAL PATH** - IR preferred, legacy fallback available

### B) Complete Mapping Chain

#### Path 1: IR Planner (Preferred)

```
User Input: (wkd data)
    ↓
[Layer 1: Normalization]
    normalize.rs:40 normalize()
    → No macro expansion
    → Returns: (wkd data)
    ↓
[Layer 3: IR Path]
    main.rs:574 try_ir_eval()
    → main.rs:548 planner::plan()
        planner.rs:132 match "wkd"
        → planner.rs:132 plan_unary(NumericFunc::MSK_WKE, ...)
    → main.rs:551 exec::execute()
        exec.rs:126 execute_unary()
        → exec.rs:133 special case for MSK_WKE
        → **KERNEL:** wkd_mask_weekends() (exec.rs:134)
```

#### Path 2: Legacy Builtin (Fallback)

```
User Input: (w5 data) or (wkd data)
    ↓
[Layer 2: Legacy Eval]
    eval.rs:11 Runtime::eval()
    → eval.rs:36 eval_list()
    → eval.rs:82 is_builtin() check → TRUE
    → eval.rs:84-87 evaluate arguments
    → eval.rs:90 call_builtin()
        → **KERNEL:** builtin_wkd() (builtins.rs:963)
```

### C) File+Line Anchors

**IR Path:**

| Hop | File | Line | Description |
|-----|------|------|-------------|
| Plan Match | planner.rs | 132 | `"wkd"` case |
| IR Enum | planner.rs | 132 | `NumericFunc::MSK_WKE` |
| Execute | exec.rs | 126 | `execute_unary()` |
| Special Case | exec.rs | 133 | Check for MSK_WKE |
| Kernel | exec.rs | 134 | `wkd_mask_weekends()` |

**Legacy Path:**

| Hop | File | Line | Description |
|-----|------|------|-------------|
| Registration | builtins.rs | 186 | `register_builtin("wkd", builtin_wkd)` |
| Registration | builtins.rs | 187 | `register_builtin("w5", builtin_wkd)` (alias) |
| Builtin Check | eval.rs | 82 | `is_builtin()` |
| Builtin Call | eval.rs | 90 | `call_builtin()` |
| Kernel | builtins.rs | 963 | `fn builtin_wkd()` |

**IR Enum:** `Operation::Unary(UnaryOp::MapNumeric { func: NumericFunc::MSK_WKE })`
**Legacy Builtin:** `builtin_wkd` (both "w5" and "wkd" map to same function)
**Status:** Dual path - IR preferred (Frame input), legacy fallback (other types)

**Selection Condition:**
- Frame/TableView input → IR succeeds
- Scalar/other types → IR fails with "Cannot plan" → legacy fallback executes

---

## Example 3: `(dlog <expr>)`

### A) Dispatch Path
**IR Planner (Layer 3) - IR-ONLY**

### B) Complete Mapping Chain

#### Path 1: IR Planner (Only Path)

```
User Input: (dlog data)
    ↓
[Layer 1: Normalization]
    normalize.rs:40 normalize()
    → No macro expansion
    → Returns: (dlog data)
    ↓
[Layer 3: IR Path]
    main.rs:574 try_ir_eval()
    → main.rs:548 planner::plan()
        planner.rs:123 match "dlog"
        → planner.rs:123 plan_unary(NumericFunc::SHF_PTW_OBS_NLN_DLOG, ...)
        → Comment: "default: OBS (NA-skipping)"
    → main.rs:551 exec::execute()
        exec.rs:126 execute_unary()
        → exec.rs:155-174 non-rolling case
        → exec.rs:157 match NumericFunc::SHF_PTW_OBS_NLN_DLOG
        → **KERNEL:** dlog_obs_column(col, 1)
```

#### Path 2: Legacy Builtin (Does NOT Exist!)

```
User Input: (dlog data)
    ↓
[Layer 2: Legacy Eval]
    eval.rs:11 Runtime::eval()
    → eval.rs:36 eval_list()
    → eval.rs:82 is_builtin() check → FALSE (dlog not registered!)
    → eval.rs:94 resolve() attempts variable lookup
    → **ERROR:** "Undefined variable: dlog"
```

**NOTE:** `dlog` has **NO** legacy builtin registration!
Only `dlog-col` and `dlog-cols` exist in legacy for backward compatibility.

### C) File+Line Anchors

**IR Path:**

| Hop | File | Line | Description |
|-----|------|------|-------------|
| Plan Match | planner.rs | 123 | `"dlog"` case |
| IR Enum | planner.rs | 123 | `NumericFunc::SHF_PTW_OBS_NLN_DLOG` |
| Comment | planner.rs | 123 | "default: OBS (NA-skipping)" |
| Execute | exec.rs | 126 | `execute_unary()` |
| Match | exec.rs | 157 | `SHF_PTW_OBS_NLN_DLOG` case |
| Kernel | exec.rs | 157 | `dlog_obs_column(col, 1)` |

**Legacy Path (BROKEN):**

| Hop | File | Line | Description |
|-----|------|------|-------------|
| NOT FOUND | builtins.rs | - | **MISSING** - `dlog` not registered! |
| Alternative | builtins.rs | 133 | `builtin_dlog_col` (single-column version) |
| Alternative | builtins.rs | 162 | `builtin_dlog_cols` (multi-column version) |
| Kernel | builtins.rs | 2252 | `fn builtin_dlog()` (implementation) |

**IR Enum:** `Operation::Unary(UnaryOp::MapNumeric { func: NumericFunc::SHF_PTW_OBS_NLN_DLOG })`
**Legacy Builtin:** **MISSING** - `dlog` not registered (only `dlog-col` exists)
**Status:** IR-only operation (intentional design decision)

**Selection Condition:**
- Frame input → IR succeeds with **OBS semantics** (observation-based, NA-skipping lag)
- Scalar/other → IR fails with "Cannot plan" → legacy eval fails with "Undefined variable"

**Design Note:** The split between `dlog` (IR, OBS semantics) and `dlog-col` (legacy) reflects different NA-handling strategies:
- `dlog` (IR): Skips NAs when computing lag (business-day lag when masked)
- `dlog-col` (legacy): Positional lag (may use NA values)

---

## Summary Table: Operation Coverage

| Surface Token | Macro? | Legacy Builtin? | IR Planner? | Primary Path | Notes |
|---------------|--------|-----------------|-------------|--------------|-------|
| `file` | No | ❌ MISSING | ✅ Source::File | **IR-only** | New design |
| `stdin` | No | ✅ builtin_stdin | ✅ Source::Stdin | IR (preferred) | Dual path |
| `wkd` | No | ✅ builtin_wkd | ✅ MSK_WKE | IR (preferred) | Dual path |
| `w5` | No | ✅ builtin_wkd | ❌ | **Legacy-only** | Alias for wkd |
| `dlog` | No | ❌ MISSING | ✅ SHF_PTW_OBS_NLN_DLOG | **IR-only** | OBS semantics |
| `dlog-ofs` | No | ❌ MISSING | ✅ SHF_PTW_OFS_NLN_DLOG | **IR-only** | OFS semantics |
| `dlog-col` | No | ✅ builtin_dlog | ❌ | **Legacy-only** | Old API |
| `dlog-cols` | No | ✅ builtin_dlog_cols | ❌ | **Legacy-only** | Old API |
| `shift` | No | ✅ builtin_shift | ✅ SHF_PTW_LIN_SHF | IR (preferred) | Dual path |
| `shift-col` | No | ✅ builtin_shift | ❌ | **Legacy-only** | Old API |
| `shift-cols` | No | ✅ builtin_shift_cols | ❌ | **Legacy-only** | Old API |
| `->` | ✅ **YES** | No | No | **Macro** | normalize.rs |
| `defparameter` | No | N/A (special form) | ❌ | **Legacy-only** | eval.rs:72 |
| `let*` | No | N/A (special form) | ❌ | **Legacy-only** | eval.rs:71 |
| `if` | No | N/A (special form) | ❌ | **Legacy-only** | eval.rs:70 |

---

## IR Numeric Function Registry (ir.rs:196-300)

Complete list of IR operations with canonical names:

| IR Enum | Canonical Name | Surface Token | Category | Description |
|---------|----------------|---------------|----------|-------------|
| `SHF_PTW_OBS_NLN_DLOG` | Shift-Pointwise-Obs-Nonlinear-DLog | `dlog` | Temporal | Diff log (NA-skipping lag) |
| `SHF_PTW_OFS_NLN_DLOG` | Shift-Pointwise-Ofs-Nonlinear-DLog | `dlog-ofs` | Temporal | Diff log (positional lag) |
| `SHF_PTW_LIN_SHF` | Shift-Pointwise-Linear-Shift | `shift` | Temporal | Lag by k rows |
| `LAG_OBS` | Lag-Observation | `lag-obs`, `shift-obs` | Temporal | Mask-aware lag |
| `SHF_REC_NLN_LOCF` | Shift-Recursive-Nonlinear-LOCF | `locf` | Fill | Last obs carried forward |
| `MSK_WKE` | Mask-Weekend | `wkd` | Mask | Set weekends to NA |
| `SHF_PFX_LIN_SUM` | Shift-Prefix-Linear-Sum | `cs1` | Aggregate | Cumsum starting at 1.0 |
| `SHF_WIN_LIN_AVG` | Shift-Window-Linear-Avg | `rolling-mean` | Rolling | Trailing window mean |
| `SHF_WIN_NLN_SDV` | Shift-Window-Nonlinear-StdDev | `rolling-std` | Rolling | Trailing window std |
| `SHF_WIN_MIN2_LIN_AVG` | Shift-Window-Min2-Linear-Avg | `rolling-mean-min2` | Rolling | Mean (≥2 obs) |
| `SHF_WIN_MIN2_NLN_SDV` | Shift-Window-Min2-Nonlinear-StdDev | `rolling-std-min2` | Rolling | Std (≥2 obs) |
| `KEEP` | Keep | `keep` | Downsample | Keep every k-th row |
| `LOG` | Log | `log` | Math | Natural logarithm |
| `EXP` | Exp | `exp` | Math | Exponential |
| `SQRT` | Sqrt | `sqrt` | Math | Square root |
| `ABS` | Abs | `abs` | Math | Absolute value |
| `INV` | Inverse | `inv` | Math | 1/x |
| `RET` | Return | `ret` | Finance | x/x[-1] - 1 |

---

## Key Observations

1. **IR is winning:** New operations like `file`, `dlog`, `shift` go IR-first or IR-only
2. **Legacy compatibility:** Old operations like `w5`, `dlog-col`, `shift-col` remain in legacy
3. **Naming convention:**
   - Clean tokens (`dlog`, `shift`, `wkd`) → IR path
   - Suffixed tokens (`dlog-col`, `shift-cols`) → Legacy path
   - IR uses canonical names (e.g., `SHF_PTW_OBS_NLN_DLOG`)
4. **HYBRID mode** (default) seamlessly combines both paths
5. **Macro layer** is independent: `->` expands before either IR or legacy sees it
6. **Schema validation:** IR path enforces contracts.md at plan time (before execution)
7. **Fusion ready:** IR can fuse operations (e.g., `abs → log → cs1` → single pass)

---

## Evolution Pattern

**Old Design (Legacy):**
```
User → eval.rs → builtins.rs → kernel
```

**New Design (IR):**
```
User → normalize.rs → planner.rs → ir.rs → exec.rs → kernel
      (macro expand)  (validate)   (DAG)    (execute)
```

**Current (Hybrid):**
```
User → normalize.rs → try_ir_eval() ✅ → result
                           ↓ ❌
                      eval.rs → builtins.rs → result
```

---

## Testing the Dispatch

### Check which path is taken:

```bash
# IR path (Frame operation)
echo "date;px" > test.csv
echo "2024-01-01;100" >> test.csv
blisp -e '(dlog (file "test.csv"))'
# → Uses IR: normalize → plan (SHF_PTW_OBS_NLN_DLOG) → exec

# Legacy path (defparameter)
blisp -e '(defparameter x 10)'
# → Uses legacy: eval.rs special form

# Macro expansion
blisp -e '(-> (file "test.csv") (dlog))'
# → normalize.rs expands to: (dlog (file "test.csv"))
# → Then IR path
```

### Force specific path:

```bash
# Force IR-only (experimental)
BLISP_IR_ONLY=1 blisp -e '(dlog (file "test.csv"))'

# Force legacy-only
BLISP_LEGACY=1 blisp -e '(+ 1 2)'
```

---

## Future Work

1. **More IR operations:** Gradually move legacy builtins to IR
2. **Fusion optimization:** Combine multiple IR ops into single pass
3. **Type inference:** Propagate schema info through IR for compile-time checks
4. **Code generation:** JIT compile IR plans for further speedup
5. **Macro expansion caching:** Avoid re-expanding same forms

---

## References

- **contracts.md** - Schema invariants (I1, I2, I3) enforced by IR
- **ir_equivalence.rs** - 116 tests proving IR ≡ legacy semantics
- **BLADE phases** - Phase 2 (primitives) → Phase 3 (IR compiler)
- **GLD_NUM compatibility** - IR operations match CLISPI/blawk behavior

---

**End of Document**
