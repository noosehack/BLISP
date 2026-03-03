# Phase C Implementation: Hybrid CURRENT/PLANNED Split

**Date:** 2026-03-03
**Status:** Phase 1 Complete ✅ | Phase 2 In Progress
**Policy:** Option C (Hybrid) with hard rule

---

## Hard Rule Enforced

**OPS_CURRENT.yml must describe ONLY what resolves today.**
Aspirations go to OPS_PLANNED.yml (separate file, not consumed by default).

---

## Phase 1: Split CURRENT/PLANNED ✅ COMPLETE

### Created Files:

**OPS_CURRENT.yml (31 entries, 41 aliases)**
- Contains only operations where ALL aliases resolve
- `blisp dic` reads this by default
- **Tripwire enforced:** `--check-resolve` must show 0 FAIL

**OPS_PLANNED.yml (40 entries, 53 aliases)**
- Contains operations with unresolved aliases
- Roadmap only, not guaranteed resolvable
- `blisp dic --planned` shows this separately
- **Tripwire:** Planned ops MAY have failures (informational)

### Validation Results:

**CURRENT ops:**
```
✅ All names resolve successfully
Total checked: 41
0 FAIL, 41 OK (100% success rate)
```

**PLANNED ops:**
```
⚠️  Roadmap only - not expected to resolve yet
Total: 53 names across 40 entries
```

### Code Changes:

**src/dic.rs:**
- Split `CANONICAL_MAP_YAML` → `CURRENT_OPS_YAML` + `PLANNED_OPS_YAML`
- Added `load_current_ops()` and `load_planned_ops()`
- Added `View::Planned` enum variant
- Validation skipped for PLANNED (allowed to fail)
- Clear headers showing "PLANNED (Roadmap Only)"

**src/main.rs:**
- Added `--planned` flag
- Help text updated
- Default behavior unchanged (reads CURRENT)

**Test status:** 159 tests passing, 0 failures

---

## Phase 2: Register Cheap Aliases (IN PROGRESS)

### Target: Reduce PLANNED from 53 → ~20-25 names

Follow baby-step ordering for fast wins:

### Bucket C1.1: Operator Word Forms (12 aliases) - NEXT

**Names to register:**
- Arithmetic: `add`, `sub`, `mul`, `div`
- Comparison: `eq`, `neq`, `gt`, `lt`, `lte`, `gte`

**Implementation:**
```rust
// In src/builtins.rs, register_builtins()
rt.register_builtin("add", builtin_add);  // Same as +
rt.register_builtin("sub", builtin_sub);  // Same as -
rt.register_builtin("mul", builtin_mul);  // Same as *
rt.register_builtin("div", builtin_div);  // Same as /
rt.register_builtin("eq", builtin_eq);    // Same as ==
rt.register_builtin("neq", builtin_neq);  // Same as !=
rt.register_builtin("gt", builtin_gt);    // Same as >
rt.register_builtin("lt", builtin_lt);    // Same as <
rt.register_builtin("lte", builtin_lte);  // Same as <=
rt.register_builtin("gte", builtin_gte);  // Same as >=
```

**Why now:** Immediate user ergonomics, near-zero risk, pure aliases.

**After registration:**
1. Verify with `blisp dic --check-resolve`
2. Move entries from PLANNED → CURRENT
3. Rerun tests

### Bucket C1.2: Math Functions (7 aliases) - HIGH PRIORITY

**Names to register:**
- `abs`, `exp`, `inv`, `sqrt` (straightforward)
- `log`, `ln` (must decide: identical or separate?)
- `ret` (must verify: is it return = dlog?)

**Implementation strategy:**
- These IR enums exist but lack builtin registration
- Need wrappers that dispatch to IR planner
- Check semantics carefully (especially `ret`)

**Decision needed:**
- `ln` vs `log`: Identical? Or ln=natural, log=base10?
- `ret`: Is this dlog (log returns)? If ambiguous, defer to PLANNED.

### Bucket C1.3: Friendly Shift Family (8 aliases) - MEDIUM PRIORITY

