# BLISP Dispatch Map - Authoritative Reference

**Version:** 1.2
**Date:** 2026-02-27 (Updated post-comparison operators extension)
**Repository:** /home/ubuntu/blisp
**Branch:** reconstruct/tableview-only
**Status:** Ground Truth - Single Source of Authority
**Last Changes:**
- Level 1 Migration: Added 5 deprecated aliases (dlog-col, shift-col, cs1-col, ur-col, w5) to IR planner (commits 2141d00, b3849ed)
- Canonical Extension: Added 5 comparison operators (<, <=, >=, ==, !=) to IR system (commit 56a8d18)

---

## Executive Summary

This document is the **single authoritative reference** for BLISP's dispatch system. Every user form follows exactly one path through the system, determined by mode flags and token recognition. This map explains where every token routes, which paths win when both exist, and how to fix broken routes.

**Key Principle:** In HYBRID mode (default), IR ALWAYS tries first and shadows builtins.

---

## Part A: Dispatch Decision Tree

```
┌─────────────────────────────────────────────────────────────────┐
│                    USER INPUT: Expression                        │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                    ┌───────▼────────┐
                    │  main.rs:556   │
                    │  eval_code()   │
                    └───────┬────────┘
                            │
              ┌─────────────┼─────────────┐
              │             │             │
    ┌─────────▼──────┐  ┌──▼───────┐  ┌─▼──────────┐
    │  use_legacy?   │  │use_ir    │  │ HYBRID     │
    │  (forced)      │  │_only?    │  │ (DEFAULT)  │
    └────────┬───────┘  └──┬───────┘  └─┬──────────┘
             │             │             │
             │             │             │
    ┌────────▼────────┐   │    ┌────────▼─────────┐
    │  LEGACY PATH    │   │    │ Try IR First     │
    │  eval.rs:11     │   │    │ main.rs:574      │
    │  rt.eval()      │   │    │ try_ir_eval()    │
    └─────────────────┘   │    └────────┬─────────┘
                          │             │
                          │    ┌────────▼─────────┐
                          │    │ normalize.rs:40  │
                          │    │ (macro expand)   │
                          │    └────────┬─────────┘
                          │             │
                          │    ┌────────▼─────────┐
                          │    │ planner.rs:42    │
                          │    │ plan()           │
                          │    └────────┬─────────┘
                          │             │
                   ┌──────▼──────┐      │
                   │  IR PATH    │      │
                   │  exec.rs:48 ├──────┘
                   │  execute()  │   Success
                   └─────────────┘      │
                                   ┌────▼────┐
                          Failure  │ Result  │
                            ↓      └─────────┘
              ┌─────────────┴─────────────┐
              │ Error contains:           │
              │ "Cannot plan" OR          │
              │ "not supported" OR        │
              │ "Unknown function"?       │
              └──────────┬────────────────┘
                         │
                    YES  │  NO
                ┌────────┼────────┐
                │                 │
    ┌───────────▼────────┐   ┌────▼─────────┐
    │ LEGACY FALLBACK    │   │ PROPAGATE    │
    │ main.rs:587        │   │ ERROR        │
    │ rt.eval()          │   │ main.rs:591  │
    └────────────────────┘   └──────────────┘
```

**Precedence Rules (HYBRID mode):**

1. **Normalization layer** (main.rs:545): Macro expansion happens BEFORE dispatch
2. **IR planner** (main.rs:548): Tries first, shadows builtins
3. **Fallback trigger** (main.rs:584): Specific error strings ONLY
4. **Legacy evaluator** (main.rs:587): Universal fallback for unknown tokens
5. **Error propagation** (main.rs:591): Real IR errors bypass fallback

**Mode Selection Priority:**

```
use_legacy  (BLISP_LEGACY=1 or --legacy)
    ↓ if not set
use_ir_only (BLISP_IR_ONLY=1 or --ir-only)
    ↓ if not set
HYBRID (default)
```

**Code locations:**
- Mode flags: main.rs:39-40
- Mode check: main.rs:565-594
- Fallback condition: main.rs:584 (exact string match)

---

## Part B: The Three Layers

### Layer 1: Normalization / Macro Expansion

**Entry:** `normalize.rs:40` - `fn normalize(expr: Expr, interner: &mut Interner) -> CanonExpr`

**Purpose:** Transform surface syntax to canonical form

**Transformations:**
1. Thread-first macro: `(-> x (f a) (g b))` → `(g (f x a) b)`
   - Check: normalize.rs:53-57 `if name == "->"`
   - Expand: normalize.rs:85 `normalize_thread_first()`

**Recognized macros (1 total):**
- `->` (thread-first)

**Note:** Normalization happens ONCE at entry (main.rs:545), BEFORE IR/legacy decision.

**Idempotent:** `normalize(normalize(x)) == normalize(x)`

---

### Layer 2: Legacy Evaluator / Builtins

**Entry:** `eval.rs:11` - `fn eval(&mut self, expr: &Expr) -> Result<Value, String>`

**Dispatch order:**

1. **Literals** (eval.rs:13-18): Return as-is
2. **Symbols** (eval.rs:21): Variable lookup
3. **Quote** (eval.rs:24): Return unevaluated
4. **Lists** (eval.rs:36): `eval_list()`
   - **Macro check** (eval.rs:55-59): Expand if macro
   - **Special forms** (eval.rs:66-79): Direct handling
   - **Builtin check** (eval.rs:82): `is_builtin()`
   - **Builtin call** (eval.rs:90): `call_builtin()`
   - **Variable lookup** (eval.rs:94): Final fallback

**Special forms (never go to IR):**
```
quote, progn, if, let*, defparameter, setf, define, lambda, defmacro, ->
```

**Builtin registrations:** builtins.rs:119-222 (71 total)

**Builtin call:** eval.rs:90 → Runtime::call_builtin() → builtins.rs function

---

### Layer 3: IR Planner / Executor

**Entry:** `main.rs:543` - `fn try_ir_eval(rt: &mut Runtime, expr: ast::Expr) -> Result<Value, String>`

**Pipeline:**

1. **Normalize** (main.rs:545): `normalize::normalize(expr, &mut rt.interner)`
2. **Plan** (main.rs:548): `planner::plan(&normalized, &rt.interner)?`
   - Match: planner.rs:86 `match func_name`
   - Build IR: Create `Node { op: Operation::..., schema: ... }`
   - Validate: `plan.validate()` (planner.rs:49)
3. **Execute** (main.rs:551): `exec::execute(&plan, rt)?`
   - Dispatch: exec.rs:70 `match node.op`
   - Kernels: exec.rs:84, 134, 157, etc.

