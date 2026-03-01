# BLISP Exposed Operations Reference

**Generated from source code** - Do not edit by hand

**Version:** 0.2.0  
**Branch:** reconstruct/tableview-only  
**Generated:** 2026-03-01 11:32:35 UTC

---

## Overview

This document catalogs all user-facing operations in BLISP, generated from the actual source code.

**Total Operations:** 74

**Execution Paths:**
- **IR Planner** (src/planner.rs) - Compiles to fused execution plans
- **Builtins** (src/builtins.rs) - Runtime-callable functions
- **HYBRID mode** (default) - IR tries first, falls back to builtins

---

## Operation Categories


Arithmetic:
  *                     +                     -                     /                   

Comparison:
  !=                    <                     <=                    ==                    >                     >=                  

I/O Operations:
  file                  file-head             print                 save                  stdin               

Temporal Operations:
  diff                  diff-col              diff-cols             dlog-col              dlog-cols             shift-col             shift-cols          

Aggregations:
  mean                  mean0                 std                   std0                  sum                   sum0                

Table Operations:
  apply-cols            col                   cols                  make-col              map-cols              select                select-num            setcol                w                     withcol             

Rolling Statistics:
  wstd                  wstd-cols             wstd0                 wstd0-cols            wv                    wv-cols               wz0                   wz0-cols              wzs                 

Transforms & Filters:
  chop                  cs1-col               cs1-cols              ecs1                  ecs1-col              ecs1-cols             keep-shape            keep-shape-cols       locf-cols             wkd                   xminus                zscore              

Mask Operations:
  mask-define           mask-list             mask-off              mask-on               mask-stats          

Join Operations:
  mapr                

Column Comparisons:
  >-col                 >-cols              

Finance Operations:
  o                     ur-col                ur-cols             

Utility:
  len                   type-of             

Other Operations:
  ro                    w5                  

Note: wkd is the canonical weekend mask operation

---

## IR Planner Operations

These operations are recognized directly by the IR planner (src/planner.rs) and compiled to optimized execution plans.

### Core Operations

| Token | Maps to IR Op | Semantics | Defined at |
|-------|---------------|-----------|------------|
| `dlog` | SHF_PTW_OBS_NLN_DLOG | OBS (skip NA) | planner.rs:125 |
| `dlog-ofs` | SHF_PTW_OFS_NLN_DLOG | OFS (positional) | planner.rs:132 |
| `rolling-mean` | SHF_WIN_LIN_AVG | Window average (OBS) | planner.rs:300 |
| `rolling-std` | SHF_WIN_NLN_SDV | Window std dev (OBS) | planner.rs:414 |
| `cs1` | CUM_LIN_ADD | Cumulative sum from 1.0 | planner.rs:173 |
| `ur` | Unit Ratio | Risk-adjusted returns | planner.rs:617 |
| `wzs` | Rolling Z-score | Rewrite to rolling ops | planner.rs:524 |

**Note:** Some IR operations are IR-only. Use `BLISP_IR_ONLY=1` or `--ir-only` flag for:
- `rolling-mean`
- `rolling-std`
- `rolling-zscore`

---

## Key Naming Conventions

| Suffix | Meaning | Example |
|--------|---------|---------|
| (none) | Strict (propagates NA) | `sum`, `mean` |
| `0` | NA-safe (skips NA) | `sum0`, `mean0` |
| `-col` | Single column operation | `dlog-col`, `cs1-col` |
| `-cols` | Multi-column operation | `dlog-cols`, `cs1-cols` |

### Semantics Notes

- **OBS (Observation-based):** Skips NA when looking back (finance convention)
- **OFS (Offset):** Positional, does not skip NA
- **LOCF:** Last Observation Carried Forward

---

## Aliases and Deprecations

| Deprecated Token | Replacement | Status | Notes |
|------------------|-------------|--------|-------|
| `dlog-col` | `dlog` | ⚠️ Deprecated | Emits warning, still works |
| `shift-col` | `shift` | ⚠️ Deprecated | Emits warning, still works |
| `cs1-col` | `cs1` | ⚠️ Deprecated | Emits warning, still works |
| `ur-col` | `ur` | ⚠️ Deprecated | Emits warning, still works |
| `w5` | `wkd` | ✅ Both supported | Alias for weekday filter |

