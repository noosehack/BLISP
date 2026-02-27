# BLISP Dispatch Status: Corrected Analysis

**Date**: 2026-02-27
**Purpose**: Authoritative classification of all BLISP operations by routing path

---

## Executive Summary

**Total registered builtins**: 73
**Total planner-recognized tokens**: 43
**Dual-routing tokens (IR shadows builtin)**: 11
**Builtin-only tokens**: 62
**Planner-only tokens**: 32

### Critical Finding

The original audit incorrectly classified `file` and `wzs` as builtin-only. **They have dual routing** - both exist in planner.rs and builtins.rs. In HYBRID mode, the IR path wins for these tokens.

---

## Part 1: Dual-Routing Tokens (IR Shadows Builtin)

These 11 tokens are registered in BOTH planner.rs and builtins.rs:

```
*        → IR: BinaryFunc::MUL        | Builtin: builtin_mul
+        → IR: BinaryFunc::ADD        | Builtin: builtin_add
-        → IR: BinaryFunc::SUB        | Builtin: builtin_sub
/        → IR: BinaryFunc::DIV        | Builtin: builtin_div
>        → IR: BinaryFunc::GTR        | Builtin: builtin_gtr
file     → IR: Source::File           | Builtin: builtin_file
mapr     → IR: JoinOp::ALIGN          | Builtin: builtin_mapr
stdin    → IR: Source::Stdin          | Builtin: builtin_stdin
wkd      → IR: SchemaOp::MSK_WKE_DEF  | Builtin: builtin_wkd
wzs      → IR: NumericFunc::SHF_PTW_OBS_NLN_WZS | Builtin: builtin_wzs
xminus   → IR: JoinOp::XMINUS         | Builtin: builtin_xminus
```

**Dispatch Behavior (HYBRID mode)**:
- IR planner is tried FIRST (main.rs:574)
- If IR succeeds, builtin is NEVER called
- If IR fails with "Cannot plan" / "Unknown function" / "not supported", fallback to builtin
- For Frame inputs, IR always wins → **builtins are unreachable**

**Recommendation**: Remove builtin registrations for these 11 tokens to avoid maintenance burden of duplicate implementations.

---

## Part 2: Builtin-Only Tokens (62 tokens)

These operations are registered in builtins.rs but NOT recognized in planner.rs. They are reachable ONLY via legacy evaluator fallback.

### Category A: Critical Gaps (P0 Priority)

**Missing Comparison Operators** - break IR pipeline for predicates:

```
<        builtins.rs:125  builtin_lss       → MISSING in planner
>=       builtins.rs:126  builtin_gte       → MISSING in planner
<=       builtins.rs:127  builtin_lte       → MISSING in planner
==       builtins.rs:128  builtin_eql       → MISSING in planner
!=       builtins.rs:129  builtin_neq       → MISSING in planner
```

**Impact**: Any IR expression using these operators causes "Unknown function" error.
**Fix**: Add to planner.rs as BinaryFunc::LSS, GTE, LTE, EQL, NEQ.

---

### Category B: Dangerous Aliases (P1 Priority)

**Legacy-only names that break IR trees** when nested:

```
w5         builtins.rs:187  builtin_wkd      → Alias for wkd (which IS in planner!)
dlog-col   builtins.rs:133  builtin_dlog     → Alias for dlog (which IS in planner!)
shift-col  builtins.rs:134  builtin_shift    → Alias for shift (which IS in planner!)
cs1-col    builtins.rs:163  builtin_cs1      → Alias for cs1 (which IS in planner!)
ur-col     builtins.rs:173  builtin_ur       → Alias for ur (which IS in planner!)
```

**Double-Fail Example**:
```lisp
(dlog (w5 20 PRC))
;; IR path: planner recognizes "dlog" but NOT "w5" → IR fails "Unknown function: w5"
;; Legacy path: eval recognizes "w5" but NOT "dlog" → Legacy fails "Unknown function: dlog"
;; Result: TOTAL FAILURE on valid expression
```

**Impact**: HIGH - breaks valid user expressions in HYBRID mode
**Fix**: Add these 5 aliases to planner.rs:
```rust
"w5" => { /* delegate to wkd logic */ }
"dlog-col" => { /* delegate to dlog logic */ }
"shift-col" => { /* delegate to shift logic */ }
"cs1-col" => { /* delegate to cs1 logic */ }
"ur-col" => { /* delegate to ur logic */ }
```

