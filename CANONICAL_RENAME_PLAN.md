# BLISP Canonical Rename Plan

**Date:** 2026-02-26
**Status:** Planning Phase - DO NOT EXECUTE YET
**Scope:** 83 operations renamed from readable to taxonomic canonical names

---

## Executive Summary

Rename all 83 BLISP operations from current readable names (e.g., `wstd`, `locf`, `dlog`) to structured canonical names (e.g., `SHF_WIN_NLN_SDV`, `SHF_REC_NLN_LOCF`, `DLOG`) to enable:

1. **Grepability** - Find all operations by category (`grep "SHF_"`, `grep "NLN_"`)
2. **Taxonomic Classification** - Encode semantics in name (shift/window/linear/nonlinear)
3. **Invariance Analysis** - Discover mathematical properties by prefix patterns
4. **Code Organization** - Clear namespace separation (MSK_, SHF_, RSK_ADJ_)

**User Impact:** NONE (when complete) - Macros will provide ergonomic aliases back to familiar names.

---

## Motivation

### Problem
Current names are readable but unstructured:
- Can't grep for "all window operations" or "all nonlinear operations"
- No way to discover invariances (linearity, causality, etc.)
- Semantic properties hidden in documentation, not in code

### Solution
Two-layer architecture:
```
User writes:     (wstd (locf (dlog prices 1)) 20)
                        ↓ [macro expansion]
Canonical:       (SHF_WIN_NLN_SDV (SHF_REC_NLN_LOCF (DLOG prices 1)) 20)
                        ↓ [planner]
IR execution:    RollStd{w=20}(Locf(Dlog{k=1}))
```

---

## Naming Taxonomy

### Prefixes (Semantic Categories)

| Prefix | Meaning | Example |
|--------|---------|---------|
| `SHF_` | Shift-based operations | `SHF_WIN_NLN_SDV` |
| `MSK_` | Mask operations | `MSK_WKE`, `MSK_ON` |
| `RSK_ADJ_` | Risk-adjusted (finance) | `RSK_ADJ` (ur) |
| `ALIGN` | Join/alignment operations | `ALIGN`, `ASOF_ALIGN` |
| (none) | Primitives | `DLOG`, `ABS`, `ADD` |

### Middle Components (Operation Type)

| Component | Meaning | Example |
|-----------|---------|---------|
| `WIN` | Window operations | `SHF_WIN_*` |
| `REC` | Recursive operations | `SHF_REC_*` |
| `PFX` | Prefix operations | `SHF_PFX_*` |

### Properties (Mathematical)

| Component | Meaning | Example |
|-----------|---------|---------|
| `LIN` | Linear operation | `SHF_PFX_LIN_SUM` |
| `NLN` | Nonlinear operation | `SHF_WIN_NLN_SDV` |

### Suffixes (Variants)

| Suffix | Meaning | Example |
|--------|---------|---------|
| `_FLD` | Single field/column | `DLOG_FLD` |
| `_FLDS` | Multiple fields/columns | `DLOG_FLDS` |
| `_OMT` | Omit NA (treat as 0) | `AVG_OMT`, `SUM_OMT` |
| `_MIN2` | Minimum 2 observations | `SHF_WIN_MIN2_NLN_SDV` |

---

## Complete Rename Mapping (83 Operations)

### Arithmetic & Comparison (11)
```
*         → MUL
+         → ADD
-         → SUB
/         → DIV
!=        → NEQ
<         → LSS
<=        → LEQ
==        → EQL
>         → GTR
>=        → GEQ
abs       → ABS
```

### Math Functions (2)
```
exp       → EXP
log       → LOG
```

### I/O Operations (5)
```
file      → SRC
file-head → SRC_HED
print     → PRN
save      → SAVE
stdin     → STDIN
```

### Temporal Operations (9)
```
diff         → DIFF
diff-col     → DIFF_FLD
diff-cols    → DIFF_FLDS
dlog         → DLOG
dlog-col     → DLOG_FLD
dlog-cols    → DLOG_FLDS
shift        → SHF
lag          → LAG
shift-col    → SHF_FLD
shift-cols   → SHF_FLDS
```

### Aggregations (6)
```
mean   → AVG
mean0  → AVG_OMT
std    → SDV
std0   → SDV_OMT
sum    → SUM
sum0   → SUM_OMT
```

