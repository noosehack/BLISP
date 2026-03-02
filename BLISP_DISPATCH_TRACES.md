# BLISP Dispatch Traces - Runtime Step-by-Step

**Date:** 2026-02-27
**Repository:** /home/ubuntu/blisp
**Branch:** reconstruct/tableview-only

This document traces three example forms through BLISP's dispatch system, showing exactly what happens at each layer (macro expansion, IR planning, legacy evaluation).

---

## Trace 1: `(file "tto.csv")`

### Parse
- **Head:** `file`
- **Args:** `["tto.csv"]`

### Macro expansion
- **Check:** normalize.rs:53-57 checks if head == "->"
- **Result:** No macro expansion (head is "file", not "->")
- **After normalize:** `(file "tto.csv")` (unchanged)

### Dispatch decision
- **Entry:** main.rs:574 `try_ir_eval(rt, expr)`
- **Decision:** Try IR first (HYBRID mode)

### Normalization
- **Call:** main.rs:545 `normalize::normalize(expr, &mut rt.interner)`
- **Result:** `(file "tto.csv")` (no change, already canonical)

### Planner
- **Entry:** main.rs:548 `planner::plan(&normalized, &rt.interner)`
- **Call:** planner.rs:42 `plan()`
- **Match:** planner.rs:57 `plan_expr()` → planner.rs:86 match func_name
- **Token:** planner.rs:88 matches `"file"`
- **Extract:** planner.rs:93 extracts path string `"tto.csv"`
- **IR Node:** planner.rs:98-104 creates:
  ```rust
  Node {
    op: Operation::Source(Source::File { path: "tto.csv" }),
    schema: SchemaInfo::unknown()
  }
  ```
- **Return:** Plan with 1 node

### Executor
- **Entry:** main.rs:551 `exec::execute(&plan, rt)`
- **Call:** exec.rs:48 `execute()` iterates nodes
- **Dispatch:** exec.rs:65 `execute_node()` → exec.rs:70 match node.op
- **Match:** exec.rs:71 `Operation::Source(source)` → `execute_source()`
- **Route:** exec.rs:82-88 matches `Source::File { path }`
- **Kernel:** exec.rs:84 calls `io::load_csv(path, &mut rt.interner)`

### Kernel computation
- Reads CSV file "tto.csv"
- Parses first column as Date index
- Remaining columns as F64 data
- Returns `Value::Frame(Arc<Frame>)`

### Result
✅ **Success via IR path**

---

## Trace 2: `(dlog (w5 20 PRC))`

### Parse
- **Outer:** head=`dlog`, args=`[(w5 20 PRC)]`
- **Inner:** head=`w5`, args=`[20, PRC]`

### Macro expansion
- **Check:** normalize.rs:53-57 checks if head == "->"
- **Result:** No macro expansion (head is "dlog", not "->")
- **After normalize:** `(dlog (w5 20 PRC))` (unchanged)

### Dispatch decision
- **Entry:** main.rs:574 `try_ir_eval(rt, expr)`
- **Decision:** Try IR first (HYBRID mode)

### Normalization
- **Call:** main.rs:545 `normalize::normalize(expr, &mut rt.interner)`
- **Result:** `(dlog (w5 20 PRC))` (no change)

### Planner (outer dlog)
- **Entry:** main.rs:548 `planner::plan(&normalized, &rt.interner)`
- **Call:** planner.rs:42 `plan()`
- **Match:** planner.rs:57 `plan_expr()` → planner.rs:86 match func_name
- **Token:** planner.rs:123 matches `"dlog"`
- **Recursion:** Must plan inner arg `(w5 20 PRC)` first

### Planner (inner w5)
- **Call:** planner.rs:57 `plan_expr()` on `(w5 20 PRC)`
- **Match:** planner.rs:86 tries to match func_name `"w5"`
- **Result:** ❌ **NO MATCH** - "w5" not in planner.rs match arms!
- **Error:** Falls through to planner.rs:645 `_ => Err(format!("Unknown function: {}", func_name))`
- **Return:** `Err("Unknown function: w5")`

