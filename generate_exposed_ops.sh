#!/bin/bash
# Generate EXPOSED_OPS.md from source code

OUTPUT="EXPOSED_OPS.md"

cat > "$OUTPUT" << 'EOF'
# BLISP Exposed Operations Reference

**Generated from source code** - Do not edit by hand

**Version:** 0.2.0  
**Branch:** reconstruct/tableview-only  
**Generated:** $(date -u +"%Y-%m-%d %H:%M:%S UTC")

---

## Overview

This document catalogs all user-facing operations in BLISP, generated from the actual source code.

**Total Operations:** $(./target/release/blisp --dic 2>/dev/null | grep "Total operations:" | awk '{print $3}')

**Execution Paths:**
- **IR Planner** (src/planner.rs) - Compiles to fused execution plans
- **Builtins** (src/builtins.rs) - Runtime-callable functions
- **HYBRID mode** (default) - IR tries first, falls back to builtins

---

## Operation Categories

EOF

# Extract from --dic output
./target/release/blisp --dic 2>&1 | tail -n +5 >> "$OUTPUT"

cat >> "$OUTPUT" << 'EOF'

---

## IR Planner Operations

These operations are recognized directly by the IR planner (src/planner.rs) and compiled to optimized execution plans.

EOF

# Extract IR operations from planner.rs
echo "### Core Operations" >> "$OUTPUT"
echo "" >> "$OUTPUT"
echo "| Token | Maps to IR Op | Semantics | Defined at |" >> "$OUTPUT"
echo "|-------|---------------|-----------|------------|" >> "$OUTPUT"

grep -n "\"dlog\"" src/planner.rs | head -1 | awk -F: '{print "| `dlog` | SHF_PTW_OBS_NLN_DLOG | OBS (skip NA) | planner.rs:" $1 " |"}' >> "$OUTPUT"
grep -n "\"dlog-ofs\"" src/planner.rs | head -1 | awk -F: '{print "| `dlog-ofs` | SHF_PTW_OFS_NLN_DLOG | OFS (positional) | planner.rs:" $1 " |"}' >> "$OUTPUT"
grep -n "\"shift\"" src/planner.rs | head -1 | awk -F: '{print "| `shift` | SHF_OFS_LIN_IDT | Temporal shift | planner.rs:" $1 " |"}' >> "$OUTPUT"
grep -n "\"rolling-mean\"" src/planner.rs | head -1 | awk -F: '{print "| `rolling-mean` | SHF_WIN_LIN_AVG | Window average (OBS) | planner.rs:" $1 " |"}' >> "$OUTPUT"
grep -n "\"rolling-std\"" src/planner.rs | head -1 | awk -F: '{print "| `rolling-std` | SHF_WIN_NLN_SDV | Window std dev (OBS) | planner.rs:" $1 " |"}' >> "$OUTPUT"

echo "" >> "$OUTPUT"
echo "**Note:** IR operations are IR-only in some cases. Use \`BLISP_IR_ONLY=1\` or \`--ir-only\` flag." >> "$OUTPUT"
echo "" >> "$OUTPUT"

cat >> "$OUTPUT" << 'EOF'

---

## Builtin Operations

These operations are registered as runtime-callable functions (src/builtins.rs).

### Key Naming Conventions

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

## Detailed Operation Reference

EOF

# Generate detailed entries for key operations by grepping builtins.rs
echo "### File I/O" >> "$OUTPUT"
echo "" >> "$OUTPUT"

for op in file stdin save; do
    LINE=$(grep -n "fn builtin_$op" src/builtins.rs 2>/dev/null | head -1 | cut -d: -f1)
    if [ -n "$LINE" ]; then
        echo "#### \`$op\`" >> "$OUTPUT"
        echo "- **Defined at:** builtins.rs:$LINE" >> "$OUTPUT"
        echo "- **Type:** I/O operation" >> "$OUTPUT"
        echo "" >> "$OUTPUT"
    fi
done

echo "### Temporal Operations" >> "$OUTPUT"
echo "" >> "$OUTPUT"

for op in dlog diff shift; do
    LINE=$(grep -n "fn builtin_${op}\(" src/builtins.rs 2>/dev/null | head -1 | cut -d: -f1)
    if [ -n "$LINE" ]; then
        echo "#### \`$op\`" >> "$OUTPUT"
        echo "- **Defined at:** builtins.rs:$LINE" >> "$OUTPUT"
        # Check for IR mapping
        IR_LINE=$(grep -n "\"$op\"" src/planner.rs 2>/dev/null | head -1 | cut -d: -f1)
        if [ -n "$IR_LINE" ]; then
            echo "- **Also in IR:** planner.rs:$IR_LINE" >> "$OUTPUT"
        fi
        echo "" >> "$OUTPUT"
    fi
