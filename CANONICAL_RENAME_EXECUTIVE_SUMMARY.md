# BLISP Canonical Rename - Executive Summary

**Date:** 2026-02-26
**Status:** Planning Complete - Awaiting Approval
**Prepared by:** Claude Sonnet 4.5

---

## Overview

This is a comprehensive renaming of all 83 BLISP operations to align with a formal taxonomic framework that encodes mathematical properties in operation names.

---

## The Framework

### Structure: `<INV>_<SUPP>_<ALG>_<OP>`

**Four orthogonal axes:**

1. **INV** (Invariance): `SHF` (shift-equivariant), `NON` (no symmetry)
2. **SUPP** (Support): `PTW` (pointwise), `WIN` (window), `PFX` (prefix), `REC` (recursive), `GLO` (global)
3. **ALG** (Algebraic): `LIN` (linear), `NLN` (nonlinear), `EXP` (exponential)
4. **OP** (Operation): `DLOG`, `SDV`, `SUM`, `LOCF`, etc.

**Example:**
```
SHF_WIN_NLN_SDV = Shift-equivariant + Window + Nonlinear + Standard Deviation
                = Rolling standard deviation (wstd)
```

---

## What Changes

### Three-Tier Naming System

**Tier 1: Full Framework (27 operations)**
- Window operations: `SHF_WIN_NLN_SDV` (was `wstd`)
- Prefix operations: `SHF_PFX_LIN_SUM` (was `cs1`)
- Recursive operations: `SHF_REC_NLN_LOCF` (was `locf`)
- **NEW:** Pointwise operations: `SHF_PTW_NLN_DLOG` (was `DLOG`)

**Tier 2: Category-Prefixed (35 operations)**
- Mask operations: `MSK_WKE`, `MSK_ON`
- I/O operations: `SRC`, `PRN`, `SAVE`
- Table operations: `FLD`, `FLDS`, `SEL`
- Join operations: `ALIGN`, `ASOF_ALIGN`

**Tier 3: Simple Names (21 operations)**
- Arithmetic: `ADD`, `SUB`, `MUL`, `DIV`
- Comparison: `GTR`, `LSS`, `EQL`
- Math: `ABS`, `EXP`, `LOG`
- Aggregations: `AVG`, `SDV`, `SUM`

---

## Specific Changes

### 10 Operations Modified (12% of total)

**All pointwise temporal operations:**

```
dlog       → SHF_PTW_NLN_DLOG       (differenced log)
dlog-col   → SHF_PTW_NLN_DLOG_FLD
dlog-cols  → SHF_PTW_NLN_DLOG_FLDS

diff       → SHF_PTW_LIN_DIFF       (first difference)
diff-col   → SHF_PTW_LIN_DIFF_FLD
diff-cols  → SHF_PTW_LIN_DIFF_FLDS

shift      → SHF_PTW_LIN_SHF        (temporal shift)
shift-col  → SHF_PTW_LIN_SHF_FLD
shift-cols → SHF_PTW_LIN_SHF_FLDS

xminus     → SHF_PTW_LIN_SPR        (cross-sectional spread)
```

### 73 Operations Unchanged (88% of total)

All other operations remain as specified in `CANONICAL_RENAME.csv`.

---

## Why This Matters

### Before (No Taxonomy)
```bash
# How do I find all nonlinear operations?
# → No way to know without reading documentation

# How do I find all window operations?
# → Search for "rolling"? "w*"? Inconsistent naming

# Is cumulative sum linear?
# → Have to check the math or run tests
```

### After (With Taxonomy)
```bash
# Find all nonlinear operations
grep "NLN_" src/*.rs

# Find all window operations
grep "WIN_" src/*.rs

# Is cumulative sum linear?
grep "SHF_PFX" src/ir.rs | grep "LIN"
# → Yes! SHF_PFX_LIN_SUM
```

---

## Benefits