---

### Category C: Column-wise Variants

```
>-col      builtins.rs:137  builtin_gtr_col   → Column broadcast variant of >
>-cols     builtins.rs:138  builtin_gtr_cols  → Multi-column variant
```

**Status**: Legacy syntax for vectorized operations. Low priority (use `map-cols` instead).

---

### Category D: Computational Operations (P2 Priority)

**Aggregations**:
```
sum        builtins.rs:139  builtin_sum
sum0       builtins.rs:140  builtin_sum0      (NA-as-zero variant)
mean       builtins.rs:141  builtin_mean
mean0      builtins.rs:142  builtin_mean0
std        builtins.rs:143  builtin_std
std0       builtins.rs:144  builtin_std0
```

**Rolling/Windowed**:
```
wstd       builtins.rs:191  builtin_wstd      → Windowed standard deviation
wstd0      builtins.rs:192  builtin_wstd0
wv         builtins.rs:193  builtin_wv        → Windowed variance
wz0        builtins.rs:195  builtin_wz0       → Windowed z-score (zero-fill)
```

**Transforms**:
```
diff       builtins.rs:132  builtin_diff
zscore     builtins.rs:197  builtin_zscore
chop       builtins.rs:198  builtin_chop      → Trim leading/trailing NAs
keep-shape builtins.rs:156  builtin_keep_shape
ecs1       builtins.rs:168  builtin_ecs1      → Exponential cumsum
```

**Analysis**: These are computationally valid Frame operations. Consider migrating to IR if performance critical.

---

### Category E: Multi-Column Operations

**Column variants** (suffixed with `-col`, `-cols`):
```
dlog-cols    builtins.rs:177  builtin_dlog_cols
shift-cols   builtins.rs:178  builtin_shift_cols
diff-cols    builtins.rs:179  builtin_diff_cols
locf-cols    builtins.rs:180  builtin_locf_cols
cs1-cols     builtins.rs:181  builtin_cs1_cols
ecs1-cols    builtins.rs:182  builtin_ecs1_cols
ur-cols      builtins.rs:183  builtin_ur_cols
wz0-cols     builtins.rs:184  builtin_wz0_cols
wstd-cols    builtins.rs:185  builtin_wstd_cols
wstd0-cols   builtins.rs:186  builtin_wstd0_cols
wv-cols      builtins.rs:188  builtin_wv_cols
keep-shape-cols builtins.rs:189 builtin_keep_shape_cols
```

**Status**: Legacy batch syntax. Modern approach: use `map-cols`.

---

### Category F: Table Manipulation (Low Priority)

**Schema operations** (not computational Frame ops):
```
col          builtins.rs:149  builtin_col       → Extract column by name
cols         builtins.rs:150  builtin_cols      → Extract multiple columns
setcol       builtins.rs:151  builtin_setcol    → Bind column name
withcol      builtins.rs:152  builtin_withcol   → Bind column for scope
w            builtins.rs:153  builtin_w         → Rolling window config
make-col     builtins.rs:154  builtin_make_col  → Construct column
select       builtins.rs:160  builtin_select    → Row filter
select-num   builtins.rs:161  builtin_select_num → Row filter by numeric mask
map-cols     builtins.rs:175  builtin_map_cols  → Apply fn to each column
apply-cols   builtins.rs:176  builtin_apply_cols → Apply to column set
```

**Analysis**: These are table manipulation primitives, not numeric operations. Correctly excluded from IR.

---

### Category G: Mask Operations (P3 Priority)

```
mask-on      builtins.rs:204  builtin_mask_on    → Set global mask
mask-off     builtins.rs:205  builtin_mask_off   → Clear global mask
mask-list    builtins.rs:206  builtin_mask_list  → List active masks
mask-stats   builtins.rs:207  builtin_mask_stats → Mask diagnostics
mask-define  builtins.rs:208  builtin_mask_define → Define named mask
```

**Status**: These are side-effect operations (mutate global state). Should NOT be in IR planner.

---

### Category H: I/O and Side Effects (Correct Exclusion)

```
file-head    builtins.rs:146  builtin_file_head  → Load first N rows
save         builtins.rs:148  builtin_save       → Write to file
print        builtins.rs:199  builtin_print      → Display to stdout
```