### Table/Field Operations (10)
```
apply-cols → APL_FLDS
col        → FLD
cols       → FLDS
make-col   → MK_FLD
map-cols   → MAP_FLDS
select     → SEL
select-num → SEL_NUM
setcol     → SET_FLD
w          → GET
withcol    → WTH_FLD
```

### Window Operations (Rolling) (8)
```
wstd       → SHF_WIN_NLN_SDV
wstd-cols  → SHF_WIN_NLN_SDV_FLDS
wstd0      → SHF_WIN_MIN2_NLN_SDV
wstd0-cols → SHF_WIN_MIN2_NLN_SDV_FLDS
wv         → SHF_WIN_NLN_VOL
wv-cols    → SHF_WIN_NLN_VOL_FLDS
wz0        → SHF_WIN_MIN2_NLN_ZSC
wz0-cols   → SHF_WIN_MIN2_NLN_ZSC_FLDS
wzs        → SHF_WIN_NLN_ZSC
```

### Transform Operations (11)
```
chop            → CHOP
cs1             → SHF_PFX_LIN_SUM
cs1-col         → SHF_PFX_LIN_SUM_FLD
cs1-cols        → SHF_PFX_LIN_SUM_FLDS
ecs1            → SHF_REC_EXP_LIN_SUM
ecs1-col        → SHF_REC_EXP_LIN_SUM_FLD
ecs1-cols       → SHF_REC_EXP_LIN_SUM_FLDS
keep-shape      → KEEP_SHAPE
keep-shape-cols → KEEP_SHAPE_FLDS
locf            → SHF_REC_NLN_LOCF
locf-cols       → SHF_REC_NLN_LOCF_FLDS
```

### Mask Operations (8)
```
wkd          → MSK_WKE
mask-weekend → MSK_WKE
mask-define  → MSK_DEF
mask-list    → MSK_LIST
mask-off     → MSK_OFF
mask-on      → MSK_ON
mask-stats   → MSK_STATS
with-mask    → WTH_MSK
```

### Join Operations (2)
```
asofr → ASOF_ALIGN
mapr  → ALIGN
```

### Comparison Operations (2)
```
>-col  → GTR_FLD
>-cols → GTR_FLDS
```

### Finance Operations (4)
```
o       → ORI
ur      → RSK_ADJ
ur-col  → RSK_ADJ_FLD
ur-cols → RSK_ADJ_FLDS
```

### Utility (3)
```
len     → LEN
type-of → TYPE
xminus  → XMINUS
```

---

## Implementation Plan

### Phase 1: IR Layer (Core Enums)
**File:** `src/ir.rs`

Update enum variants:
```rust
// Before
pub enum NumericFunc {
    Dlog, Ret, Log, Exp, Sqrt, Abs, Inv,
    Locf, Wkd, CumSum,
    // ...
}

// After
pub enum NumericFunc {
    DLOG, RET, LOG, EXP, SQRT, ABS, INV,
    SHF_REC_NLN_LOCF, MSK_WKE, SHF_PFX_LIN_SUM,
    // ...
}
```

**Impact:**
- All pattern matches in `exec.rs`, `ir_fusion.rs` must be updated
- Enum Display/Debug implementations
- Serialization if any

---

### Phase 2: Planner Layer (Token Mapping)
**File:** `src/planner.rs`

Update token → IR mapping:
```rust
// Before
"dlog" => NumericFunc::Dlog,
"locf" => NumericFunc::Locf,
"wkd" => NumericFunc::Wkd,

// After
"DLOG" => NumericFunc::DLOG,
"SHF_REC_NLN_LOCF" => NumericFunc::SHF_REC_NLN_LOCF,
"MSK_WKE" => NumericFunc::MSK_WKE,
```

**Decision Point:** Should planner accept BOTH old and new tokens during transition?
- Option A: Clean break (only new tokens)
- Option B: Dual support (old + new tokens)
- Option C: Old tokens via macro expansion ONLY

**Recommendation:** Option C - Forces user-facing code through macro layer.

---

### Phase 3: Builtin Registry (Legacy Evaluator)
**File:** `src/builtins.rs`

Update all `register_builtin()` calls:
```rust
// Before
register_builtin(rt, "dlog", builtin_dlog);
register_builtin(rt, "locf", builtin_locf);
register_builtin(rt, "wkd", builtin_wkd);

// After
register_builtin(rt, "DLOG", builtin_dlog);
register_builtin(rt, "SHF_REC_NLN_LOCF", builtin_locf);
register_builtin(rt, "MSK_WKE", builtin_wkd);
```