### 1. Grepability
Find operations by mathematical property:
```bash
grep "SHF_" src/*.rs      # All shift-equivariant operations
grep "PTW_" src/*.rs      # All pointwise operations
grep "WIN_" src/*.rs      # All window operations
grep "LIN_" src/*.rs      # All linear operations
grep "NLN_" src/*.rs      # All nonlinear operations
```

### 2. Self-Documentation
```
SHF_WIN_NLN_SDV tells you:
- SHF_ → Shift-equivariant (translates in time)
- WIN_ → Window operation (rolling/moving window)
- NLN_ → Nonlinear (doesn't distribute over addition)
- SDV  → Standard deviation
```

### 3. Invariance Discovery
```
# Linear operations compose linearly
(SHF_PFX_LIN_SUM (ADD x y)) ≡ (ADD (SHF_PFX_LIN_SUM x) (SHF_PFX_LIN_SUM y))

# Nonlinear operations don't
(SHF_WIN_NLN_SDV (ADD x y)) ≠ (ADD (SHF_WIN_NLN_SDV x) (SHF_WIN_NLN_SDV y))
```

### 4. Future Extensions
```
# Want to add a new window operation?
# Follow the pattern:
SHF_WIN_<ALG>_<OP>

# Want to add a new pointwise operation?
SHF_PTW_<ALG>_<OP>
```

---

## User Impact

### During Transition
**User code breaks:** Yes, all operations renamed
```lisp
;; Old (breaks)
(dlog prices 1)
(wstd returns 20)

;; New (required)
(SHF_PTW_NLN_DLOG prices 1)
(SHF_WIN_NLN_SDV returns 20)
```

### After Macro System (Future)
**User code unchanged:** Macros provide ergonomic aliases
```lisp
;; User writes (familiar syntax)
(dlog prices 1)
(wstd returns 20)

;; Expands to (canonical)
(SHF_PTW_NLN_DLOG prices 1)
(SHF_WIN_NLN_SDV returns 20)
```

**Two-layer system:**
- Internal: Canonical names (taxonomic, grepable)
- User-facing: Familiar names (ergonomic, convenient)

---

## Implementation Plan

### Phase 1: IR Layer (30 min)
Update `src/ir.rs` enum variants:
```rust
// Before
pub enum NumericFunc {
    Dlog, Diff, Shift, // ...
}

// After
pub enum NumericFunc {
    SHF_PTW_NLN_DLOG, SHF_PTW_LIN_DIFF, SHF_PTW_LIN_SHF, // ...
}
```

### Phase 2: Planner (30 min)
Update `src/planner.rs` token mappings:
```rust
// Before
"dlog" => NumericFunc::Dlog,

// After
"dlog" => NumericFunc::SHF_PTW_NLN_DLOG,
```

### Phase 3: Builtins (1 hour)
Update `src/builtins.rs` registration (83 operations)

### Phase 4: Executor (1 hour)
Update `src/exec.rs` pattern matches

### Phase 5: Fusion (30 min)
Update `src/ir_fusion.rs` optimization rules

### Phase 6: Docs (30 min)
Update all markdown files with new names

### Phase 7: Tests (1 hour)
Update all test cases

### Phase 8: Macros (Future)
Implement macro system for user-facing aliases

**Total Time:** ~5-7 hours (excluding macro system)

---

## Risk Assessment

### HIGH RISK
- **Breaking all pattern matches** - Compilation will fail if any enum variant missed
- **Token parser changes** - All planner tokens must be updated consistently

### MEDIUM RISK
- **Test updates** - Time consuming but mechanical
- **Documentation** - Many files to update

### LOW RISK
- **Rollback plan** - Git branch with backups, easy to revert

### MITIGATION
- ✅ Single atomic commit for core changes
- ✅ Compile after each phase
- ✅ Backup critical files before starting
- ✅ Incremental testing

---

## Success Criteria

