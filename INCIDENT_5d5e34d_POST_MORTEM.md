# Post-Mortem: Commit 5d5e34d Registration Removal Incident

**Date**: 2026-02-26 (incident) → 2026-02-27 (resolution)
**Status**: RESOLVED
**Severity**: High (20 operations unreachable, GLD_NUM broken)

---

## Incident Summary

Commit `5d5e34d` (PR1: Remove 20 builtin registrations shadowing IR mappings) removed 20 builtin registrations under the assumption that planner/IR mappings existed. They did not. Operations became unreachable for 1 day until discovered.

---

## Timeline

| Time | Event |
|------|-------|
| 2026-02-26 16:39 | Commit 5d5e34d removes 20 registrations (84 → 64) |
| 2026-02-26 16:39 | **No planner mappings added** (assumption failure) |
| 2026-02-27 | User attempts GLD_NUM test: `Error: Undefined variable: w5` |
| 2026-02-27 | Forensic analysis: functions exist but not registered |
| 2026-02-27 | Fix: e5a33de restores wkd/w5/xminus registrations |
| 2026-02-27 | Fix: 91dbb4b adds mapr registration + date parsing |

---

## Root Cause Analysis

### Commit Message vs Reality

**Commit 5d5e34d claimed**:
> "All removed tokens now route through planner.rs → IR → exec.rs."

**Actual state**:
- ❌ Removed 20 builtin registrations
- ❌ **No planner mappings added**
- ❌ **No IR mappings added**
- 💥 Result: Operations disappeared

### Operations Affected

```
Core ops:     dlog, shift, locf, wkd, cs1
Join ops:     mapr, asofr, ur
Schema ops:   xminus, mask-weekend, with-mask
Arithmetic:   +, -, *, /, >
Math:         log, exp, abs
I/O:          stdin
```

### Why It Happened

1. **Incomplete refactoring**: Removed old path, never added new path
2. **Missing gate condition**: No check that planner mappings exist before removal
3. **No reachability test**: CI didn't verify all public heads resolve
4. **Contract violation**: Commit message asserted completion that didn't exist

---

## Impact

### User-Visible
- **GLD_NUM golden test**: Completely broken
- **Production scripts**: Any using w5, xminus, mapr, etc. failed
- **Error message**: `Error: Undefined variable: <op>` (confusing - functions existed!)

### Developer-Visible
- Functions remained in code (`builtin_wkd`, etc.) but unreachable
- 84 → 64 registrations without corresponding planner additions
- IR fusion work proceeded unaware of broken operations

---

## Resolution

### Emergency Fix (2026-02-27)

**Commit e5a33de**: Restored registrations for wkd, w5, xminus
```rust
rt.register_builtin("wkd", builtin_wkd);
rt.register_builtin("w5", builtin_wkd);           // Alias
rt.register_builtin("xminus", builtin_xminus);
```

**Commit 91dbb4b**:
- Added mapr registration
- Fixed wkd to handle Frame's Date index (not just TableView columns)

### Why This Was The Right Fix

Direct builtin path is **working and tested**. Restoring registrations:
- ✅ Immediate resolution (no new code needed)
- ✅ Preserves user scripts (backward compatible)
- ✅ Maintains test coverage (operations already tested)
- ✅ Defers IR refactor until properly gated

---

## Prevention: Non-Negotiable Invariants

### 1. Reachability Invariant

**Rule**: Every public lispy head MUST resolve through at least one path:
- IR planner mapping, OR
- Builtin registration

**Enforcement**: CI test (see Test T1 below)

### 2. No Silent Removals

**Rule**: Removing a builtin registration REQUIRES in the same commit:
- ✅ Added planner mapping
- ✅ Test proving equivalence: `direct_eval(expr) ≡ execute(plan(expr))`
- ✅ Test proving reachability

**Enforcement**: CI test (see Test T2, T3 below)

### 3. Single Source of Truth Registry

**Proposal**: Create `src/ops_registry.rs`:

```rust
pub struct OpSpec {
    pub head: &'static str,
    pub eval_paths: EvalPaths,  // IR, Builtin, or Both
    pub canonical: Option<&'static str>,  // If IR
    pub builtin_fn: Option<fn>,  // If Builtin
    pub aliases: &'static [&'static str],
    pub deprecated: bool,
}

pub enum EvalPaths {
    IROnly,
    BuiltinOnly,
    Both,  // During transition
}

pub const OPS_REGISTRY: &[OpSpec] = &[
    OpSpec {
        head: "wkd",
        eval_paths: EvalPaths::BuiltinOnly,  // TODO: migrate to IR
        canonical: Some("SHF_PTW_WKD_MASK"),
        builtin_fn: Some(builtin_wkd),
        aliases: &["w5"],
        deprecated: false,
    },
    // ... all other ops
];
```

**Benefits**:
- Declarative specification of all operations
- Generate both planner mappings AND builtin registrations from registry
- Single place to check "does this head resolve?"
- Explicit migration state (BuiltinOnly → Both → IROnly)

---

## Required CI Tests

### Test T1: "All public heads resolve"

```rust
#[test]
fn test_all_heads_reachable() {
    for spec in OPS_REGISTRY {
        let expr = format!("({} dummy)", spec.head);

        match spec.eval_paths {
            EvalPaths::IROnly => {
                assert!(planner_resolves(&expr),
                    "Head '{}' declared IROnly but planner doesn't resolve it",
                    spec.head);
            }
            EvalPaths::BuiltinOnly => {
                assert!(builtin_exists(spec.head),
                    "Head '{}' declared BuiltinOnly but no builtin registered",
                    spec.head);
            }
            EvalPaths::Both => {
                assert!(planner_resolves(&expr) || builtin_exists(spec.head),
                    "Head '{}' declared Both but neither path works",
                    spec.head);
            }
        }
    }
}
```

