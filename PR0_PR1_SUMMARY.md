# PR0 + PR1 Implementation Summary

**Date**: 2026-02-26
**Status**: ✅ READY TO EXECUTE

---

## PR0: CI Tripwires ✅ COMPLETE

### Files Created

1. **`ci/test_no_token_conflicts.sh`**
   - Detects tokens in both `builtins.rs` and `planner.rs`
   - Currently catches 20 conflicts
   - Exit 0 = pass, Exit 1 = fail

2. **`ci/test_no_kernel_dupes.sh`**
   - Detects local kernels that shadow blawktrust imports
   - Currently catches `dlog_column` duplication
   - Exit 0 = pass, Exit 1 = fail

### Verification Results

```bash
$ cd /home/ubuntu/blisp
$ ./ci/test_no_token_conflicts.sh .
FAIL: token(s) are defined in BOTH builtins and planner (shadowing IR):
  - * + - / > abs asofr cs1 dlog exp locf log mapr
  - mask-weekend shift stdin ur with-mask wkd xminus

$ ./ci/test_no_kernel_dupes.sh .
FAIL: local dlog_column() exists but blawktrust dlog_column is imported.
  Local: src/exec.rs:1092
  Import: src/builtins.rs:12
```

✅ **Both tripwires correctly detect known bugs**

### CI Integration

Add to `.github/workflows/ci.yml`:

```yaml
- name: Architecture Tripwires
  run: |
    bash ci/test_no_token_conflicts.sh .
    bash ci/test_no_kernel_dupes.sh .
```

---

## PR1: Remove Builtin Shadowing ⏳ READY

### Scope

**Delete 20 builtin registrations** from `src/builtins.rs`:

| Category | Lines | Tokens | Count |
|----------|-------|--------|-------|
| Arithmetic | 63-66 | `+`, `-`, `*`, `/` | 4 |
| Math | 69-71 | `log`, `exp`, `abs` | 3 |
| Column | 74-75 | `dlog`, `shift` | 2 |
| I/O | 94 | `stdin` | 1 |
| Comparison | 113 | `>` | 1 |
| Shape | 123, 129 | `locf`, `wkd` | 2 |
| Mask | 130-131 | `mask-weekend`, `with-mask` | 2 |
| Schema | 137-138 | `xminus`, `cs1` | 2 |
| Join | 146-148 | `mapr`, `asofr`, `ur` | 3 |

### Automated Execution

**One-command application**:
```bash
cd /home/ubuntu/blisp
./PR1_DELETE_COMMANDS.sh
```

This script will:
1. Create backup: `src/builtins.rs.pr1_backup`
2. Delete 20 lines (in reverse order)
3. Run tripwire (verify no conflicts)
4. Test compilation (cargo build)
5. Auto-rollback if any step fails

**Manual alternative**: Follow line-by-line guide in `PR1_REMOVE_SHADOWING_GUIDE.md`

### Expected Results

✅ **Tripwire passes**:
```bash
$ ./ci/test_no_token_conflicts.sh .
OK: no planner/builtin token conflicts.
```

✅ **Compilation succeeds**:
```bash
$ cargo build
   Compiling blisp v0.1.0
    Finished dev [unoptimized + debuginfo]
```

⚠️ **Some tests may fail**: Tests calling removed builtins need migration.

### Impact

**Before PR1**:
```lisp
(dlog data)  ; → builtin_dlog_cols (bypasses IR)
```

**After PR1**:
```lisp
(dlog data)  ; → planner.rs:123 → NumericFunc::SHF_PTW_NLN_DLOG → exec.rs:157
```

**Result**: All 20 tokens now route through IR, enabling fusion optimizations.

---

## Files Generated

### Documentation
- `PR0_TRIPWIRE_REPORT.md` - Tripwire verification results
- `PR1_REMOVE_SHADOWING_GUIDE.md` - Line-by-line edit guide
- `PR0_PR1_SUMMARY.md` - This file

### Scripts
- `ci/test_no_token_conflicts.sh` - Token conflict detector
- `ci/test_no_kernel_dupes.sh` - Kernel duplication detector
- `PR1_DELETE_COMMANDS.sh` - Automated PR1 execution