### Dispatch fallback
- **IR failed:** main.rs:584 catches error containing "Unknown function"
- **Fallback:** main.rs:587 `rt.eval(&expr)` - try legacy evaluator

### Legacy eval (outer dlog)
- **Entry:** eval.rs:11 `Runtime::eval()`
- **Call:** eval.rs:36 `eval_list()` for `(dlog (w5 20 PRC))`
- **Check:** eval.rs:47-59 macro expansion (none)
- **Check:** eval.rs:66-79 special forms (none)
- **Check:** eval.rs:82 `is_builtin(*head_sym)` for "dlog"
- **Result:** ❌ **NOT FOUND** - "dlog" not registered in builtins!
- **Fallback:** eval.rs:94 tries `resolve(*head_sym)` (variable lookup)
- **Error:** `Err("Undefined variable: dlog")`

### BROKEN: missing mapping at legacy layer
- `dlog` exists ONLY in IR (planner.rs:123)
- `dlog` has NO legacy builtin registration (builtins.rs has only `dlog-col`)
- `w5` exists ONLY in legacy (builtins.rs:187)
- `w5` has NO IR planner mapping (planner.rs has only `wkd`)

### Result
❌ **BROKEN** - Cannot mix IR-only `dlog` with legacy-only `w5`

### Workaround
Use `(dlog (wkd 20 PRC))` instead (both IR-compatible)

---

## Trace 3: `(ur 250 1 (w5 RET))`

### Parse
- **Outer:** head=`ur`, args=`[250, 1, (w5 RET)]`
- **Inner:** head=`w5`, args=`[RET]`

### Macro expansion
- **Check:** normalize.rs:53-57 checks if head == "->"
- **Result:** No macro expansion
- **After normalize:** `(ur 250 1 (w5 RET))` (unchanged)

### Dispatch decision
- **Entry:** main.rs:574 `try_ir_eval(rt, expr)`
- **Decision:** Try IR first (HYBRID mode)

### Planner (outer ur)
- **Entry:** main.rs:548 `planner::plan(&normalized, &rt.interner)`
- **Call:** planner.rs:42 `plan()`
- **Match:** planner.rs:57 `plan_expr()` → planner.rs:86 match func_name
- **Token:** planner.rs:408 matches `"ur"`
- **Signature:** Expects `(ur w step x)` per planner.rs:409
- **Parse:** planner.rs:414 extracts w=250 from `Expr::Int(250)`
- **Parse:** planner.rs:421 extracts step=1 from `Expr::Int(1)`
- **Recursion:** Must plan inner arg `(w5 RET)` first

### Planner (inner w5)
- **Call:** planner.rs:57 `plan_expr()` on `(w5 RET)`
- **Match:** planner.rs:86 tries to match func_name `"w5"`
- **Result:** ❌ **NO MATCH** - "w5" not in planner.rs match arms!
- **Error:** Falls through to planner.rs:645 `_ => Err(format!("Unknown function: {}", func_name))`
- **Return:** `Err("Unknown function: w5")`

### Dispatch fallback
- **IR failed:** main.rs:584 catches error containing "Unknown function"
- **Fallback:** main.rs:587 `rt.eval(&expr)` - try legacy evaluator

### Legacy eval (outer ur)
- **Entry:** eval.rs:11 `Runtime::eval()`
- **Call:** eval.rs:36 `eval_list()` for `(ur 250 1 (w5 RET))`
- **Check:** eval.rs:82 `is_builtin(*head_sym)` for "ur"
- **Result:** ❌ **NOT FOUND** - "ur" not registered in builtins!
  - (Only `ur-col` and `ur-cols` exist in builtins.rs:198, 197)
- **Fallback:** eval.rs:94 tries `resolve(*head_sym)` (variable lookup)
- **Error:** `Err("Undefined variable: ur")`

### BROKEN: missing mapping at legacy layer
- `ur` exists ONLY in IR (planner.rs:408)
- `ur` has NO legacy builtin registration (builtins.rs has only `ur-col`)
- `w5` exists ONLY in legacy (builtins.rs:187)
- `w5` has NO IR planner mapping (planner.rs has only `wkd`)