**Recognized tokens:** planner.rs:86-641 (36 total)

**IR Enums:**
- `Operation::Source` → exec.rs:71 → execute_source()
- `Operation::Unary` → exec.rs:72 → execute_unary()
- `Operation::Binary` → exec.rs:73 → execute_binary()
- `Operation::Join` → exec.rs:74 → execute_join()
- `Operation::Schema` → exec.rs:75 → execute_schema()

**Error that triggers fallback:** planner.rs:640, 647

---

## Part C: Master Dispatch Table

| Surface Token | Macro Expansion | IR Planner Mapping | IR Enum | Exec Kernel | Legacy Builtin | Builtin Function | Status | Notes |
|---------------|-----------------|-------------------|---------|-------------|----------------|------------------|--------|-------|
| `->` | YES (normalize.rs:85) | N/A | N/A | N/A | N/A | N/A | **Macro-only** | Thread-first expansion |
| `file` | No | planner.rs:88 | Source::File | io::load_csv (exec.rs:84) | builtins.rs:146 | builtin_file | **Dual** | IR wins |
| `stdin` | No | planner.rs:108 | Source::Stdin | io::parse_csv_to_frame (exec.rs:107) | builtins.rs:148 | builtin_stdin | **Dual** | IR wins |
| `dlog` | No | planner.rs:123 | NumericFunc::SHF_PTW_OBS_NLN_DLOG | dlog_obs_column (exec.rs:157) | **MISSING** | **MISSING** | **IR-only** | ⚠️ No fallback |
| `dlog-ofs` | No | planner.rs:124 | NumericFunc::SHF_PTW_OFS_NLN_DLOG | dlog_ofs_column (exec.rs:158) | **MISSING** | **MISSING** | **IR-only** | Positional lag |
| `dlog-col` | No | planner.rs:127 | NumericFunc::SHF_PTW_OBS_NLN_DLOG | dlog_obs_column (exec.rs:157) | builtins.rs:133 | builtin_dlog | **Dual (DEPRECATED)** | ✅ Alias for dlog |
| `dlog-cols` | No | **MISSING** | **MISSING** | **MISSING** | builtins.rs:162 | builtin_dlog_cols | **Legacy-only** | Multi-column |
| `ret` | No | planner.rs:125 | NumericFunc::RET | ret_column (exec.rs:159) | **MISSING** | **MISSING** | **IR-only** | x/x[-1] - 1 |
| `log` | No | planner.rs:126 | NumericFunc::LOG | log_column (exec.rs:160) | **MISSING** | **MISSING** | **IR-only** | Natural log |
| `exp` | No | planner.rs:127 | NumericFunc::EXP | exp_column (exec.rs:161) | **MISSING** | **MISSING** | **IR-only** | Exponential |
| `sqrt` | No | planner.rs:128 | NumericFunc::SQRT | sqrt_column (exec.rs:162) | **MISSING** | **MISSING** | **IR-only** | Square root |
| `abs` | No | planner.rs:129 | NumericFunc::ABS | abs_column (exec.rs:163) | **MISSING** | **MISSING** | **IR-only** | Absolute value |
| `inv` | No | planner.rs:130 | NumericFunc::INV | inv_column (exec.rs:164) | **MISSING** | **MISSING** | **IR-only** | 1/x |
| `locf` | No | planner.rs:131 | NumericFunc::SHF_REC_NLN_LOCF | locf_column (exec.rs:165) | **MISSING** | **MISSING** | **IR-only** | ⚠️ No fallback |
| `locf-cols` | No | **MISSING** | **MISSING** | **MISSING** | builtins.rs:176 | builtin_locf_cols | **Legacy-only** | Multi-column |
| `wkd` | No | planner.rs:132 | NumericFunc::MSK_WKE | wkd_mask_weekends (exec.rs:134) | builtins.rs:186 | builtin_wkd | **Dual** | IR wins |
| `w5` | No | planner.rs:142 | NumericFunc::MSK_WKE | wkd_mask_weekends (exec.rs:134) | builtins.rs:187 | builtin_wkd | **Dual (DEPRECATED)** | ✅ Alias for wkd |
| `cs1` | No | planner.rs:133 | NumericFunc::SHF_PFX_LIN_SUM | cumsum_column (exec.rs:166) | **MISSING** | **MISSING** | **IR-only** | ⚠️ No fallback |
| `cs1-col` | No | planner.rs:143 | NumericFunc::SHF_PFX_LIN_SUM | cumsum_column (exec.rs:166) | builtins.rs:190 | builtin_cs1 | **Dual (DEPRECATED)** | ✅ Alias for cs1 |
| `cs1-cols` | No | **MISSING** | **MISSING** | **MISSING** | builtins.rs:189 | builtin_cs1_cols | **Legacy-only** | Multi-column |
| `shift` | No | planner.rs:136 | NumericFunc::SHF_PTW_LIN_SHF{k} | shift_column (exec.rs:167) | **MISSING** | **MISSING** | **IR-only** | ⚠️ No fallback |
| `shift-col` | No | planner.rs:166 | NumericFunc::SHF_PTW_LIN_SHF{k} | shift_column (exec.rs:167) | builtins.rs:134 | builtin_shift | **Dual (DEPRECATED)** | ✅ Alias for shift |
| `shift-cols` | No | **MISSING** | **MISSING** | **MISSING** | builtins.rs:163 | builtin_shift_cols | **Legacy-only** | Multi-column |
| `lag-obs` | No | planner.rs:154 | NumericFunc::LAG_OBS{k} | apply_shift_obs_mask_aware (exec.rs:148) | **MISSING** | **MISSING** | **IR-only** | Mask-aware lag |
| `shift-obs` | No | planner.rs:154 | NumericFunc::LAG_OBS{k} | apply_shift_obs_mask_aware (exec.rs:148) | **MISSING** | **MISSING** | **IR-only** | Alias for lag-obs |
| `keep` | No | planner.rs:171 | NumericFunc::KEEP{k} | keep_column (exec.rs:168) | **MISSING** | **MISSING** | **IR-only** | Downsample |
| `rolling-mean` | No | planner.rs:188 | NumericFunc::SHF_WIN_LIN_AVG{w} | apply_rolling_mask_aware (exec.rs:145) | **MISSING** | **MISSING** | **IR-only** | Trailing window |
| `rolling-std` | No | planner.rs:256 | NumericFunc::SHF_WIN_NLN_SDV{w} | apply_rolling_mask_aware (exec.rs:145) | **MISSING** | **MISSING** | **IR-only** | Trailing window |
| `ur` | No | planner.rs:408 | NumericFunc::SHF_WIN_NLN_UR{w} | apply_rolling_mask_aware (exec.rs:145) | **MISSING** | **MISSING** | **IR-only** | ⚠️ No fallback |
| `ur-col` | No | planner.rs:498 | NumericFunc::SHF_WIN_NLN_UR{w} | apply_rolling_mask_aware (exec.rs:145) | builtins.rs:198 | builtin_ur | **Dual (DEPRECATED)** | ✅ Alias for ur |
| `ur-cols` | No | **MISSING** | **MISSING** | **MISSING** | builtins.rs:197 | builtin_ur_cols | **Legacy-only** | Multi-column |
| `+` | No | planner.rs:520 | BinaryFunc::ADD | add_scalar/add_columns (exec.rs:350) | builtins.rs:121 | builtin_add | **Dual** | IR wins |
| `-` | No | planner.rs:521 | BinaryFunc::SUB | sub_scalar/sub_columns (exec.rs:350) | builtins.rs:122 | builtin_sub | **Dual** | IR wins |
| `*` | No | planner.rs:522 | BinaryFunc::MUL | mul_scalar/mul_columns (exec.rs:350) | builtins.rs:123 | builtin_mul | **Dual** | IR wins |
| `/` | No | planner.rs:523 | BinaryFunc::DIV | div_scalar/div_columns (exec.rs:350) | builtins.rs:124 | builtin_div | **Dual** | IR wins |
| `>` | No | planner.rs:524 | BinaryFunc::GTR | gt_columns (exec.rs:350) | builtins.rs:125 | builtin_gt | **Dual** | IR wins |
| `<` | No | planner.rs:618 | BinaryFunc::LSS | binary_scalar_column/binary_column_column (exec.rs:2174) | builtins.rs:169 | builtin_lt | **Dual** | IR wins |
| `>=` | No | planner.rs:620 | BinaryFunc::GTE | binary_scalar_column/binary_column_column (exec.rs:2174) | builtins.rs:170 | builtin_gte | **Dual** | IR wins |
| `<=` | No | planner.rs:619 | BinaryFunc::LTE | binary_scalar_column/binary_column_column (exec.rs:2174) | builtins.rs:171 | builtin_lte | **Dual** | IR wins |
| `==` | No | planner.rs:621 | BinaryFunc::EQL | binary_scalar_column/binary_column_column (exec.rs:2174) | builtins.rs:172 | builtin_eq | **Dual** | IR wins |
| `!=` | No | planner.rs:622 | BinaryFunc::NEQ | binary_scalar_column/binary_column_column (exec.rs:2174) | builtins.rs:173 | builtin_neq | **Dual** | IR wins |
| `mapr` | No | planner.rs:527 | JoinOp::ALIGN | reindex_by (exec.rs:420) | builtins.rs:196 | builtin_mapr | **Dual** | IR wins |
| `asofr` | No | planner.rs:528 | JoinOp::ASOF_ALIGN | asofr (exec.rs:446) | **MISSING** | **MISSING** | **IR-only** | ASOF join |
| `xminus` | No | planner.rs:531 | SchemaOp::SHF_PTW_LIN_SPR{half} | xminus_columns (exec.rs:507) | builtins.rs:188 | builtin_xminus | **Dual** | IR wins |
| `mask-weekend` | No | planner.rs:558 | SchemaOp::MSK_WKE_DEF{name} | mask_weekend_define (exec.rs:550) | **MISSING** | **MISSING** | **IR-only** | Define mask |
| `with-mask` | No | planner.rs:587 | SchemaOp::WTH_MSK{mask_expr} | with_mask_apply (exec.rs:616) | builtins.rs:181 | builtin_with_mask | **Dual** | IR wins |
| `mask-on` | No | **MISSING** | **MISSING** | **MISSING** | builtins.rs:181 | builtin_with_mask | **Legacy-only** | Alias for with-mask |
| `print` | No | **MISSING** | **MISSING** | **MISSING** | builtins.rs:216 | builtin_print | **Legacy-only** | I/O side effect |
| `save` | No | **MISSING** | **MISSING** | **MISSING** | builtins.rs:149 | builtin_save | **Legacy-only** | I/O side effect |
| `file-head` | No | **MISSING** | **MISSING** | **MISSING** | builtins.rs:147 | builtin_file_head | **Legacy-only** | Partial read |
| `type-of` | No | **MISSING** | **MISSING** | **MISSING** | builtins.rs:217 | builtin_type_of | **Legacy-only** | Introspection |
| `len` | No | **MISSING** | **MISSING** | **MISSING** | builtins.rs:218 | builtin_len | **Legacy-only** | Length |