### Reports (from earlier analysis)
- `AS_IS_ARCHITECTURE_REPORT.md` - Full architecture audit
- `TOKEN_INVENTORY_COMPLETE.md` - Complete token catalog

---

## Execution Plan

### Phase 1: Verify Tripwires (NOW)

```bash
cd /home/ubuntu/blisp
./ci/test_no_token_conflicts.sh .  # Should FAIL (20 conflicts)
./ci/test_no_kernel_dupes.sh .     # Should FAIL (dlog duplication)
```

✅ **Status**: Both failing as expected (bugs detected)

### Phase 2: Execute PR1 (NEXT)

```bash
cd /home/ubuntu/blisp
git checkout -b pr1-remove-builtin-shadowing
./PR1_DELETE_COMMANDS.sh
```

**Expected**:
- 20 lines deleted
- Tripwire passes
- Compilation succeeds
- Some tests fail (need migration)

### Phase 3: Commit PR1

```bash
git add src/builtins.rs ci/
git commit -m "PR1: Remove 20 builtin registrations shadowing IR mappings

- Remove arithmetic operators (+, -, *, /, >) from builtins
- Remove math functions (log, exp, abs) from builtins
- Remove core ops (dlog, shift, locf, wkd, cs1) from builtins
- Remove joins (mapr, asofr, ur) from builtins
- Remove schema ops (xminus, mask-weekend, with-mask) from builtins
- Remove stdin I/O builtin

All removed tokens now route through planner.rs → IR → exec.rs
Enables fusion optimization for these operations.

Fixes: #<issue> (18-token shadowing bug)
Tripwire: ci/test_no_token_conflicts.sh now passes"
```

### Phase 4: PR2 (FUTURE)

Delete `src/exec.rs:1092` (local dlog_column) → call blawktrust version

---

## Risk Assessment

### Low Risk ✅
- **Tripwires**: Deterministic, no side effects
- **PR1 script**: Auto-rollback on failure
- **Backup**: `.pr1_backup` file created

### Medium Risk ⚠️
- **Test failures**: Some tests may need token updates
- **Behavioral changes**: IF builtin and IR implementations differ (PR2 will verify)

### Mitigation
- Comprehensive documentation (3 guides)
- Automated execution script
- Backup and rollback mechanism
- Tripwires prevent regression

---

## Validation Checklist

Before merging PR1:

- [ ] `./ci/test_no_token_conflicts.sh .` passes
- [ ] `cargo build` succeeds
- [ ] `cargo test` results reviewed (some failures expected)
- [ ] Manual smoke test: `echo '(dlog (file "test.csv"))' | blisp` works
- [ ] Diff reviewed: `git diff src/builtins.rs`

---

## Next PRs (Roadmap)

**PR2**: Unify dlog_column kernel
- Delete exec.rs:1092
- Call blawktrust::builtins::ops::dlog_column everywhere
- Differential tests (old vs new output)

**PR3**: Rebuild `*-cols` as IR combinators
- `dlog-cols` → `MAP_NUM_COLS(dlog)` with IR subplan per column
- Enables fusion inside column pipelines

**PR4**: Implement fusion optimization
- Unary chain fusion
- Allocation reuse
- Benchmark harness

**PR5**: Remove dead code
- Clean up unreachable branches
- Remove `.backup_before_canonical` files

---

## Key Metrics

| Metric | Before | After PR1 | Target (PR4) |
|--------|--------|-----------|--------------|
| Token conflicts | 20 | 0 | 0 |
| Kernel duplicates | 1 | 1 → 0 (PR2) | 0 |
| IR-routed ops | 0% | 100% (20 tokens) | 100% |
| Fusion-eligible | 0% | 100% | 100% |

---

## Summary

✅ **PR0 Complete**: Tripwires deployed and verified
⏳ **PR1 Ready**: One-command execution available
📋 **Documentation**: 3 comprehensive guides
🔬 **Validation**: Deterministic, automated, safe

**Recommendation**: Execute PR1 now using `./PR1_DELETE_COMMANDS.sh`

---

**END OF SUMMARY**