**Analysis**: Pure side effects. Correctly excluded from IR.

---

### Category I: Introspection (Correct Exclusion)

```
type-of      builtins.rs:200  builtin_type_of    → Reflect value type
len          builtins.rs:201  builtin_len        → Count elements
o            builtins.rs:202  builtin_o          → Debug inspect
```

**Analysis**: Meta-operations. Correctly excluded from IR.

---

## Part 3: Planner-Only Tokens (32 tokens)

These operations are recognized ONLY in planner.rs, NOT registered as builtins:

```
abs              planner.rs:104   NumericFunc::SHF_PTW_OBS_NLN_ABS
and              planner.rs:548   (special handling for boolean ops)
asofr            planner.rs:457   JoinOp::ASOF_ALIGN
cs1              planner.rs:120   NumericFunc::SHF_PTW_OBS_LIN_CS1
dlog             planner.rs:123   NumericFunc::SHF_PTW_OBS_NLN_DLOG
dlog-ofs         planner.rs:125   NumericFunc::SHF_PTW_OBS_NLN_DLOG (with offset)
exp              planner.rs:105   NumericFunc::SHF_PTW_OBS_NLN_EXP
ft-mean          planner.rs:296   NumericFunc::SHF_FTR_XXX_LIN_MEAN
ft-std           planner.rs:299   NumericFunc::SHF_FTR_XXX_LIN_STD
ft-zscore        planner.rs:302   NumericFunc::SHF_FTR_XXX_LIN_ZSCORE
inv              planner.rs:106   NumericFunc::SHF_PTW_OBS_NLN_INV
keep             planner.rs:108   NumericFunc::SHF_PTW_OBS_XXX_KEEP
lag-obs          planner.rs:115   (internal alias handling)
let              planner.rs:86    (special form for bindings)
locf             planner.rs:109   NumericFunc::SHF_PTW_OBS_XXX_LOCF
log              planner.rs:107   NumericFunc::SHF_PTW_OBS_NLN_LOG
mask-weekend     planner.rs:135   SchemaOp::MSK_WKE
not              planner.rs:554   (special handling for boolean ops)
or               planner.rs:551   (special handling for boolean ops)
read-csv         planner.rs:88    Source::File (alias for "file")
ret              planner.rs:113   NumericFunc::SHF_PTW_OBS_NLN_RET
rolling-mean     planner.rs:273   NumericFunc::SHF_ROL_XXX_LIN_MEAN
rolling-mean-min2 planner.rs:285  NumericFunc::SHF_ROL_XXX_LIN_MEAN (min_valid=2)
rolling-std      planner.rs:278   NumericFunc::SHF_ROL_XXX_LIN_STD
rolling-std-min2 planner.rs:290   NumericFunc::SHF_ROL_XXX_LIN_STD (min_valid=2)
rolling-zscore   planner.rs:320   NumericFunc::SHF_PTW_OBS_NLN_WZS (alias)
shift            planner.rs:114   NumericFunc::SHF_PTW_OBS_LIN_SHF
shift-obs        planner.rs:115   (internal alias)
sqrt             planner.rs:103   NumericFunc::SHF_PTW_OBS_NLN_SQRT
ur               planner.rs:121   NumericFunc::SHF_PTW_OBS_XXX_UR
with-mask        planner.rs:133   SchemaOp::WTH_MSK
```

**Behavior (HYBRID mode)**:
- These work ONLY if top-level form is plannable
- If nested inside legacy builtin, causes "Unknown function" error
- Example failure: `(sum (dlog RET))` — sum is builtin-only, dlog is planner-only

---

## Part 4: Verification Commands

### Check if token is in planner:
```bash
rg '"TOKEN"' src/planner.rs
```

### Check if token is registered builtin:
```bash
rg 'register_builtin.*"TOKEN"' src/builtins.rs
```

### Extract all planner tokens:
```bash
rg '^\s*"[^"]+"\s*[\|=]' src/planner.rs | grep -o '"[^"]*"' | tr -d '"' | sort -u
```

### Extract all builtin tokens:
```bash
rg 'register_builtin\("([^"]+)"' src/builtins.rs -o -r '$1' | sort -u
```