**Status Legend:**
- **IR-only:** Recognized by planner, NO builtin (fails on fallback)
- **Legacy-only:** Registered builtin, NOT recognized by planner (bypasses IR)
- **Dual:** Both paths exist, IR wins in HYBRID mode
- **Macro-only:** Handled by normalization layer only

**Critical Mismatches:**
- **7 IR-only operations with NO fallback:** dlog, locf, cs1, shift, ur, ret, log
- **0 Legacy-only aliases that break IR trees:** ✅ (w5 fixed in commit b3849ed)
- **5 Deprecated dual-routing aliases (Level 1 migration):** dlog-col, shift-col, cs1-col, ur-col, w5 ✅
- **15 Dual-routing tokens where builtin is shadowed:** +, -, *, /, >, <, <=, >=, ==, !=, mapr, stdin, wkd, xminus, file

---

## Part D: Gotchas - Ways Expressions Can Double-Fail

### Gotcha 1: IR-Only Outer + Legacy-Only Inner (FIXED)

**Problem:** Nesting IR-only operation with legacy-only alias

**Status:** ✅ RESOLVED - All Level 1 migrations complete (commits 2141d00, b3849ed)

**Example 1 (FIXED in commit b3849ed):**
```lisp
(dlog (w5 20 PRC))
```

**Before fix - Failed:**
1. IR tries to plan outer `dlog` → Success (planner.rs:123)
2. IR recurses to plan inner `(w5 20 PRC)`
3. IR tries to match `w5` → **FAIL** (planner.rs:640 "Unknown function: w5")
4. main.rs:584 catches "Unknown function" → Fallback to legacy
5. Legacy tries to eval outer `dlog` → **FAIL** (eval.rs:94 "Undefined variable: dlog")
6. **RESULT:** Both paths fail!

