# PR1: Remove Builtin Shadowing (Surgical Edit List)

**Date**: 2026-02-26
**Goal**: Remove 20 builtin registrations that shadow IR planner mappings
**Validation**: `ci/test_no_token_conflicts.sh` must pass after edits

---

## Strategy

**Policy**: Remove builtin registrations for tokens that have IR mappings.

**Rationale**:
- IR path provides optimization opportunities (fusion)
- Canonical naming already implemented in IR
- Builtins bypass planner, preventing future compiler improvements

**Migration Path**: If legacy compatibility needed, rename to `legacy/<token>` (not implemented in this PR).

---

## File to Edit: `src/builtins.rs`

### Section 1: Arithmetic Operators (Lines 63-66)

**DELETE** these 4 registrations:

```rust
// Line 63
rt.register_builtin("+", builtin_add);

// Line 64
rt.register_builtin("-", builtin_sub);

// Line 65
rt.register_builtin("*", builtin_mul);

// Line 66
rt.register_builtin("/", builtin_div);
```

**Why**: All 4 have IR mappings to `BinaryFunc::{ADD, SUB, MUL, DIV}` (planner.rs:519-522)

**Impact**: Arithmetic expressions now compile to IR and benefit from fusion.

---

### Section 2: Math Functions (Lines 69-71)

**DELETE** these 3 registrations:

```rust
// Line 69
rt.register_builtin("log", builtin_log);

// Line 70
rt.register_builtin("exp", builtin_exp);

// Line 71
rt.register_builtin("abs", builtin_abs);
```

**Why**: IR mappings exist (planner.rs:125-128) to `NumericFunc::{LOG, EXP, ABS}`

**Impact**: Math ops now use IR executor path.

---

### Section 3: Column Operations (Lines 74-75)

**DELETE** these 2 registrations:

```rust
// Line 74
rt.register_builtin("dlog", builtin_dlog_cols);   // Table version (default lag=1)

// Line 75
rt.register_builtin("shift", builtin_shift_cols); // Table version (default lag=1)
```

**Why**:
- `dlog` → `NumericFunc::SHF_PTW_NLN_DLOG` (planner.rs:123)
- `shift` → `NumericFunc::SHF_PTW_LIN_SHF{k}` (planner.rs:135-149)

**Impact**: Core time-series ops now go through IR.

**Note**: Keep `dlog-cols` and `shift-cols` (lines 108-109) for explicit table operations (will be refactored in PR3).

---

### Section 4: I/O Operations (Line 94)

**DELETE** this registration:

```rust
// Line 94
rt.register_builtin("stdin", builtin_stdin);
```

**Why**: `stdin` has IR mapping to `Source::Stdin` (planner.rs:108-120)

**Impact**: Stdin loading now uses IR source node.

---

### Section 5: Comparison (Line 113)

**DELETE** this registration:

```rust
// Line 113
rt.register_builtin(">", builtin_gt_cols);      // Surface name → table version
```

**Why**: `>` → `BinaryFunc::GTR` (planner.rs:523)

**Impact**: Comparison ops now compile to IR.

**Note**: Keep `>-cols` (line 114) and `>-col` (line 115) as explicit variants.

---

### Section 6: Shape Operations (Lines 123, 129)

**DELETE** these 2 registrations:

```rust
// Line 123
rt.register_builtin("locf", builtin_locf);

// Line 129
rt.register_builtin("wkd", builtin_wkd);
```

**Why**:
- `locf` → `NumericFunc::SHF_REC_NLN_LOCF` (planner.rs:130)
- `wkd` → `NumericFunc::MSK_WKE` (planner.rs:131)

**Impact**: Fill-forward and weekday masking now use IR.

**Note**: Keep `locf-cols` (line 124) for table operations.

---

### Section 7: Mask Operations (Lines 130-131)

**DELETE** these 2 registrations:

```rust
// Line 130
rt.register_builtin("mask-weekend", builtin_mask_weekend);

// Line 131
rt.register_builtin("with-mask", builtin_with_mask);
```

**Why**:
- `mask-weekend` → `SchemaOp::MSK_WKE_DEF` (planner.rs:557-584)
- `with-mask` → `SchemaOp::WTH_MSK` (planner.rs:586-605)

**Impact**: Mask system now uses IR schema operations.

**Note**: Keep `mask-on` (line 132) as it's an alias for `with-mask` and won't conflict.

---

### Section 8: Schema Operations (Lines 137-138)

**DELETE** these 2 registrations:

```rust
// Line 137
rt.register_builtin("xminus", builtin_xminus);

// Line 138
rt.register_builtin("cs1", builtin_cs1_cols);      // Surface name → table version
```

**Why**:
- `xminus` → `SchemaOp::SHF_PTW_LIN_SPR` (planner.rs:530-554)
- `cs1` → `NumericFunc::SHF_PFX_LIN_SUM` (planner.rs:132)

**Impact**: Pairwise spreads and cumulative sum now use IR.

**Note**: Keep `cs1-cols` (line 139) and `cs1-col` (line 140) as explicit variants.