### Find dual-routing tokens:
```bash
comm -12 <(rg 'register_builtin\("([^"]+)"' src/builtins.rs -o -r '$1' | sort) \
         <(rg '^\s*"[^"]+"\s*[\|=]' src/planner.rs | grep -o '"[^"]*"' | tr -d '"' | sort)
```

### Find builtin-only tokens:
```bash
comm -23 <(rg 'register_builtin\("([^"]+)"' src/builtins.rs -o -r '$1' | sort) \
         <(rg '^\s*"[^"]+"\s*[\|=]' src/planner.rs | grep -o '"[^"]*"' | tr -d '"' | sort)
```

---

## Part 5: Priority Fix Plan

### P0: Add Missing Comparison Operators to Planner

**File**: `src/planner.rs` (around line 520, with other binary ops)

```rust
"<" => {
    ensure_args_count(func_name, &children, 2)?;
    let lhs = plan(&children[0], rt)?;
    let rhs = plan(&children[1], rt)?;
    Ok(Node::new(Operation::Binary(BinaryFunc::LSS, Box::new(lhs), Box::new(rhs))))
}
"<=" => {
    ensure_args_count(func_name, &children, 2)?;
    let lhs = plan(&children[0], rt)?;
    let rhs = plan(&children[1], rt)?;
    Ok(Node::new(Operation::Binary(BinaryFunc::LTE, Box::new(lhs), Box::new(rhs))))
}
">=" => {
    ensure_args_count(func_name, &children, 2)?;
    let lhs = plan(&children[0], rt)?;
    let rhs = plan(&children[1], rt)?;
    Ok(Node::new(Operation::Binary(BinaryFunc::GTE, Box::new(lhs), Box::new(rhs))))
}
"==" => {
    ensure_args_count(func_name, &children, 2)?;
    let lhs = plan(&children[0], rt)?;
    let rhs = plan(&children[1], rt)?;
    Ok(Node::new(Operation::Binary(BinaryFunc::EQL, Box::new(lhs), Box::new(rhs))))
}
"!=" => {
    ensure_args_count(func_name, &children, 2)?;
    let lhs = plan(&children[0], rt)?;
    let rhs = plan(&children[1], rt)?;
    Ok(Node::new(Operation::Binary(BinaryFunc::NEQ, Box::new(lhs), Box::new(rhs))))
}
```

**Also add to**: `src/ir.rs` BinaryFunc enum and `src/exec.rs` dispatcher.

---

### P1: Add Dangerous Aliases to Planner

**File**: `src/planner.rs` (near existing token matches)

```rust
"w5" => {
    // Deprecated alias for wkd (weekday mask)
    eprintln!("Warning: 'w5' is deprecated, use 'wkd' instead");
    // Delegate to wkd logic (copy from line 132)
    ensure_args_count("wkd", &children, 2)?;
    let window = parse_int(&children[0], rt)?;
    let frame_expr = &children[1];
    let frame_node = plan(frame_expr, rt)?;
    Ok(Node::new(Operation::Schema(
        SchemaOp::MSK_WKE_DEF,
        vec![Operand::Const(window)],
        Box::new(frame_node)
    )))
}

"dlog-col" => {
    // Deprecated alias for dlog
    eprintln!("Warning: 'dlog-col' is deprecated, use 'dlog' instead");
    // Delegate to dlog logic (copy from line 123)
    // ... (same as dlog implementation)
}

"shift-col" => {
    // Deprecated alias for shift
    eprintln!("Warning: 'shift-col' is deprecated, use 'shift' instead");
    // Delegate to shift logic (copy from line 114)
}

"cs1-col" => {
    // Deprecated alias for cs1
    eprintln!("Warning: 'cs1-col' is deprecated, use 'cs1' instead");
    // Delegate to cs1 logic (copy from line 120)
}

"ur-col" => {
    // Deprecated alias for ur
    eprintln!("Warning: 'ur-col' is deprecated, use 'ur' instead");
    // Delegate to ur logic (copy from line 121)
}
```

---

### P2: Remove Redundant Builtin Registrations

**File**: `src/builtins.rs` (remove these 11 lines)