**Impact:** 3971 lines, ~100+ operations

---

### Phase 4: Executor (Pattern Matches)
**File:** `src/exec.rs`

Update all IR pattern matches:
```rust
// Before
NumericFunc::Dlog => dlog_kernel(...),
NumericFunc::Locf => locf_kernel(...),
NumericFunc::Wkd => wkd_kernel(...),

// After
NumericFunc::DLOG => dlog_kernel(...),
NumericFunc::SHF_REC_NLN_LOCF => locf_kernel(...),
NumericFunc::MSK_WKE => wkd_kernel(...),
```

**Impact:** 1956 lines, extensive pattern matching

---

### Phase 5: Fusion Optimizer
**File:** `src/ir_fusion.rs`

Update fusion rules:
```rust
// Before
(NumericFunc::Shift{k: 1}, NumericFunc::Dlog) => optimize_shift_dlog(),
(NumericFunc::Locf, NumericFunc::Wkd) => optimize_locf_wkd(),

// After
(NumericFunc::SHF{k: 1}, NumericFunc::DLOG) => optimize_shift_dlog(),
(NumericFunc::SHF_REC_NLN_LOCF, NumericFunc::MSK_WKE) => optimize_locf_wkd(),
```

**Impact:** 662 lines, complex optimization logic

---

### Phase 6: CLI & Documentation
**Files:** `src/main.rs`, `README.md`, `*.md`

Update:
- `--dic` output (dictionary of operations)
- Help text
- Examples in README
- All markdown documentation

---

### Phase 7: Tests
**Files:** `tests/*.rs`, `benches/*.rs`

Update all test cases:
```rust
// Before
assert!(eval("(dlog prices 1)").is_ok());
assert!(eval("(locf data)").is_ok());

// After
assert!(eval("(DLOG prices 1)").is_ok());
assert!(eval("(SHF_REC_NLN_LOCF data)").is_ok());
```

**Impact:** All integration tests must be updated

---

### Phase 8: Macro Aliases (User Ergonomics)
**New file:** `stdlib/aliases.lisp` or built into normalizer

Create macro expansions:
```lisp
; User writes
(defmacro dlog (col k)
  `(DLOG ,col ,k))

(defmacro locf (col)
  `(SHF_REC_NLN_LOCF ,col))

(defmacro wkd (table)
  `(MSK_WKE ,table))