### ✅ Compilation
```bash
cargo build --release
# → No errors, no warnings about unused variants
```

### ✅ Tests Pass
```bash
cargo test
# → 100% pass rate
```

### ✅ GLD_NUM Pipeline Works
```lisp
(SHF_FLDS
  (GTR_FLDS
    (SHF_WIN_NLN_ZSC
      (SHF_PFX_LIN_SUM_FLDS
        (SHF_PTW_LIN_SPR
          (SHF_PTW_NLN_DLOG_FLDS (MSK_WKE (SRC "At.csv")) 1)
          1))
      25)
    -1.0)
  2)
# → Same output as before
```

### ✅ Grepability
```bash
grep "SHF_PTW" src/ir.rs | wc -l
# → Should find 10 pointwise operations

grep "SHF_WIN" src/ir.rs | wc -l
# → Should find 9 window operations

grep "LIN_" src/ir.rs | wc -l
# → Should find linear operations
```

---

## Documentation Delivered

### ✅ Created Files

1. **CANONICAL_RENAME_PLAN.md** (Original plan, 83 operations)
2. **CANONICAL_TAXONOMY.md** (Taxonomic analysis)
3. **CANONICAL_RENAME_QUICKREF.md** (Quick lookup table)
4. **BLISP_Canonical_Framework_Documentation.md** (Formal framework - pre-existing)
5. **CANONICAL_FRAMEWORK_INTEGRATED.md** (Framework integration analysis)
6. **CANONICAL_RENAME_CORRECTED.csv** (Corrected CSV with 10 changes)
7. **CANONICAL_RENAME_DIFF.md** (Change documentation)
8. **CANONICAL_RENAME_EXECUTIVE_SUMMARY.md** (This document)

---

## Decision Points

### ❓ Option A: Use Original CSV (DLOG, XMINUS, etc.)
- **Pros:** Less typing, simpler names
- **Cons:** Inconsistent framework, harder to grep for properties

### ❓ Option B: Use Corrected CSV (SHF_PTW_NLN_DLOG, SHF_PTW_LIN_SPR)
- **Pros:** Consistent framework, full grepability, self-documenting
- **Cons:** 10 more changes, longer names

### ❓ Option C: Hybrid (Framework for complex ops, simple for primitives)
- **Pros:** Balanced approach, framework where it matters
- **Cons:** Less systematic

---

## Recommendation

### ✅ Proceed with Option B (Corrected CSV)

**Rationale:**
1. **Consistency** - All temporal operations follow same pattern
2. **Completeness** - Framework fully realized for its intended scope
3. **Future-proof** - Easy to add new pointwise operations
4. **Minimal overhead** - Only 10 additional changes (already planned for 73)

**With macro system (future):**
- Users never see the canonical names
- Write `(dlog prices 1)`, get `(SHF_PTW_NLN_DLOG prices 1)` internally
- Best of both worlds: ergonomics + taxonomy

---

## Next Steps

### 1. Approve Approach ✋
- [ ] Confirm: Use corrected CSV (10 operation changes)
- [ ] Confirm: Three-tier naming system
- [ ] Confirm: Defer macro system until framework complete

### 2. Create Git Branch 🌿
```bash
cd /home/ubuntu/blisp
git checkout -b canonical-rename
```

### 3. Backup Files 💾
```bash
cp src/ir.rs src/ir.rs.backup_before_canonical
cp src/planner.rs src/planner.rs.backup_before_canonical
cp src/builtins.rs src/builtins.rs.backup_before_canonical
cp src/exec.rs src/exec.rs.backup_before_canonical
```

### 4. Execute Phases 1-7 🚀
- Phase 1: IR layer (enums)
- Phase 2: Planner (tokens)
- Phase 3: Builtins (registration)
- Phase 4: Executor (pattern matches)
- Phase 5: Fusion (optimization)
- Phase 6: Documentation (markdown)
- Phase 7: Tests (test cases)