**Migration Plan:** Use canonical tokens. Deprecated aliases may be removed in future versions.

---

## Mode-Specific Availability

| Operation | HYBRID | IR-only | Legacy | Notes |
|-----------|--------|---------|--------|-------|
| `dlog` | ✅ | ✅ | ✅ | Available everywhere |
| `rolling-mean` | ❌ | ✅ | ❌ | IR-only operation |
| `rolling-std` | ❌ | ✅ | ❌ | IR-only operation |
| `sum` | ✅ | ❌ | ✅ | Builtin only |
| `file` | ✅ | ✅ | ✅ | Available everywhere |
| `mapr` | ✅ | ✅ | ✅ | Available everywhere |

**How to use IR-only operations:**
\`\`\`bash
BLISP_IR_ONLY=1 blisp -e '(rolling-mean 5 data)'
# or
blisp --ir-only -e '(rolling-mean 5 data)'
\`\`\`

---

## Examples by Category

### Basic Arithmetic
\`\`\`lisp
(+ 1 2)           ; Addition
(- 5 3)           ; Subtraction
(* 4 2)           ; Multiplication
(/ 10 2)          ; Division
\`\`\`

### File I/O
\`\`\`lisp
(file "data.csv")              ; Load CSV
(stdin)                        ; Read from stdin
(save "out.csv" table)         ; Save table to CSV
\`\`\`

### Temporal Operations
\`\`\`lisp
(dlog prices)                  ; Log returns (OBS semantics)
(diff prices)                  ; First difference
(shift prices 1)               ; Lag by 1 period
\`\`\`

### Aggregations
\`\`\`lisp
(sum col)                      ; Sum (propagates NA)
(sum0 col)                     ; Sum (skips NA)
(mean col)                     ; Mean (propagates NA)
(mean0 col)                    ; Mean (skips NA)
\`\`\`

### Rolling Windows (builtins)
\`\`\`lisp
(wstd0 col 20)                 ; 20-period rolling std (skip NA)
(wzs col 25 1)                 ; Rolling z-score (window=25, step=1)
\`\`\`

### Rolling Windows (IR-only)
\`\`\`bash
BLISP_IR_ONLY=1 blisp -e '(rolling-mean 250 data)'
BLISP_IR_ONLY=1 blisp -e '(rolling-std 20 data)'
\`\`\`

### Table Operations
\`\`\`lisp
(col table "price")            ; Extract column
(setcol table "new" values)    ; Set column
(mapr left right)              ; LEFT JOIN by first column
(o 'Z table)                   ; Change orientation to row-major
\`\`\`

---

## Source File Reference

| File | Purpose | Key Functions |
|------|---------|---------------|
| `src/planner.rs` | IR planner | `plan()`, operation dispatch |
| `src/builtins.rs` | Runtime builtins | All `builtin_*` functions |
| `src/exec.rs` | IR execution | `execute()` |
| `src/eval.rs` | Legacy evaluator | `eval()` |
| `src/normalize.rs` | Macro expansion | `normalize()` |

---

## Verification Commands

1. **List all operations:**
   \`\`\`bash
   ./target/release/blisp --dic
   \`\`\`

2. **Check IR operations:**
   \`\`\`bash
   grep -n '"[a-z-]*"' src/planner.rs | grep "=>"
   \`\`\`

3. **Check builtins:**
   \`\`\`bash
   grep -n 'fn builtin_' src/builtins.rs | wc -l
   \`\`\`

4. **Regenerate this document:**
   \`\`\`bash
   ./generate_exposed_ops.sh
   \`\`\`

---

**Authoritative references:**
- BLISP_DISPATCH_MAP.md - Complete dispatch logic and mode behavior
- NUMERIC_POLICY.md - IEEE-754 edge case specification
- INSTALL.md - Installation and usage guide