---

### Section 9: Join Operations (Lines 146-148)

**DELETE** these 3 registrations:

```rust
// Line 146
rt.register_builtin("mapr", builtin_mapr);

// Line 147
rt.register_builtin("asofr", builtin_asofr);

// Line 148
rt.register_builtin("ur", builtin_ur_cols);      // Surface name → table version
```

**Why**:
- `mapr` → `JoinOp::ALIGN` (planner.rs:526)
- `asofr` → `JoinOp::ASOF_ALIGN` (planner.rs:527)
- `ur` → Composite IR plan (planner.rs:407-463)

**Impact**: Join operations and unit ratio now compile to IR.

**Note**: Keep `ur-cols` (line 149) and `ur-col` (line 150) as explicit variants.

---

## Summary of Deletions

**File**: `src/builtins.rs`
**Lines to DELETE**: 20 total

| Section | Lines | Tokens | Count |
|---------|-------|--------|-------|
| Arithmetic | 63-66 | +, -, *, / | 4 |
| Math | 69-71 | log, exp, abs | 3 |
| Column | 74-75 | dlog, shift | 2 |
| I/O | 94 | stdin | 1 |
| Comparison | 113 | > | 1 |
| Shape | 123, 129 | locf, wkd | 2 |
| Mask | 130-131 | mask-weekend, with-mask | 2 |
| Schema | 137-138 | xminus, cs1 | 2 |
| Join | 146-148 | mapr, asofr, ur | 3 |

---

## Implementation Steps

### 1. Create Branch
```bash
cd /home/ubuntu/blisp
git checkout -b pr1-remove-builtin-shadowing
```

### 2. Apply Edits

**Recommended approach**: Edit in reverse order (bottom to top) to avoid line number shifts.

```bash
# Delete lines in reverse order
sed -i '148d' src/builtins.rs  # ur
sed -i '147d' src/builtins.rs  # asofr
sed -i '146d' src/builtins.rs  # mapr
sed -i '138d' src/builtins.rs  # cs1
sed -i '137d' src/builtins.rs  # xminus
sed -i '131d' src/builtins.rs  # with-mask
sed -i '130d' src/builtins.rs  # mask-weekend
sed -i '129d' src/builtins.rs  # wkd
sed -i '123d' src/builtins.rs  # locf
sed -i '113d' src/builtins.rs  # >
sed -i '94d' src/builtins.rs   # stdin
sed -i '75d' src/builtins.rs   # shift
sed -i '74d' src/builtins.rs   # dlog
sed -i '71d' src/builtins.rs   # abs
sed -i '70d' src/builtins.rs   # exp
sed -i '69d' src/builtins.rs   # log
sed -i '66d' src/builtins.rs   # /
sed -i '65d' src/builtins.rs   # *
sed -i '64d' src/builtins.rs   # -
sed -i '63d' src/builtins.rs   # +
```

**OR** manually delete using editor (safer for review).

### 3. Verify Tripwire Passes

```bash
./ci/test_no_token_conflicts.sh .
# Expected: OK: no planner/builtin token conflicts.
```

### 4. Verify Compilation

```bash
cargo build
# Expected: success (no syntax errors)
```

### 5. Run Tests

```bash
cargo test
# Expected: tests may fail due to missing builtins, but compilation succeeds
```

---

## Expected Test Failures

**Some tests may fail** because they invoke the removed builtin tokens directly. These need to be updated to use explicit variants or IR path.

### Test Migration Examples

**Before** (fails after PR1):
```rust
rt.eval_str("(dlog data)")?;  // FAILS: "dlog" builtin removed
```

**After** (uses IR path):
```rust
rt.eval_str("(dlog data)")?;  // OK: goes through planner → IR
```

**OR** (uses explicit variant):
```rust
rt.eval_str("(dlog-cols data)")?;  // OK: still a builtin
```

---

## Acceptance Criteria

✅ **Tripwire passes**: `./ci/test_no_token_conflicts.sh .` returns exit code 0
✅ **Compilation succeeds**: `cargo build` completes without errors
✅ **IR routing works**: Add test that `(dlog data)` produces expected output via IR path

---

## Rollback Plan

If issues arise:
```bash
git checkout src/builtins.rs  # Restore original
```

---

## Next Steps After PR1

1. **PR2**: Delete duplicate `dlog_column` (exec.rs:1092)
2. **PR3**: Refactor `*-cols` builtins to use `MAP_NUM_COLS` with IR
3. **PR4**: Implement fusion optimization

---

## Questions/Concerns

### Q: Will this break existing user code?
**A**: Only if users relied on builtin-specific behavior that differs from IR. Differential testing (PR2) will verify equivalence.

### Q: Why keep `*-cols` variants?
**A**: They will be refactored in PR3 to compile to IR. Keeping them now avoids breaking table operations during migration.

### Q: What about `stdin` builtin removal?
**A**: IR has `Source::Stdin` node (planner.rs:108-120). File loading routes through IR.

---

**END OF PR1 GUIDE**