done

echo "### Aggregations" >> "$OUTPUT"
echo "" >> "$OUTPUT"

for op in sum sum0 mean mean0 std std0; do
    LINE=$(grep -n "fn builtin_${op}\(" src/builtins.rs 2>/dev/null | head -1 | cut -d: -f1)
    if [ -n "$LINE" ]; then
        echo "#### \`$op\`" >> "$OUTPUT"
        echo "- **Defined at:** builtins.rs:$LINE" >> "$OUTPUT"
        if [[ "$op" == *"0" ]]; then
            echo "- **Semantics:** NA-safe (skips NA values)" >> "$OUTPUT"
        else
            echo "- **Semantics:** Strict (propagates NA)" >> "$OUTPUT"
        fi
        echo "" >> "$OUTPUT"
    fi
done

cat >> "$OUTPUT" << 'EOF'

---

## Aliases and Deprecations

| Deprecated Token | Replacement | Status | Notes |
|------------------|-------------|--------|-------|
| `dlog-col` | `dlog` | ⚠️ Deprecated | Legacy compatibility |
| `shift-col` | `shift` | ⚠️ Deprecated | Legacy compatibility |
| `cs1-col` | `cs1` | ⚠️ Deprecated | Legacy compatibility |
| `ur-col` | `ur` | ⚠️ Deprecated | Legacy compatibility |
| `w5` | `wkd` | ✅ Both supported | Alias for weekday filter |

**Migration Plan:** Deprecated aliases will emit warnings in future versions.

---

## Mode-Specific Availability

| Operation | HYBRID | IR-only | Legacy | Notes |
|-----------|--------|---------|--------|-------|
| `dlog` | ✅ | ✅ | ✅ | Available everywhere |
| `rolling-mean` | ❌ | ✅ | ❌ | IR-only operation |
| `rolling-std` | ❌ | ✅ | ❌ | IR-only operation |
| `sum` | ✅ | ❌ | ✅ | Builtin only |
| `file` | ✅ | ✅ | ✅ | Available everywhere |

**How to use IR-only operations:**
```bash
BLISP_IR_ONLY=1 blisp -e '(rolling-mean 5 data)'
# or
blisp --ir-only -e '(rolling-mean 5 data)'
```

---

## Examples by Category

### Basic Arithmetic
```lisp
(+ 1 2)           ; Addition
(- 5 3)           ; Subtraction
(* 4 2)           ; Multiplication
(/ 10 2)          ; Division
```

### File I/O
```lisp
(file "data.csv")              ; Load CSV
(stdin)                        ; Read from stdin
(save "out.csv" table)         ; Save table to CSV
```

### Temporal Operations
```lisp
(dlog prices)                  ; Log returns (OBS semantics)
(diff prices)                  ; First difference
(shift prices 1)               ; Lag by 1 period
```

### Aggregations
```lisp
(sum col)                      ; Sum (propagates NA)
(sum0 col)                     ; Sum (skips NA)
(mean col)                     ; Mean (propagates NA)
(mean0 col)                    ; Mean (skips NA)
```

### Rolling Windows
```lisp
(wstd0 col 20)                 ; 20-period rolling std (skip NA)
(wzs col 25 1)                 ; Rolling z-score (window=25, step=1)
```

### Table Operations
```lisp
(col table "price")            ; Extract column
(setcol table "new" values)    ; Set column
(mapr left right)              ; LEFT JOIN by first column
```

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

## How to Verify This Document

1. **List all operations:**
   ```bash
   ./target/release/blisp --dic
   ```

2. **Check IR operations:**
   ```bash
   grep -n '"[a-z-]*"' src/planner.rs | grep "=>"
   ```

3. **Check builtins:**
   ```bash
   grep -n 'fn builtin_' src/builtins.rs | wc -l
   ```

4. **Regenerate this document:**
   ```bash
   ./generate_exposed_ops.sh
   ```

---

**Last updated:** Auto-generated from source code  
**Authoritative reference:** See BLISP_DISPATCH_MAP.md for complete dispatch logic
EOF

echo "Generated $OUTPUT"