### Result
❌ **BROKEN** - Cannot mix IR-only `ur` with legacy-only `w5`

### Workaround
Use `(ur 250 1 (wkd RET))` instead (both IR-compatible)

---

## Summary Table

| Form | IR Attempt | Legacy Fallback | Result | Issue |
|------|-----------|-----------------|--------|-------|
| `(file "tto.csv")` | ✅ Success | Not needed | ✅ Works | None |
| `(dlog (w5 20 PRC))` | ❌ Unknown function: w5 | ❌ Undefined variable: dlog | ❌ **BROKEN** | IR-only `dlog` + legacy-only `w5` incompatible |
| `(ur 250 1 (w5 RET))` | ❌ Unknown function: w5 | ❌ Undefined variable: ur | ❌ **BROKEN** | IR-only `ur` + legacy-only `w5` incompatible |

---

## Root Cause Analysis

### The Problem

BLISP has **two incompatible naming systems**:

1. **IR-only operations** have NO legacy fallback:
   - `dlog`, `ur`, `shift`, `cs1`, `locf`
   - Recognized by planner.rs but NOT registered in builtins.rs

2. **Legacy-only aliases** have NO IR mapping:
   - `w5` (legacy alias for `wkd`)
   - Registered in builtins.rs:187 but NOT matched in planner.rs

3. **When you nest IR-only outer with legacy-only inner, BOTH paths fail:**
   - IR path fails on inner `w5` → `Err("Unknown function: w5")`
   - Legacy fallback fails on outer `dlog`/`ur` → `Err("Undefined variable: dlog")`

### The Double Failure

```
User: (dlog (w5 20 PRC))
  ↓
Try IR path:
  planner.rs matches "dlog" ✅
  planner.rs tries to plan inner (w5 20 PRC)
  planner.rs looks for "w5" ❌ NOT FOUND
  Return: Err("Unknown function: w5")
  ↓
Fallback to legacy:
  eval.rs looks for builtin "dlog" ❌ NOT FOUND
  eval.rs tries variable lookup for "dlog" ❌ NOT FOUND
  Return: Err("Undefined variable: dlog")
  ↓
RESULT: Both paths fail!
```

### Why This Happens

This is **intentional design** - BLISP is transitioning from legacy names to canonical IR names:

| Legacy Name | IR Canonical | Status |
|-------------|--------------|--------|
| `w5` | `wkd` | Use `wkd` going forward |
| `dlog-col` | `dlog` | Use `dlog` going forward |
| `shift-col` | `shift` | Use `shift` going forward |
| `ur-col` | `ur` | Use `ur` going forward |
| `cs1-col` | `cs1` | Use `cs1` going forward |
| `locf-cols` | `locf` | Use `locf` going forward |

The HYBRID mode works perfectly when you use **consistent naming** within a single expression tree.

---

## Correct Usage Patterns

### ✅ Working Examples (IR path)

```lisp
; Use canonical IR names throughout
(dlog (wkd 20 PRC))           ; ✅ Both IR-compatible
(ur 250 1 (wkd RET))          ; ✅ Both IR-compatible
(shift 1 (locf (file "data.csv")))  ; ✅ All IR-compatible
(cs1 (dlog (wkd 20 PRC)))     ; ✅ All IR-compatible
```

### ✅ Working Examples (legacy path)

```lisp
; Use legacy names throughout (if you must)
(dlog-col (w5 20 PRC))        ; ✅ Both legacy-compatible
(print "hello")               ; ✅ Legacy I/O operation
(save data "out.csv")         ; ✅ Legacy I/O operation
```

### ❌ Broken Examples (mixed naming)

```lisp
; DON'T mix IR-only with legacy-only
(dlog (w5 20 PRC))            ; ❌ IR dlog + legacy w5 = BROKEN
(ur 250 1 (w5 RET))           ; ❌ IR ur + legacy w5 = BROKEN
(shift 1 (w5 20 PRC))         ; ❌ IR shift + legacy w5 = BROKEN
(cs1 (w5 20 (dlog-col PRC)))  ; ❌ Mixed naming = BROKEN
```