**After fix - Works:**
1. IR tries to plan outer `dlog` → Success
2. IR recurses to plan inner `(w5 20 PRC)`
3. IR matches `w5` → Success (planner.rs:142, emits deprecation warning)
4. Expression evaluates through IR path
5. **RESULT:** ✅ SUCCESS (with warning: "w5 is deprecated, use wkd instead")

**Recommended:** Use canonical naming: `(dlog (wkd 20 PRC))` to avoid warnings

**Example 2 (FIXED in commit b3849ed):**
```lisp
(ur 250 1 (w5 RET))
```

**Before fix - Failed:**
1. IR tries to plan outer `ur` → Success (planner.rs:408)
2. IR recurses to plan inner `(w5 RET)`
3. IR tries to match `w5` → **FAIL** "Unknown function: w5"
4. Fallback to legacy
5. Legacy tries to eval outer `ur` → **FAIL** "Undefined variable: ur"
6. **RESULT:** Both paths fail!

**After fix - Works:**
1. IR tries to plan outer `ur` → Success
2. IR recurses to plan inner `(w5 RET)`
3. IR matches `w5` → Success (planner.rs:142, emits deprecation warning)
4. Expression evaluates through IR path
5. **RESULT:** ✅ SUCCESS (with warning: "w5 is deprecated, use wkd instead")

**Recommended:** Use canonical naming: `(ur 250 1 (wkd RET))` to avoid warnings

**Example 3 (FIXED in Level 1 migration commit 2141d00):**
```lisp
(shift 1 (dlog-col PRC))
```

**Before Level 1 migration - Failed:**
1. IR tries to plan outer `shift` → Success (planner.rs:136)
2. IR recurses to plan inner `(dlog-col PRC)`
3. IR tries to match `dlog-col` → **FAIL** "Unknown function: dlog-col"
4. Fallback to legacy
5. Legacy tries to eval outer `shift` → **FAIL** "Undefined variable: shift"
6. **RESULT:** Both paths fail!

**After Level 1 migration (commit 2141d00) - Works:**
1. IR tries to plan outer `shift` → Success
2. IR recurses to plan inner `(dlog-col PRC)`
3. IR matches `dlog-col` → Success (planner.rs:127, emits deprecation warning)
4. Expression evaluates through IR path
5. **RESULT:** ✅ SUCCESS (with warning: "dlog-col is deprecated, use dlog instead")

**Recommended:** Use canonical naming: `(shift 1 (dlog PRC))` to avoid warnings

### Gotcha 2: Scalar Input to IR-Only Operation

**Problem:** IR operations require Frame input, no legacy fallback exists

**Example:**
```lisp
(dlog 42)
```

**Failure sequence:**
1. IR tries to plan `(dlog 42)`
2. IR matches `dlog` → Success (planner.rs:123)
3. exec.rs attempts to execute on scalar `42`
4. **FAIL:** "Expected Frame, got Int" (runtime type error)
5. main.rs:591 propagates error (NOT "Unknown function")
6. **NO FALLBACK** - error propagates to user
7. **RESULT:** IR fails, no legacy fallback tried

**Fix:** Use Frame input: `(dlog (file "data.csv"))`

**Workaround:** Force legacy mode: `BLISP_LEGACY=1 blisp -e '(dlog-col 42)'`

### Gotcha 3: Legacy-Only Operation in IR Tree (FIXED)

**Problem:** Legacy operation nested inside IR-recognized tree

**Status:** ✅ RESOLVED - w5 alias fixed in commit b3849ed

**Example (FIXED):**
```lisp
(mapr LHS (w5 RHS))
```

**Before fix - Bypassed IR path:**
1. IR tries to plan outer `mapr` → Success (planner.rs:527)
2. IR recurses to plan second arg `(w5 RHS)`
3. IR tries to match `w5` → **FAIL** "Unknown function: w5"
4. Fallback to legacy
5. Legacy tries to eval entire tree starting with outer `mapr`
6. Legacy CAN handle `mapr` (builtins.rs:196)
7. **SUCCESS** via legacy path (but slower, no schema validation)

**After fix - Works in IR:**
1. IR tries to plan outer `mapr` → Success (planner.rs:527)
2. IR recurses to plan second arg `(w5 RHS)`
3. IR matches `w5` → Success (planner.rs:142, emits deprecation warning)
4. Expression evaluates through IR path with full schema validation
5. **RESULT:** ✅ SUCCESS through IR (faster, type-safe)

**Recommended:** Use canonical naming: `(mapr LHS (wkd RHS))` to avoid warnings

**Fix:** Use IR name: `(mapr LHS (wkd RHS))`

### Gotcha 4: Macro Expansion Breaks Token Recognition

**Problem:** Macro expands to IR-unrecognized form

**Example:**
```lisp
;; User defines custom macro:
(defmacro my-dlog (x) `(dlog-col ,x))