**Names to register (if unambiguous):**
- `dlog` → must decide: obs or ofs? Or table version?
- `cs1`, `cumsum` → dispatch to cs1-cols (table version)
- `shift` → must decide: col or cols default?
- `lag`, `lag-obs` → check semantics
- `locf`, `ffill` → dispatch to locf-cols (table version)

**Warning:** Only register if semantics are unambiguous.
- If unclear whether col/cols, keep in PLANNED.
- If unclear between OBS/OFS variants, keep in PLANNED.

---

## Phase 3: Hold Complex Names as PLANNED

### Bucket C2.1: Rolling Friendly Names (14 aliases) - DEFER

**Keep in PLANNED:**
- `rolling-mean`, `rolling-std`, `roll-mean`, `roll-std`
- `rolling-mean-min`, `rolling-mean-excl`
- `rolling-std-min`, `rolling-std-excl`
- `roll-z`, `rolling-zscore`

**Reason:** These touch window parameters (min2, excl) and require:
- Default window parameter policy
- Min-periods behavior locked
- Naming consistency with canonical split (MIN2, EXCL)

**Decision:** Don't expose until semantics are stable.

### Bucket C2.2: Structural/Ambiguous (12 aliases) - DEFER

**Keep in PLANNED:**
- `demean`, `left-join`, `orient`, `nrow`, `keep`
- `load`, `read-csv`, `length`
- `sharpe`, `weekday`, `write-csv`

**Reason:** Policy decisions needed:
- What is `load` vs `file` vs `read-csv`?
- Is `orient` same as `o`?
- Is `length` same as `len`?
- Is `nrow` a table primitive?

**Decision:** Keep planned unless stable semantics exist.

---

## Acceptance Criteria

### Target 1: CURRENT Operations (Immediate) ✅

**Status:** ACHIEVED
- `blisp dic --check-resolve` → 0 FAIL on CURRENT ✅
- OPS_CURRENT.yml contains only resolvable names ✅

### Target 2: Roadmap Preserved ✅

**Status:** ACHIEVED
- `blisp dic --planned` shows aspirational aliases ✅
- Clear headers: "PLANNED (Roadmap Only)" ✅
- `blisp dic --planned --check-resolve` can show FAILs (informational) ✅

### Target 3: Register Cheap Aliases (In Progress)

**Status:** TODO
- Register 12 operator word forms → Move to CURRENT
- Register 7 math functions → Move to CURRENT (if unambiguous)
- Register 8 friendly shift family → Move to CURRENT (if unambiguous)
- **Goal:** Reduce PLANNED to ~20-25 complex operations

**Expected outcome:** ~27 names promoted from PLANNED → CURRENT

---

## Migration Process (Controlled Promotion)

When an operation becomes resolvable:

1. Register in `src/builtins.rs`
2. Verify: `cargo build && ./target/release/blisp dic --planned --check-resolve`
3. Check alias shows `OK(BUILTIN)` or `OK(IR:...)`
4. Move entry from OPS_PLANNED.yml → OPS_CURRENT.yml
5. Rebuild: `cargo build --release`
6. Verify CURRENT still clean: `./target/release/blisp dic --check-resolve`
7. Run tests: `cargo test`
8. Commit: "dic: promote <name> from PLANNED to CURRENT"

This ensures controlled, auditable promotion of operations.

---

## Summary

**Phase 1:** ✅ Complete (CURRENT/PLANNED split)
- CURRENT: 41 aliases, 0 FAIL (100%)
- PLANNED: 53 aliases (roadmap)

**Phase 2:** In Progress (Register cheap aliases)
- Next: Operator word forms (12)
- Then: Math functions (7)
- Then: Friendly shift family (8)

**Phase 3:** Deferred (Complex operations stay PLANNED)
- Rolling: 14 aliases (semantics not locked)
- Structural: 12 aliases (policy decisions needed)

**Tests:** 159 passing, 0 failures
**Tripwire:** Enforced on CURRENT, informational on PLANNED

**Next action:** Register operator word forms (Bucket C1.1)