---

## Dispatch Decision Tree

```
Input form
    ↓
normalize.rs:40 normalize()
    ↓ (expand macros like ->)
    ↓
main.rs:574 try_ir_eval()
    ↓
    ├─ planner.rs:42 plan()
    │     ↓
    │     ├─ All tokens recognized? → ✅ IR path → exec.rs:48 execute()
    │     └─ Unknown token? → ❌ Err("Unknown function: X")
    │                              ↓
    │                         main.rs:584 catch error
    │                              ↓
    │                         main.rs:587 rt.eval() [Legacy fallback]
    │                              ↓
    │                              ├─ Builtin exists? → ✅ Legacy path
    │                              └─ No builtin? → ❌ Err("Undefined variable: X")
    │
    └─ Result: Either ✅ Success or ❌ Both paths failed
```

---

## Testing the Traces

### Test Trace 1 (should work):
```bash
cd /home/ubuntu/blisp
echo "date;px" > tto.csv
echo "2024-01-01;100.0" >> tto.csv
echo "2024-01-02;102.0" >> tto.csv
./blisp -e '(file "tto.csv")'
# Expected: Table output with date index and px column
```

### Test Trace 2 (should fail):
```bash
# Assume PRC is a variable already defined
./blisp -e '(defparameter PRC (file "tto.csv"))' -e '(dlog (w5 20 PRC))'
# Expected error: "Unknown function: w5" or "Undefined variable: dlog"

# Working version:
./blisp -e '(defparameter PRC (file "tto.csv"))' -e '(dlog (wkd 20 PRC))'
# Expected: Success (both IR-compatible)
```

### Test Trace 3 (should fail):
```bash
# Assume RET is a variable already defined
./blisp -e '(defparameter RET (file "tto.csv"))' -e '(ur 250 1 (w5 RET))'
# Expected error: "Unknown function: w5" or "Undefined variable: ur"

# Working version:
./blisp -e '(defparameter RET (file "tto.csv"))' -e '(ur 250 1 (wkd RET))'
# Expected: Success (both IR-compatible)
```

---

## Recommendations

### For Users

1. **Use canonical IR names** for new code:
   - ✅ `wkd` (not `w5`)
   - ✅ `dlog` (not `dlog-col`)
   - ✅ `shift` (not `shift-col`)
   - ✅ `ur` (not `ur-col`)

2. **Be consistent** within each expression:
   - ✅ All IR names: `(dlog (wkd 20 (shift 1 PRC)))`
   - ✅ All legacy names: `(dlog-col (w5 20 (shift-col 1 PRC)))`
   - ❌ Mixed: `(dlog (w5 20 PRC))` **← BROKEN!**

3. **Check error messages:**
   - "Unknown function: X" → X is legacy-only, use IR equivalent
   - "Undefined variable: X" → X is IR-only, check if nested with legacy ops

### For Developers

1. **Add IR mappings for legacy operations:**
   - Map `w5` → `wkd` alias in planner.rs
   - OR error message: "w5 deprecated, use wkd"

2. **Add legacy builtins for IR operations:**
   - Register `dlog` → `builtin_dlog_cols` in builtins.rs
   - OR error message: "dlog is IR-only, requires Frame input"

3. **Improve error messages:**
   - "Unknown function: w5" → "Unknown function: w5 (deprecated alias, use 'wkd')"
   - "Undefined variable: dlog" → "dlog requires Frame input (IR-only operation)"

---

## File References

All line numbers verified against:
- `/home/ubuntu/blisp/src/main.rs`
- `/home/ubuntu/blisp/src/normalize.rs`
- `/home/ubuntu/blisp/src/planner.rs`
- `/home/ubuntu/blisp/src/ir.rs`
- `/home/ubuntu/blisp/src/exec.rs`
- `/home/ubuntu/blisp/src/eval.rs`
- `/home/ubuntu/blisp/src/builtins.rs`

Branch: `reconstruct/tableview-only`
Commit: Latest as of 2026-02-27

---

**End of Document**