**Result**: Would have caught 5d5e34d immediately
**Status**: Not implemented (TODO)

### Test T2: "No cardinality regressions"

```rust
#[test]
fn test_no_head_count_regression() {
    let current_count = OPS_REGISTRY.len();
    let baseline_count = 70;  // From last release

    assert!(
        current_count >= baseline_count,
        "Public head count dropped from {} to {} without BREAKING_CHANGE flag",
        baseline_count, current_count
    );
}
```

**Result**: Would have flagged 84 → 64 drop
**Status**: Not implemented (TODO)

### Test T3: "Dual-path equivalence"

```rust
#[test]
fn test_dual_path_equivalence() {
    for spec in OPS_REGISTRY.iter().filter(|s| matches!(s.eval_paths, EvalPaths::Both)) {
        let test_cases = generate_test_cases(spec);

        for case in test_cases {
            let builtin_result = eval_direct(&case);
            let ir_result = execute(plan(&case));

            assert_results_equivalent(builtin_result, ir_result,
                "Head '{}' returns different results via builtin vs IR",
                spec.head);
        }
    }
}
```

**Result**: Validates IR migrations are safe
**Status**: Not implemented (TODO)

---

## Migration Path (Completing Original Intent)

The original intent of 5d5e34d was valid: route operations through IR for fusion. Here's the **gated** approach:

### Phase 1: Add Planner Mappings (while keeping builtins)

```rust
// In planner.rs
pub fn plan_operation(head: &str) -> Option<IRNode> {
    match head {
        "wkd" => Some(IRNode::Unary(UnaryOp::Weekday { input })),
        "xminus" => Some(IRNode::Binary(BinaryOp::PairwiseSpread { ... })),
        "mapr" => Some(IRNode::Join(JoinOp::MapR { ... })),
        // ... add all 20 operations
        _ => None
    }
}
```

**Status**: NOT DONE (5d5e34d assumed this existed)

### Phase 2: Add Equivalence Tests

```rust
#[test]
fn test_wkd_ir_equivalence() {
    let data = load_test_csv("dates.csv");
    let builtin_result = eval("(wkd data)");
    let ir_result = execute(plan("(wkd data)"));
    assert_frames_equal(builtin_result, ir_result);
}
```

**Status**: NOT DONE

### Phase 3: Mark as Both (transition state)

```rust
OpSpec {
    head: "wkd",
    eval_paths: EvalPaths::Both,  // ← Mark as dual-path
    ...
}
```

**Status**: Registry doesn't exist yet

### Phase 4: Remove Builtin (only after IR proven)

```rust
// Remove from builtins.rs:
// rt.register_builtin("wkd", builtin_wkd);  // ← Safe to remove now

OpSpec {
    head: "wkd",
    eval_paths: EvalPaths::IROnly,  // ← Now IR-only
    ...
}
```

**Status**: Future work (do this RIGHT next time)

---

## Lessons Learned

### What Went Wrong

1. **Assumption without verification**: Commit assumed planner mappings existed
2. **Missing safety net**: No reachability tests to catch this
3. **Incomplete change**: Removed old path without adding new path
4. **Misleading commit message**: Claimed completion that didn't exist

### What Went Right

1. **Clear error messages**: `Undefined variable: w5` led directly to problem
2. **Code preserved**: Functions existed, only registrations missing
3. **Fast diagnosis**: Git forensics identified exact cause in minutes
4. **Simple fix**: Restoring registrations was one-line changes

### Process Improvements

1. **Gate all refactors**: No removal without proven replacement
2. **Registry-driven**: Single source of truth for all operations
3. **Reachability testing**: CI must verify all heads resolve
4. **Explicit migration**: Use Both state during transitions

---

## Current State (2026-02-27)

### Fixed Operations

| Operation | Status | Registration | Notes |
|-----------|--------|--------------|-------|
| wkd, w5 | ✅ FIXED | e5a33de | Now handles Frame date index |
| xminus | ✅ FIXED | e5a33de | Pairwise spreads working |
| mapr | ✅ FIXED | 91dbb4b | Type coercion issue remains |
| cs1 | ✅ Working | (never broken) | Was registered via IR |

### Still TODO

| Item | Priority | Status |
|------|----------|--------|
| Add ops_registry.rs | High | Not started |
| Add Test T1 (reachability) | High | Not started |
| Add Test T2 (cardinality) | Medium | Not started |
| Add Test T3 (equivalence) | Medium | Not started |
| Complete planner mappings | Low | Defer until registry exists |

---

## Accountability

**Commit 5d5e34d violated**:
- ✗ Contract between commit message and code state
- ✗ Invariant: "no removal without replacement"
- ✗ Testing requirement: "prove equivalence before migration"

**Correct approach would have been**:
1. Add planner mappings FIRST
2. Add equivalence tests
3. Mark operations as Both (dual-path)
4. Verify reachability tests pass
5. THEN remove builtin registrations
6. Update to IROnly in registry

**What actually happened**:
- Skipped steps 1-5
- Went directly to removal
- Left system in broken state

---

## References

- Incident discovery: User session 2026-02-27
- Fix commits: e5a33de, 91dbb4b
- Breaking commit: 5d5e34d
- Related: RECOVERY_STATUS.md, GLD_NUM_GOLDEN_TEST_GUIDE.md
- Contract docs: contracts.md (should reference this post-mortem)

---

## Sign-off

**Incident**: CLOSED
**Fix verified**: GLD_NUM test now works through signal generation
**Prevention**: Post-mortem recorded, invariants documented
**Follow-up**: Implement registry + reachability tests (tracked separately)

*This post-mortem should be referenced in any future "remove shadowing" or "migrate to IR" work.*