### 5. Validate ✅
- Compilation check
- Test suite
- GLD_NUM pipeline
- Grep validation

### 6. Commit 📝
```bash
git add -A
git commit -m "Implement canonical naming framework

Breaking change: Rename 83 operations to taxonomic canonical names.

Taxonomy: <INV>_<SUPP>_<ALG>_<OP>
- INV: Invariance (SHF, NON)
- SUPP: Support geometry (PTW, WIN, PFX, REC, GLO)
- ALG: Algebraic structure (LIN, NLN, EXP)
- OP: Semantic token (DLOG, SDV, SUM, etc.)

Changed operations (10):
- dlog → SHF_PTW_NLN_DLOG
- diff → SHF_PTW_LIN_DIFF
- shift → SHF_PTW_LIN_SHF
- xminus → SHF_PTW_LIN_SPR
(+ field variants)

Framework-compliant operations (27):
- Window: SHF_WIN_*
- Prefix: SHF_PFX_*
- Recursive: SHF_REC_*
- Pointwise: SHF_PTW_*

Category-prefixed operations (35):
- MSK_, SRC_, FLD/FLDS, ALIGN, etc.

Simple names (21):
- ADD, SUB, MUL, DIV, GTR, LEN, etc.

Benefits:
- Grepable by mathematical property
- Self-documenting names
- Invariance analysis enabled
- Systematic extension path

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Estimated Timeline

| Phase | Duration | Cumulative |
|-------|----------|------------|
| Approval | 10 min | 0:10 |
| Setup (branch, backups) | 10 min | 0:20 |
| Phase 1 (IR) | 30 min | 0:50 |
| Phase 2 (Planner) | 30 min | 1:20 |
| Phase 3 (Builtins) | 60 min | 2:20 |
| Phase 4 (Executor) | 60 min | 3:20 |
| Phase 5 (Fusion) | 30 min | 3:50 |
| Phase 6 (Docs) | 30 min | 4:20 |
| Phase 7 (Tests) | 60 min | 5:20 |
| Validation | 30 min | 5:50 |
| Commit | 10 min | 6:00 |
| **TOTAL** | **~6 hours** | |

---

## Final Checklist

### Before Starting
- [ ] Read all planning documents
- [ ] Understand the framework (`<INV>_<SUPP>_<ALG>_<OP>`)
- [ ] Review corrected CSV changes (10 operations)
- [ ] Create git branch
- [ ] Backup critical files

### During Implementation
- [ ] Compile after each phase
- [ ] Check pattern match coverage
- [ ] Test incrementally
- [ ] Document issues encountered

### After Completion
- [ ] Full compilation successful
- [ ] All tests pass
- [ ] GLD_NUM pipeline works
- [ ] Grep queries return expected results
- [ ] Documentation updated
- [ ] Commit with detailed message

### Future Work
- [ ] Implement macro system (Phase 8)
- [ ] Add user-facing aliases
- [ ] Update external documentation
- [ ] Announce breaking change

---

## Questions?

1. **Will old code work?** No - this is a breaking change. All operation names change.
2. **Can we keep aliases?** Yes, via macro system (future Phase 8).
3. **Is this reversible?** Yes - git branch with backups, easy rollback.
4. **Performance impact?** None - names don't affect runtime, only compile-time.
5. **When do users see benefits?** Immediately for developers, after macro system for end users.

---

## Approval Required

**Ready to proceed:** YES ✅

All planning documents complete. Corrected CSV ready. Implementation plan detailed. Risk mitigated.

**Awaiting approval to:**
- Use corrected CSV (10 operation changes)
- Proceed with three-tier naming system
- Execute phases 1-7 (6 hours estimated)

---

**Status:** PLANNING COMPLETE - READY FOR EXECUTION
**Next:** Await approval, then execute implementation plan

---

**END OF EXECUTIVE SUMMARY**