```rust
// REMOVE: These are shadowed by IR in HYBRID mode
// rt.register_builtin("*", builtin_mul);        // line 123
// rt.register_builtin("+", builtin_add);        // line 121
// rt.register_builtin("-", builtin_sub);        // line 122
// rt.register_builtin("/", builtin_div);        // line 124
// rt.register_builtin(">", builtin_gtr);        // line 131
// rt.register_builtin("file", builtin_file);    // line 145
// rt.register_builtin("mapr", builtin_mapr);    // line 170
// rt.register_builtin("stdin", builtin_stdin);  // line 147
// rt.register_builtin("wkd", builtin_wkd);      // line 186
// rt.register_builtin("wzs", builtin_wzs);      // line 194
// rt.register_builtin("xminus", builtin_xminus); // line 171
```

**Rationale**: These 11 tokens have IR implementations. The builtin versions are unreachable in HYBRID mode for Frame inputs.

---

### P3: Add Tripwire Tests

**File**: `tests/dispatch_regression.rs` (new file)

```rust
// Test that dangerous aliases work in nested contexts
#[test]
fn test_w5_alias_in_ir_tree() {
    let result = eval("(dlog (w5 20 PRC))");
    assert!(result.is_ok(), "w5 alias should work in IR tree");
}

#[test]
fn test_comparison_ops_in_ir() {
    let tests = vec![
        "(< A B)",
        "(<= A B)",
        "(>= A B)",
        "(== A B)",
        "(!= A B)",
    ];
    for expr in tests {
        let result = try_ir_eval(expr);
        assert!(result.is_ok(), "Comparison {} should plan to IR", expr);
    }
}
```

---

## Part 6: Summary Table

| Category | Count | Status | Priority | Notes |
|----------|-------|--------|----------|-------|
| **Dual-routing** | 11 | IR shadows builtin | P2 | Remove redundant builtins |
| **Builtin-only (P0)** | 5 | Missing from planner | **P0** | Comparison ops: <, >=, <=, ==, != |
| **Builtin-only (P1)** | 5 | Dangerous aliases | **P1** | w5, dlog-col, shift-col, cs1-col, ur-col |
| **Builtin-only (P2)** | 9 | Computational ops | P2 | diff, wstd, wv, wz0, zscore, chop, keep-shape, ecs1 |
| **Builtin-only (P3)** | 5 | Mask operations | P3 | Consider adding if needed |
| **Builtin-only (schema)** | 10 | Table manipulation | ✓ OK | Correctly excluded from IR |
| **Builtin-only (suffixed)** | 12 | Column variants | ✓ OK | Use map-cols instead |
| **Builtin-only (I/O)** | 3 | Side effects | ✓ OK | Correctly excluded from IR |
| **Builtin-only (meta)** | 3 | Introspection | ✓ OK | Correctly excluded from IR |
| **Planner-only** | 32 | IR-only ops | ✓ OK | No legacy fallback |

**Total Coverage**: 73 builtins + 32 planner-only = 105 unique operations

---

## Part 7: Corrected Classification of "file" and "wzs"

### Original Error in Audit

The BLISP_BUILTIN_ONLY_AUDIT.md incorrectly listed:
- `file` as builtin-only (Category H: I/O operations)
- `wzs` as builtin-only (Category E: Rolling operations)

### Correction

**Both tokens have DUAL ROUTING**:

```
file:
  - planner.rs:88   → Source::File
  - builtins.rs:145 → builtin_file
  - Status: IR SHADOWS builtin in HYBRID mode

wzs (rolling-zscore):
  - planner.rs:320  → NumericFunc::SHF_PTW_OBS_NLN_WZS (via "rolling-zscore" alias)
  - builtins.rs:194 → builtin_wzs
  - Status: IR SHADOWS builtin in HYBRID mode
```

### Impact

In HYBRID mode (default), for Frame inputs:
- `(file "data.csv")` → IR handles it
- `(wzs 250 0 1 RET)` → IR handles it
- Builtin implementations are NEVER called

**Recommendation**: Remove both builtin registrations (lines 145, 194 in builtins.rs).

---

## Conclusion

This corrected analysis identifies:
- **11 dual-routing tokens** (IR shadows builtin) → remove redundant builtins
- **5 critical missing ops** (comparison operators) → add to planner (P0)
- **5 dangerous aliases** (w5, etc.) → add to planner (P1)
- **9 computational ops** → consider migrating to IR (P2)
- **38 correctly excluded ops** (schema, I/O, meta) → no action needed

**Next Step**: Implement P0 and P1 fixes to eliminate double-fail pattern.