;; Then uses it:
(my-dlog PRC)
```

**Failure sequence:**
1. Normalize layer expands: `(my-dlog PRC)` → `(dlog-col PRC)`
2. IR tries to plan `(dlog-col PRC)`
3. IR tries to match `dlog-col` → **FAIL** "Unknown function: dlog-col"
4. Fallback to legacy
5. Legacy CAN handle `dlog-col` (builtins.rs:133)
6. **SUCCESS** via legacy (but bypasses IR optimization)

**Issue:** Macro expanded to legacy-only name, forcing suboptimal path

**Fix:** Macro should expand to IR name: `(defmacro my-dlog (x) \`(dlog ,x))`

### Gotcha 5: Special Form Bypasses IR Completely

**Problem:** Special forms never reach IR planner

**Example:**
```lisp
(defparameter RET (dlog (file "data.csv")))
```

**Flow:**
1. HYBRID mode tries IR first
2. IR tries to plan `(defparameter RET ...)`
3. IR tries to match `defparameter` → **FAIL** "Unknown function: defparameter"
4. Fallback to legacy
5. Legacy recognizes `defparameter` as special form (eval.rs:72)
6. Legacy evaluates args and defines variable
7. **SUCCESS** via legacy

**Note:** This is EXPECTED behavior. Special forms are ALWAYS legacy.

### Gotcha 6: Mode Override Silently Disables Fallback

**Problem:** IR-ONLY mode rejects legacy operations without fallback

**Example:**
```bash
BLISP_IR_ONLY=1 blisp -e '(print "hello")'
```

**Failure sequence:**
1. IR-ONLY mode forces IR path (main.rs:570)
2. IR tries to plan `(print "hello")`
3. IR tries to match `print` → **FAIL** "Unknown function: print"
4. **NO FALLBACK** in IR-ONLY mode
5. Error propagated directly to user
6. **RESULT:** "Unknown function: print"

**Fix:** Use HYBRID mode (default) or LEGACY mode for side effects

---

## Part E: Completeness Audit

### IR Planner Coverage

**Total recognized:** 42 tokens (planner.rs:86-641)

**Verified reachable in HYBRID mode:**
- ✅ All 42 tokens route to IR when input is Frame/TableView
- ✅ Fallback to legacy only on "Unknown function" error

**Recent additions:**
- w5 (commit b3849ed) - deprecated alias for wkd
- <, <=, >=, ==, != (commit 56a8d18) - canonical comparison operators

**Unreachable builtins (shadowed by IR):**
- builtin_add (builtins.rs:121) - shadowed by planner.rs:520
- builtin_sub (builtins.rs:122) - shadowed by planner.rs:521
- builtin_mul (builtins.rs:123) - shadowed by planner.rs:522
- builtin_div (builtins.rs:124) - shadowed by planner.rs:523
- builtin_gt (builtins.rs:125) - shadowed by planner.rs:524
- builtin_lt (builtins.rs:169) - shadowed by planner.rs:618 (added commit 56a8d18)
- builtin_gte (builtins.rs:170) - shadowed by planner.rs:620 (added commit 56a8d18)
- builtin_lte (builtins.rs:171) - shadowed by planner.rs:619 (added commit 56a8d18)
- builtin_eq (builtins.rs:172) - shadowed by planner.rs:621 (added commit 56a8d18)
- builtin_neq (builtins.rs:173) - shadowed by planner.rs:622 (added commit 56a8d18)
- builtin_mapr (builtins.rs:196) - shadowed by planner.rs:527
- builtin_wkd (builtins.rs:186) - shadowed by planner.rs:132
- builtin_xminus (builtins.rs:188) - shadowed by planner.rs:531
- builtin_with_mask (builtins.rs:181) - shadowed by planner.rs:587

**Note:** These builtins are UNREACHABLE in HYBRID mode for Frame inputs.

### Legacy Builtin Coverage

**Total registered:** 71 builtins (builtins.rs:119-222)

**Reachable in HYBRID mode:**
- ✅ 62 legacy-only builtins (IR doesn't recognize)
- ❌ 9 dual-routing builtins (unreachable, IR wins)

**Dangerous Aliases (break IR trees):**
- ~~`w5` (builtins.rs:187)~~ - ✅ FIXED in Level 1 migration (planner.rs:142, commit b3849ed)
- ~~`dlog-col` (builtins.rs:133)~~ - ✅ FIXED in Level 1 migration (planner.rs:127, commit 2141d00)
- ~~`shift-col` (builtins.rs:134)~~ - ✅ FIXED in Level 1 migration (planner.rs:166, commit 2141d00)
- ~~`ur-col` (builtins.rs:198)~~ - ✅ FIXED in Level 1 migration (planner.rs:498, commit 2141d00)
- ~~`cs1-col` (builtins.rs:190)~~ - ✅ FIXED in Level 1 migration (planner.rs:143, commit 2141d00)

**Missing IR mappings (high value):**
- ~~`<`, `>=`, `<=`, `==`, `!=`~~ - ✅ FIXED: Comparison operators added to IR (commit 56a8d18)

### Mismatches Summary

| Category | Count | Examples |
|----------|-------|----------|
| IR-only (no builtin) | 24 | dlog, shift, locf, cs1, ur, ret, log, exp, sqrt, abs |
| Legacy-only (no IR) | 52 | print, save, type-of, len, wstd, diff, dlog-cols, locf-cols |
| Dual-routing (IR shadows builtin) | 20 | +, -, *, /, >, <, <=, >=, ==, !=, mapr, stdin, wkd, xminus, file, **w5, dlog-col, shift-col, cs1-col, ur-col** |
| Dangerous aliases (unfixed) | 0 | ✅ All fixed |
| Deprecated aliases (Level 1) | 5 | w5 ✅, dlog-col ✅, shift-col ✅, cs1-col ✅, ur-col ✅ |

**BROKEN ROUTES:**
- Nesting IR-only with legacy-only alias → double-fail
- IR-only with scalar input → no fallback
- Legacy-only alias breaks IR tree optimization

---

## Part F: Fix Plan

### Fix 1: Add IR Aliases for Legacy Names

**Goal:** Zero-breakage migration for legacy-only aliases

**Code edits:**

```rust
// planner.rs:132 (add after "wkd")
"wkd" => plan_unary(NumericFunc::MSK_WKE, &elements[1..], plan, ctx, interner),
"w5" => {
    eprintln!("Warning: 'w5' is deprecated, use 'wkd' instead");
    plan_unary(NumericFunc::MSK_WKE, &elements[1..], plan, ctx, interner)
},

// planner.rs:123 (add after "dlog")
"dlog" => plan_unary(NumericFunc::SHF_PTW_OBS_NLN_DLOG, &elements[1..], plan, ctx, interner),
"dlog-col" => {
    eprintln!("Warning: 'dlog-col' is deprecated, use 'dlog' instead");
    plan_unary(NumericFunc::SHF_PTW_OBS_NLN_DLOG, &elements[1..], plan, ctx, interner)
},

// planner.rs:136 (add after "shift")
"shift" => { /* existing */ },
"shift-col" => {
    eprintln!("Warning: 'shift-col' is deprecated, use 'shift' instead");
    // Extract k and delegate to shift logic
    plan_unary(NumericFunc::SHF_PTW_LIN_SHF { k }, &elements[1..], plan, ctx, interner)
},
```

**Verification command:**
```bash
cd /home/ubuntu/blisp && rg '"w5"|"dlog-col"|"shift-col"' src/planner.rs
# Should show new match arms
```

**Impact:** ✅ Zero breakage - `(dlog (w5 20 PRC))` now works

### Fix 2: Add Missing Comparison Operators to IR

**Goal:** Complete arithmetic/comparison coverage in IR

**Code edits:**

```rust
// planner.rs:524 (add after ">")
">" => plan_binary(BinaryFunc::GTR, &elements[1..], plan, ctx, interner),
"<" => plan_binary(BinaryFunc::LSS, &elements[1..], plan, ctx, interner),
">=" => plan_binary(BinaryFunc::GTE, &elements[1..], plan, ctx, interner),
"<=" => plan_binary(BinaryFunc::LTE, &elements[1..], plan, ctx, interner),
"==" => plan_binary(BinaryFunc::EQL, &elements[1..], plan, ctx, interner),
"!=" => plan_binary(BinaryFunc::NEQ, &elements[1..], plan, ctx, interner),
```

**Also add to ir.rs enum:**
```rust
// ir.rs BinaryFunc enum
pub enum BinaryFunc {
    ADD, SUB, MUL, DIV,
    GTR, LSS, GTE, LTE, EQL, NEQ,  // Add these
}
```

**Verification command:**
```bash
cd /home/ubuntu/blisp && rg 'enum BinaryFunc' src/ir.rs -A 5
# Should show 6 comparison operators
```

**Impact:** IR can handle all comparison operations

### Fix 3: Remove Unreachable Dual-Routing Builtins

**Goal:** Eliminate dead code, enforce single source of truth

**Code edits:**

```rust
// builtins.rs:121-125 - REMOVE these registrations:
// rt.register_builtin("+", builtin_add);      // Shadowed by planner.rs:520
// rt.register_builtin("-", builtin_sub);      // Shadowed by planner.rs:521
// rt.register_builtin("*", builtin_mul);      // Shadowed by planner.rs:522
// rt.register_builtin("/", builtin_div);      // Shadowed by planner.rs:523
// rt.register_builtin(">", builtin_gt);       // Shadowed by planner.rs:524

// builtins.rs:186 - REMOVE:
// rt.register_builtin("wkd", builtin_wkd);    // Shadowed by planner.rs:132

// builtins.rs:188 - REMOVE:
// rt.register_builtin("xminus", builtin_xminus);  // Shadowed by planner.rs:531

// builtins.rs:196 - REMOVE:
// rt.register_builtin("mapr", builtin_mapr);  // Shadowed by planner.rs:527
```

**Keep:**
```rust
// builtins.rs:148 - KEEP (file I/O has value in both paths):
rt.register_builtin("stdin", builtin_stdin);
```

**Verification command:**
```bash
cd /home/ubuntu/blisp && rg 'register_builtin\("(\+|\-|\*|\/|>|wkd|xminus|mapr)"' src/builtins.rs
# Should return empty after removal
```

**Impact:**
- ✅ Eliminates 8 unreachable builtins
- ⚠️ Scalar arithmetic fails: `(+ 1 2)` → "Undefined variable: +"
- 🔧 Workaround: `BLISP_LEGACY=1` for scalars

### Fix 4: Improve Error Messages

**Goal:** Guide users to correct token names

**Code edits:**

```rust
// planner.rs:640 - ENHANCE error message:
_ => {
    // Check if it's a known legacy alias
    let suggestion = match func_name {
        "w5" => Some("use 'wkd' instead"),
        "dlog-col" => Some("use 'dlog' instead"),
        "shift-col" => Some("use 'shift' instead"),
        "ur-col" => Some("use 'ur' instead"),
        "cs1-col" => Some("use 'cs1' instead"),
        _ => None,
    };

    if let Some(hint) = suggestion {
        Err(format!("Unknown function: {} (deprecated alias, {})", func_name, hint))
    } else {
        Err(format!("Unknown function: {}", func_name))
    }
}

// eval.rs:94 - ENHANCE error message:
let err_msg = format!("Undefined variable: {}", self.interner.resolve(*head_sym));

// Check if it's a known IR-only operation
let is_ir_only = matches!(
    self.interner.resolve(*head_sym),
    "dlog" | "shift" | "locf" | "cs1" | "ur"
);

if is_ir_only {
    Err(format!("{} (IR-only operation, requires Frame input)", err_msg))
} else {
    Err(err_msg)
}
```

**Verification command:**
```bash
cd /home/ubuntu/blisp && echo '(w5 (file "test.csv"))' | ./blisp 2>&1 | grep deprecated
# Should show: "Unknown function: w5 (deprecated alias, use 'wkd' instead)"
```

**Impact:** Clear guidance to users on correct naming

### Fix 5: Add Tripwire Tests

**Goal:** Prevent regressions in dispatch behavior

**Test file:** `tests/dispatch_tripwires.rs`

```rust
#[test]
fn test_ir_wins_over_builtin() {
    // Verify IR shadows builtin for Frame inputs
    let mut rt = Runtime::new();
    let code = r#"(wkd (file "test.csv"))"#;
    // Should use IR path (wkd_mask_weekends), not builtin_wkd
    // Add instrumentation to verify
}

#[test]
fn test_legacy_alias_breaks_ir_tree() {
    // Verify that legacy alias causes double-fail
    let mut rt = Runtime::new();
    let code = r#"(dlog (w5 20 PRC))"#;
    let result = eval_code(&mut rt, code, false, false);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown function: w5"));
}

#[test]
fn test_ir_only_no_fallback() {
    // Verify IR-only operations fail without builtin
    let mut rt = Runtime::new();
    let code = r#"(dlog 42)"#;
    let result = eval_code(&mut rt, code, false, false);
    assert!(result.is_err());
    // Should NOT contain "Unknown function" (IR recognized it)
}

#[test]
fn test_special_form_bypasses_ir() {
    // Verify special forms never reach IR
    let mut rt = Runtime::new();
    let code = r#"(defparameter x 10)"#;
    let result = eval_code(&mut rt, code, false, false);
    assert!(result.is_ok());
    // Should use legacy path, not IR
}

#[test]
fn test_mode_override_disables_fallback() {
    // Verify IR-ONLY mode rejects legacy operations
    let mut rt = Runtime::new();
    let code = r#"(print "hello")"#;
    let result = eval_code(&mut rt, code, false, true); // ir_only=true
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown function: print"));
}
```

**Verification command:**
```bash
cd /home/ubuntu/blisp && cargo test dispatch_tripwires
# All tests should pass
```

**Impact:** Catch dispatch regressions in CI

---

## Part F.5: Level 1 Migration (Completed 2026-02-27)

### Overview

**Commit:** 2141d00
**Date:** 2026-02-27
**Status:** ✅ COMPLETE and VERIFIED

Added 4 deprecated alias tokens to IR planner to eliminate double-fail pattern without breaking backward compatibility.

### Changes Applied

**File:** `src/planner.rs` (+77 lines)

| Alias | Planner Line | Delegates To | Status |
|-------|--------------|--------------|--------|
| `dlog-col` | 127-131 | dlog (NumericFunc::SHF_PTW_OBS_NLN_DLOG) | ✅ Dual-routing |
| `cs1-col` | 143-147 | cs1 (NumericFunc::SHF_PFX_LIN_SUM) | ✅ Dual-routing |
| `shift-col` | 166-183 | shift (NumericFunc::SHF_PTW_LIN_SHF{k}) | ✅ Dual-routing |
| `ur-col` | 498-556 | ur (full composite logic) | ✅ Dual-routing |

Each alias:
- Emits deprecation warning to stderr
- Routes through IR planner (eliminates double-fail)
- Uses identical logic as canonical name
- Maintains backward compatibility (builtins.rs unchanged)

### Problem Solved

**Before Level 1 Migration:**
```lisp
(dlog (dlog-col PRC))  → DOUBLE-FAIL
;; IR path:     dlog ✓ → dlog-col ✗ → Unknown function
;; Legacy path: dlog-col ✓ → dlog ✗ → Unknown function
```

**After Level 1 Migration:**
```lisp
(dlog (dlog-col PRC))  → SUCCESS (with warning)
;; IR path:     dlog ✓ → dlog-col ✓ (alias) → Success
;; Warning:     "dlog-col is deprecated, use dlog instead"
;; Legacy path: Never reached (IR succeeded)
```

### Test Results

All 5 test cases verified (see DOUBLE_FAIL_TEST_RESULTS.md):

1. ✅ `(dlog (dlog-col X))` - simple nested
2. ✅ `(shift 1 (shift-col 2 X))` - parameter passing
3. ✅ `(cs1 (cs1-col X))` - cumulative operations
4. ✅ `(locf (ur-col 5 1 X))` - complex composite
5. ✅ `(dlog (shift-col 1 (cs1-col X)))` - triple nesting

**Verified:**
- Deprecation warnings emit correctly
- IR routing confirmed (HYBRID mode indicator)
- No "Unknown function" errors
- Backward compatible (LEGACY mode still works)

### Impact on Dispatch Map

**Updated counts:**
- Dual-routing tokens: 9 → 13 (+4 deprecated aliases)
- Legacy-only tokens: 62 → 58 (-4 migrated to dual)
- Dangerous aliases: 5 → 1 (only w5 remains unfixed)

**Status changes:**
- `dlog-col`: Legacy-only → Dual (DEPRECATED)
- `shift-col`: Legacy-only → Dual (DEPRECATED)
- `cs1-col`: Legacy-only → Dual (DEPRECATED)
- `ur-col`: Legacy-only → Dual (DEPRECATED)

### Remaining Work

**Still broken:**
- `w5` (alias for wkd) - NOT in this migration, requires same fix pattern

**Next migrations:**
- Level 2: Add comparison operators (<, >=, <=, ==, !=) to IR
- Level 3: Add `w5` alias to complete dangerous alias migration
- Level 4 (breaking): Remove redundant builtin registrations after adoption period

### Verification Commands

```bash
cd /home/ubuntu/blisp

# Confirm aliases in planner
rg '"(dlog-col|shift-col|cs1-col|ur-col)"' src/planner.rs

# Confirm builtins still registered
rg 'register_builtin.*(dlog-col|shift-col|cs1-col|ur-col)' src/builtins.rs

# Test double-fail elimination
./target/debug/blisp -e '(dlog (dlog-col (file "test.csv")))' 2>&1 | grep Warning
# Expected: Warning: 'dlog-col' is deprecated, use 'dlog' instead
```

---

## Part G: Canonical End-to-End Traces

### Trace 1: `(file "tto.csv")` - Successful IR Path

**Input:**
```lisp
(file "tto.csv")
```

**Execution (HYBRID mode):**

1. **Parse:** Reader parses to `Expr::List([Expr::Sym("file"), Expr::Str("tto.csv")])`

2. **Entry:** main.rs:574 calls `try_ir_eval(rt, expr.clone())`

3. **Normalize:** main.rs:545 `normalize::normalize()`
   - Check: normalize.rs:53-57 - head is "file", not "->"
   - Result: No macro expansion, returns unchanged

4. **Plan:** main.rs:548 `planner::plan(&normalized, &interner)`
   - Entry: planner.rs:42 `plan()`
   - Match: planner.rs:86 `plan_expr()` → planner.rs:88 matches `"file"`
   - Parse: planner.rs:93 extracts path `"tto.csv"`
   - Build: planner.rs:98-104 creates:
     ```rust
     Node {
       id: NodeId(0),
       op: Operation::Source(Source::File { path: "tto.csv" }),
       schema: SchemaInfo::unknown()
     }
     ```
   - Return: `Ok(Plan { nodes: [node] })`

5. **Execute:** main.rs:551 `exec::execute(&plan, rt)`
   - Entry: exec.rs:48 `execute()`
   - Iterate: exec.rs:52 loops over `plan.nodes`
   - Dispatch: exec.rs:65 `execute_node()` → exec.rs:70 match `node.op`
   - Match: exec.rs:71 `Operation::Source(source)` → `execute_source()`
   - Route: exec.rs:82-88 matches `Source::File { path }`
   - **Kernel:** exec.rs:84 calls `io::load_csv("tto.csv", &mut rt.interner)`
     - Reads CSV file
     - Parses first column as Date index
     - Remaining columns as F64 data
     - Returns `Value::Frame(Arc<Frame>)`

6. **Result:** main.rs:582 `result = val` → Success ✅

**Path taken:** IR-only (no fallback)

**Verification:**
```bash
cd /home/ubuntu/blisp
echo "date;px" > tto.csv
echo "2024-01-01;100.0" >> tto.csv
./blisp -e '(file "tto.csv")'
# Expected: Table output with date index and px column
```

---

### Trace 2: `(dlog (wkd 20 PRC))` - Successful Nested IR Path

**Input:**
```lisp
(dlog (wkd 20 PRC))
```

**Assumptions:** PRC is a Frame variable already defined

**Execution (HYBRID mode):**

1. **Parse:**
   - Outer: `Expr::List([Expr::Sym("dlog"), ...])`
   - Inner: `Expr::List([Expr::Sym("wkd"), Expr::Int(20), Expr::Sym("PRC")])`

2. **Entry:** main.rs:574 calls `try_ir_eval(rt, expr.clone())`

3. **Normalize:** main.rs:545 - No macro expansion

4. **Plan (outer dlog):** main.rs:548 `planner::plan()`
   - Match: planner.rs:123 matches `"dlog"`
   - Recurse: Must plan inner arg `(wkd 20 PRC)` first

5. **Plan (inner wkd):** planner.rs:57 `plan_expr()` on `(wkd 20 PRC)`
   - Match: planner.rs:132 matches `"wkd"`
   - Args: `plan_unary(NumericFunc::MSK_WKE, &[20, PRC], ...)`
   - Recurse: Plan `PRC` variable
     - planner.rs:64-78 matches `Expr::Sym("PRC")`
     - Creates `Operation::Source(Source::Variable { name: "PRC" })`
   - Build: Creates unary node:
     ```rust
     Node {
       id: NodeId(1),
       op: Operation::Unary(UnaryOp::MapNumeric {
         input: NodeId(0),  // PRC variable node
         func: NumericFunc::MSK_WKE
       }),
       schema: SchemaInfo::unknown()
     }
     ```

6. **Plan (complete outer dlog):**
   - Build: Creates unary node:
     ```rust
     Node {
       id: NodeId(2),
       op: Operation::Unary(UnaryOp::MapNumeric {
         input: NodeId(1),  // wkd result
         func: NumericFunc::SHF_PTW_OBS_NLN_DLOG
       }),
       schema: SchemaInfo::unknown()
     }
     ```
   - Return: `Ok(Plan { nodes: [prc_node, wkd_node, dlog_node] })`

7. **Execute:** main.rs:551 `exec::execute(&plan, rt)`
   - Execute node 0: Load PRC variable → Frame
   - Execute node 1: exec.rs:126 `execute_unary()`
     - Match: exec.rs:133 special case for MSK_WKE
     - **Kernel:** exec.rs:134 `wkd_mask_weekends(&input_frame)`
   - Execute node 2: exec.rs:126 `execute_unary()`
     - Match: exec.rs:157 `NumericFunc::SHF_PTW_OBS_NLN_DLOG`
     - **Kernel:** exec.rs:157 `dlog_obs_column(col, 1)` via `map_numeric_preserve_tags()`

8. **Result:** main.rs:582 `result = val` → Success ✅

**Path taken:** Pure IR (no fallback needed)

**Verification:**
```bash
cd /home/ubuntu/blisp
./blisp -e '(defparameter PRC (file "tto.csv"))' -e '(dlog (wkd 20 PRC))'
# Expected: Frame with weekend values masked to NA, then dlog applied
```

---

### Trace 3: `(dlog (w5 20 PRC))` - BROKEN Mixed-Tree Example

**Input:**
```lisp
(dlog (w5 20 PRC))
```

**Problem:** IR-only outer (`dlog`) + legacy-only inner (`w5`)

**Execution (HYBRID mode):**

1. **Parse:**
   - Outer: `Expr::List([Expr::Sym("dlog"), ...])`
   - Inner: `Expr::List([Expr::Sym("w5"), Expr::Int(20), Expr::Sym("PRC")])`

2. **Entry:** main.rs:574 calls `try_ir_eval(rt, expr.clone())`

3. **Normalize:** main.rs:545 - No macro expansion

4. **Plan (outer dlog):** main.rs:548 `planner::plan()`
   - Match: planner.rs:123 matches `"dlog"` ✅
   - Recurse: Must plan inner arg `(w5 20 PRC)` first

5. **Plan (inner w5):** planner.rs:57 `plan_expr()` on `(w5 20 PRC)`
   - Try match: planner.rs:86 looks for `"w5"` in match arms
   - **FAIL:** ❌ NOT FOUND (planner.rs only knows `"wkd"`)
   - Falls through: planner.rs:640 `_ => Err(format!("Unknown function: {}", func_name))`
   - Returns: `Err("Unknown function: w5")`

6. **IR Failure:** Plan fails, propagates to main.rs:574

7. **Fallback Check:** main.rs:584
   - Condition: `e.contains("Unknown function")` ✅ TRUE
   - Action: main.rs:587 calls `rt.eval(&expr)` (legacy fallback)

8. **Legacy Eval (outer dlog):** eval.rs:11 `Runtime::eval()`
   - Entry: eval.rs:36 `eval_list()` for `(dlog (w5 20 PRC))`
   - Check macros: eval.rs:55-59 - no macro
   - Check special forms: eval.rs:66-79 - not a special form
   - Check builtin: eval.rs:82 `is_builtin(*head_sym)` for "dlog"
   - **FAIL:** ❌ NOT FOUND (builtins.rs has only `dlog-col`, not `dlog`)
   - Fallback: eval.rs:94 tries `resolve(*head_sym)` (variable lookup for "dlog")
   - Returns: `Err("Undefined variable: dlog")`

9. **Result:** main.rs:587 propagates error → **FAILURE** ❌

**Exact failure point:** planner.rs:640 "Unknown function: w5" → fallback → eval.rs:94 "Undefined variable: dlog"

**Error message seen by user:**
```
Error: Undefined variable: dlog
```

**Fix:**
```lisp
;; Use IR-compatible name:
(dlog (wkd 20 PRC))
```

**Verification:**
```bash
cd /home/ubuntu/blisp

# Broken version:
./blisp -e '(defparameter PRC (file "tto.csv"))' -e '(dlog (w5 20 PRC))' 2>&1
# Expected error: "Undefined variable: dlog"

# Fixed version:
./blisp -e '(defparameter PRC (file "tto.csv"))' -e '(dlog (wkd 20 PRC))'
# Expected: Success ✅
```

**Root cause:** `w5` exists ONLY in legacy (builtins.rs:187), `dlog` exists ONLY in IR (planner.rs:123). Nesting them causes **double-fail**.

**Prevention:** Add IR alias for `w5` (see Fix Plan #1)

---

## Summary of Dispatch Map

### Key Takeaways

1. **HYBRID mode (default) tries IR first** (main.rs:574)
2. **IR shadows builtins** - Dual-routing tokens ALWAYS use IR for Frame inputs
3. **Fallback triggers only on specific errors** (main.rs:584): "Cannot plan" / "not supported" / "Unknown function"
4. **7 IR-only operations have NO fallback**: dlog, shift, locf, cs1, ur, ret, log
5. **5 legacy-only aliases break IR trees**: w5, dlog-col, shift-col, ur-col, cs1-col
6. **9 dual-routing builtins are unreachable** in HYBRID mode for Frame inputs

### Dispatch Guarantee

For any expression in HYBRID mode:
1. Macro expansion happens first (normalize.rs:40)
2. IR planner tries to recognize ALL tokens recursively (planner.rs:86)
3. If ANY token is unrecognized → "Unknown function" → legacy fallback
4. If IR succeeds → IR result used, builtins never called
5. If legacy also fails → error propagated to user

### Critical Fixes Needed

1. **Add IR aliases** for legacy names (w5, dlog-col, shift-col) - ZERO breakage
2. **Remove unreachable builtins** (arithmetic, wkd, xminus, mapr) - Enforce single source
3. **Improve error messages** - Guide users to correct names
4. **Add tripwire tests** - Prevent dispatch regressions

---

**End of Authoritative Dispatch Map**