(defmacro wstd (col window)
  `(SHF_WIN_NLN_SDV ,col ,window))
```

**Note:** Requires macro system to be implemented first!

---

## File Impact Analysis

### Critical Files (MUST update)
| File | Lines | Changes | Risk |
|------|-------|---------|------|
| `src/ir.rs` | 763 | Enum variants | HIGH - breaks all pattern matches |
| `src/planner.rs` | 1005 | Token mappings | HIGH - breaks parser |
| `src/builtins.rs` | 3971 | Registry calls | MEDIUM - isolated changes |
| `src/exec.rs` | 1956 | Pattern matches | HIGH - complex logic |
| `src/ir_fusion.rs` | 662 | Optimization rules | MEDIUM - complex but localized |

### Secondary Files (Should update)
| File | Lines | Changes | Risk |
|------|-------|---------|------|
| `src/main.rs` | 609 | CLI output | LOW |
| `src/normalize.rs` | 351 | Token handling | MEDIUM |
| `src/mask.rs` | 356 | Mask ops | LOW |
| `tests/*.rs` | Various | Test cases | LOW |
| `benches/*.rs` | Various | Benchmarks | LOW |

### Documentation Files
- `README.md`
- `CURRENT_OPERATION_TAXONOMY.md`
- `blisp_dev_readme.md`
- All other `*.md` files

---

## Testing Strategy

### 1. Compilation Test
```bash
cd /home/ubuntu/blisp
cargo build 2>&1 | tee build_errors.txt
```

**Expected:** All pattern match errors identified

---

### 2. Unit Tests
```bash
cargo test 2>&1 | tee test_results.txt
```

**Target:** All existing tests pass with new names

---

### 3. Integration Tests
Test GLD_NUM pipeline with new canonical names:
```lisp
(SHF_FLDS
  (GTR_FLDS
    (SHF_WIN_NLN_ZSC
      (SHF_PFX_LIN_SUM_FLDS
        (XMINUS
          (DLOG_FLDS (MSK_WKE (SRC "At.csv")) 1)
          1))
      25)
    -1.0)
  2)
```

---

### 4. Grep Validation (NEW!)
After rename, validate taxonomic properties:

```bash
# Find all shift operations
grep -r "SHF_" src/*.rs | wc -l

# Find all nonlinear operations
grep -r "NLN_" src/*.rs | wc -l

# Find all window operations
grep -r "WIN_" src/*.rs | wc -l

# Find all linear operations
grep -r "LIN_" src/*.rs | wc -l

# Find all mask operations
grep -r "MSK_" src/*.rs | wc -l
```

---

## Rollback Plan

### 1. Git Branch Strategy
```bash
# Create rename branch
git checkout -b canonical-rename

# Work in progress
git add -A
git commit -m "WIP: canonical rename phase N"

# If rollback needed
git checkout reconstruct/tableview-only
git branch -D canonical-rename
```

### 2. Backup Critical Files
Before starting:
```bash
cd /home/ubuntu/blisp
cp src/ir.rs src/ir.rs.backup_before_rename
cp src/planner.rs src/planner.rs.backup_before_rename
cp src/builtins.rs src/builtins.rs.backup_before_rename
cp src/exec.rs src/exec.rs.backup_before_rename
cp src/ir_fusion.rs src/ir_fusion.rs.backup_before_rename
```

### 3. Incremental Commits
- Commit after each phase
- Tag successful compilation points
- Easy to bisect if issues arise

---

## Risk Assessment

### HIGH RISK
- **Breaking all pattern matches** - One typo breaks entire executor
- **Token parser changes** - Breaks all user scripts
- **Fusion optimizer** - Complex optimization rules, hard to verify

### MEDIUM RISK
- **Builtin registry** - Many changes but mechanical
- **Test updates** - Time consuming but straightforward

### LOW RISK
- **Documentation** - No code impact
- **CLI output** - Isolated changes

---

## Estimated Effort

| Phase | Description | Estimated Time | Difficulty |
|-------|-------------|---------------|------------|
| 1 | IR enum variants | 30 min | Medium |
| 2 | Planner tokens | 30 min | Medium |
| 3 | Builtin registry | 1 hour | Low (mechanical) |
| 4 | Executor patterns | 1 hour | High (careful) |
| 5 | Fusion optimizer | 30 min | High (careful) |
| 6 | CLI & docs | 30 min | Low |
| 7 | Tests | 1 hour | Low (mechanical) |
| 8 | Macro aliases | 2 hours | Medium (requires macro system) |
| **TOTAL** | **~7 hours** | **Medium-High** |

**Note:** Phase 8 (macros) may require implementing macro system first if not yet available.

---

## Success Criteria

### ✅ Compilation
- `cargo build` succeeds with no errors
- No warnings about unused enum variants

### ✅ Tests
- All existing tests pass
- `cargo test` shows 100% pass rate

### ✅ GLD_NUM Pipeline
- Complete pipeline runs successfully
- Output matches previous version exactly
- Performance unchanged

### ✅ Grepability
- Can find all operations by category (SHF_, MSK_, etc.)
- Can identify linear vs nonlinear operations
- Can discover operation properties

### ✅ Documentation
- All docs updated with new names
- Examples work correctly
- `--dic` output shows canonical names

### ✅ Future: Macro Aliases
- User scripts work with old names
- Macros expand to canonical names
- No breaking change for users

---

## Post-Rename Analysis Queries

After successful rename, these queries become possible:

### Find All Operations by Category
```bash
# Shift operations
grep -r "SHF_" src/*.rs | cut -d: -f1 | sort -u

# Mask operations
grep -r "MSK_" src/*.rs | cut -d: -f1 | sort -u

# Risk-adjusted operations
grep -r "RSK_ADJ" src/*.rs | cut -d: -f1 | sort -u
```

### Find Operations by Property
```bash
# Linear operations (composable)
grep -r "LIN_" src/*.rs

# Nonlinear operations (non-composable)
grep -r "NLN_" src/*.rs
```

### Find Operations by Type
```bash
# Window operations
grep -r "WIN_" src/*.rs

# Recursive operations
grep -r "REC_" src/*.rs

# Prefix operations (cumulative)
grep -r "PFX_" src/*.rs
```

### Count Operations by Category
```bash
grep -r "enum NumericFunc" -A 100 src/ir.rs | grep "SHF_" | wc -l
grep -r "enum NumericFunc" -A 100 src/ir.rs | grep "MSK_" | wc -l
```

---

## Decision Points

### ❓ Should we do this in one commit or multiple?
**Recommendation:** Single atomic commit for IR+Planner+Builtins+Executor, separate commits for docs/tests.
- Avoids broken intermediate states
- Makes rollback simpler
- All renames happen together

### ❓ Should we keep old token aliases temporarily?
**Recommendation:** NO - Force clean break, add aliases via macros later.
- Prevents confusion about "correct" name
- Makes grep results clean
- Macro layer provides compatibility

### ❓ Should we update user-facing examples immediately?
**Recommendation:** YES - Show canonical names in docs, note that aliases coming via macros.
- Sets expectation for canonical form
- Demonstrates grepability benefits
- Users understand the system

---

## Examples: Before & After

### Simple Pipeline
```lisp
# Before (current)
(dlog prices 1)

# After (canonical)
(DLOG prices 1)

# Future (with macro alias)
(dlog prices 1)  ; expands to (DLOG prices 1)
```

### Complex Pipeline
```lisp
# Before
(wstd (locf (dlog prices 1)) 20)

# After
(SHF_WIN_NLN_SDV (SHF_REC_NLN_LOCF (DLOG prices 1)) 20)

# Future (with macros)
(wstd (locf (dlog prices 1)) 20)  ; expands to canonical
```

### GLD_NUM Pipeline
```lisp
# Before
(shift-cols
  (>-cols
    (wzs-ft-cols
      (cs1-cols
        (xminus
          (dlog-cols (wkd (file "At.csv")) 1)
          1))
      25)
    -1.0)
  2)

# After
(SHF_FLDS
  (GTR_FLDS
    (SHF_WIN_NLN_ZSC
      (SHF_PFX_LIN_SUM_FLDS
        (XMINUS
          (DLOG_FLDS (MSK_WKE (SRC "At.csv")) 1)
          1))
      25)
    -1.0)
  2)

# Future (with macros)
(shift-cols ...)  ; same as before, expands to canonical
```

---

## Name Decoding Examples

Understanding the canonical names:

### `SHF_WIN_NLN_SDV`
- `SHF_` = Shift-based operation
- `WIN_` = Window operation
- `NLN_` = Nonlinear
- `SDV` = Standard Deviation
- **Meaning:** Rolling window standard deviation (wstd)

### `SHF_REC_NLN_LOCF`
- `SHF_` = Shift-based operation
- `REC_` = Recursive operation
- `NLN_` = Nonlinear
- `LOCF` = Last Observation Carried Forward
- **Meaning:** Recursive forward fill (locf)

### `SHF_PFX_LIN_SUM`
- `SHF_` = Shift-based operation
- `PFX_` = Prefix operation
- `LIN_` = Linear
- `SUM` = Sum
- **Meaning:** Cumulative sum (cs1)

### `MSK_WKE`
- `MSK_` = Mask operation
- `WKE` = Weekend
- **Meaning:** Weekend mask (wkd)

### `RSK_ADJ`
- `RSK_ADJ_` = Risk-adjusted
- **Meaning:** Rolling univariate regression / beta (ur)

---

## Open Questions

1. **Macro System Status**: Is `defmacro` implemented? If not, Phase 8 blocked.
2. **Backward Compatibility Window**: Should we support old names for N releases?
3. **Documentation Strategy**: Show both names or canonical only?
4. **External Dependencies**: Does blawktrust need updates?
5. **Performance Impact**: Will longer names affect compilation time?

---

## Next Steps (After Approval)

1. ✅ Create git branch `canonical-rename`
2. ✅ Backup critical files
3. ✅ Start with Phase 1 (IR layer)
4. ✅ Compile after each phase
5. ✅ Commit incrementally
6. ✅ Run full test suite
7. ✅ Update documentation
8. ✅ Validate grepability
9. ⏸️  Wait for macro system before Phase 8

---

## Sign-off

**Prepared by:** Claude Sonnet 4.5
**Date:** 2026-02-26
**Status:** AWAITING APPROVAL - DO NOT EXECUTE
**Estimated Total Effort:** ~7 hours
**Risk Level:** Medium-High (breaking changes across codebase)

**Recommendation:** Proceed with rename, defer macro aliases until macro system complete.

---

**END OF PLAN**
