# Dispatch Audit Correction Summary

**Date**: 2026-02-27

## Error Found and Corrected

The original BLISP_BUILTIN_ONLY_AUDIT.md contained classification errors for 2 tokens that were incorrectly listed as "builtin-only" when they actually have dual routing.

---

## Tokens Misclassified

### 1. `file` (I/O operation)
- **Original claim**: Builtin-only (Category H)
- **Reality**: DUAL-ROUTING
  - planner.rs:88 → `"load" | "read-csv" | "file"` → Source::File
  - builtins.rs:145 → `rt.register_builtin("file", builtin_file)`
- **HYBRID mode behavior**: IR path wins, builtin never called for Frame inputs

### 2. `wzs` (rolling z-score)
- **Original claim**: Builtin-only (Category E)
- **Reality**: DUAL-ROUTING
  - planner.rs:320 → `"rolling-zscore" | "wzs"` → NumericFunc::SHF_PTW_OBS_NLN_WZS
  - builtins.rs:194 → `rt.register_builtin("wzs", builtin_wzs)`
- **HYBRID mode behavior**: IR path wins, builtin never called for Frame inputs

---

## Root Cause of Error

The original token extraction from planner.rs used:
```bash
rg 'func_name == "([^"]+)"' src/planner.rs -o -r '$1' | sort -u
```

This pattern captured only EXACT matches of `func_name == "TOKEN"`, but missed:
- **Multi-token match arms**: `"load" | "read-csv" | "file"`
- **Alias patterns**: `"rolling-zscore" | "wzs"`

The extraction returned 36 tokens, missing 7 tokens that use OR patterns in match arms.

---

## Corrected Extraction Method

Use a broader pattern to capture all string literals in match arms:
```bash
rg '^\s*"[^"]+"\s*[\|=]' src/planner.rs | grep -o '"[^"]*"' | tr -d '"' | sort -u
```

This correctly returns **43 tokens** in planner.rs.

---

## Updated Token Counts

| Category | Original Count | Corrected Count | Change |
|----------|----------------|-----------------|--------|
| **Planner tokens** | 36 | 43 | +7 tokens found |
| **Builtin tokens** | 73 | 73 | (unchanged) |
| **Dual-routing** | 9 | 11 | +2 (file, wzs) |
| **Builtin-only** | 64 | 62 | -2 (file, wzs) |
| **Planner-only** | 27 | 32 | +5 tokens found |

---

## 7 Missing Tokens from Original Extraction

The corrected extraction found these additional planner tokens:
1. `file` (was missed by original pattern)
2. `wzs` (was missed by original pattern)
3. `load` (alias for file)
4. `read-csv` (alias for file)
5. `rolling-zscore` (alias for wzs)
6. `lag-obs` (alias for shift)
7. `shift-obs` (alias for shift)

---

## Complete Dual-Routing List (11 tokens)

```
*         IR: BinaryFunc::MUL        | Builtin: builtin_mul
+         IR: BinaryFunc::ADD        | Builtin: builtin_add
-         IR: BinaryFunc::SUB        | Builtin: builtin_sub
/         IR: BinaryFunc::DIV        | Builtin: builtin_div
>         IR: BinaryFunc::GTR        | Builtin: builtin_gtr
file      IR: Source::File           | Builtin: builtin_file      ← CORRECTED
mapr      IR: JoinOp::ALIGN          | Builtin: builtin_mapr
stdin     IR: Source::Stdin          | Builtin: builtin_stdin
wkd       IR: SchemaOp::MSK_WKE_DEF  | Builtin: builtin_wkd
wzs       IR: NumericFunc::...WZS    | Builtin: builtin_wzs       ← CORRECTED
xminus    IR: JoinOp::XMINUS         | Builtin: builtin_xminus
```

---

## Impact on Original Audit Categories

### Category E: Rolling Window Operations (was 9 tokens, now 8)
**Removed**: `wzs` (moved to dual-routing)

Remaining builtin-only rolling ops:
- wstd, wstd0, wv, wz0 (and their -cols variants)

### Category H: I/O and Side Effects (was 4 tokens, now 3)
**Removed**: `file` (moved to dual-routing)

Remaining builtin-only I/O ops:
- file-head, save, print

---

## Corrected Priority Fix List

**No change to priorities**, but counts adjusted:

- **P0 (Critical)**: 5 comparison ops (<, >=, <=, ==, !=) — unchanged
- **P1 (High)**: 5 dangerous aliases (w5, dlog-col, shift-col, cs1-col, ur-col) — unchanged
- **P2 (Medium)**: 9 computational ops — unchanged
- **P3 (Low)**: 5 mask ops — unchanged
- **Correctly excluded**: 38 ops (was 36, +2 from reclassification)

---

## Verification Commands Used

### Extract all planner tokens (CORRECT method):
```bash
cd /home/ubuntu/blisp
rg '^\s*"[^"]+"\s*[\|=]' src/planner.rs | grep -o '"[^"]*"' | tr -d '"' | sort -u > /tmp/planner_tokens_fresh.txt
wc -l /tmp/planner_tokens_fresh.txt
# Output: 43
```

### Check specific tokens:
```bash
# file in planner?
rg '"file"' src/planner.rs
# Output: "load" | "read-csv" | "file" =>

# wzs in planner?
rg '"wzs"' src/planner.rs
# Output: "rolling-zscore" | "wzs" =>

# file in builtins?
rg 'register_builtin.*"file"' src/builtins.rs
# Output: rt.register_builtin("file", builtin_file);

# wzs in builtins?
rg 'register_builtin.*"wzs"' src/builtins.rs
# Output: rt.register_builtin("wzs", builtin_wzs);
```

### Compute corrected dual-routing:
```bash
comm -12 <(rg 'register_builtin\("([^"]+)"' src/builtins.rs -o -r '$1' | sort) \
         <(rg '^\s*"[^"]+"\s*[\|=]' src/planner.rs | grep -o '"[^"]*"' | tr -d '"' | sort)
# Output: 11 tokens including file and wzs
```

---

## Files Updated

1. **BLISP_DISPATCH_STATUS_CORRECTED.md** (NEW)
   - Complete rewrite with corrected classification
   - 11 dual-routing tokens (not 9)
   - 62 builtin-only tokens (not 64)
   - Detailed analysis per category
   - Priority fix plan unchanged

2. **DISPATCH_AUDIT_CORRECTION_SUMMARY.md** (THIS FILE)
   - Documents the error and correction
   - Explains root cause
   - Lists impacted categories
   - Provides verification commands

---

## Conclusion

The original audit was 97% accurate (2 tokens misclassified out of 105 total operations). The error was due to a regex pattern that missed multi-token match arms in planner.rs.

**Corrected status**:
- ✅ Priority recommendations unchanged (P0/P1/P2/P3)
- ✅ Double-fail pattern analysis still valid
- ✅ Fix plan still applicable
- ⚠️ Token counts adjusted: 11 dual-routing (was 9), 62 builtin-only (was 64)

**No action required** beyond updating documentation. The core findings and fix priorities remain accurate.
