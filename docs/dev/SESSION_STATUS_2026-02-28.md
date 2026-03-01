# BLISP Session Status - 2026-02-28

**Repository:** `/home/ubuntu/blisp`
**Branch:** `reconstruct/tableview-only`
**Last Commit:** `89fdbca` - IEEE-754 numeric policy implementation
**GLD_NUM.csv Status:** ✅ **VALIDATES** (6826 rows)

---

## Executive Summary

### What Was Accomplished Today

1. ✅ **IEEE-754 Numeric Policy Implemented** (commit 89fdbca)
   - Fixed IR and AST to preserve inf/NaN behavior (no coercion)
   - Updated test comparators to handle inf/NaN correctly
   - Unignored `diff_small_unary_dlog` test (now passes)
   - GLD_NUM.csv validates after changes

2. ✅ **Reproducibility Infrastructure** (commits ef90a49, 952f3fb, 77acc27, f5193c1, b69880f)
   - Pinned Rust toolchain to 1.93.1 (rust-toolchain.toml)
   - Enforced `--locked` in all CI commands
   - Documented 15 ignored tests with fix criteria
   - Added CI tripwire to prevent ignored count from growing
   - Created REPRODUCIBILITY_CHECKLIST.md

3. ✅ **Dependency Hygiene** (commit 73d5857)
   - Tagged blawktrust at v0.1.1-orientation-stable (daf29d3)
   - Pinned BLISP to use git+tag instead of path dependency

4. 🔄 **Started GOLDEN_2 Secondary Pipeline** (incomplete)
   - Created test data generator (irregular timestamps + 33% NA)
   - Discovered `rolling-mean`/`rolling-std` are IR-only operations
   - Did NOT complete - see "Open Issues" below

### What's Clean in Git

**Last 5 commits:**
```
89fdbca - IEEE-754 numeric policy (IR matches AST)
952f3fb - Reproducibility checklist
ef90a49 - Pin Rust toolchain to 1.93.1
b69880f - Fix CI ignored test count
f5193c1 - Add CI tripwire for ignored tests
```

**Uncommitted files (untracked):**
- `GOLDEN_2.sh` - Secondary golden pipeline script (incomplete)
- `GOLDEN_2_DATA.csv` - Test data (500 rows, irregular timestamps, 33% NA)
- `create_golden2_data.py` - Data generator
- `OPERATION_NAMING_MAP.md` - Operation naming reference (created during investigation)

**No staged changes** - working tree is clean except for untracked files.

---

## Critical Files and Their Status

### Golden Tests
| File | Status | Purpose | Validation |
|------|--------|---------|------------|
| `GLD_NUM_BLISP.sh` | ✅ Works | Primary golden test | 6826 rows ✅ |
| `GLD_NUM_BLISP.csv` | ✅ Valid | Output from golden test | Verified |
| `GOLDEN_2.sh` | ⚠️ Incomplete | Secondary golden (unapproved) | Not validated |

### Documentation
| File | Status | Purpose |
|------|--------|---------|
| `BLISP_DISPATCH_MAP.md` | ✅ Current (v1.2) | **AUTHORITATIVE** operation routing map |
| `NUMERIC_POLICY.md` | ✅ Current | IEEE-754 edge case specification |
| `IGNORED_TESTS.md` | ✅ Current | 15 ignored tests with fix criteria |
| `REPRODUCIBILITY_CHECKLIST.md` | ✅ Current | Reproducibility status + policy questions |
| `OPERATION_NAMING_MAP.md` | ⚠️ Untracked | Created during investigation (may be redundant with DISPATCH_MAP) |

### Source Files - Recent Changes
| File | Last Change | Commit | Status |
|------|-------------|--------|--------|
| `src/exec.rs` | IEEE-754 fix | 89fdbca | ✅ Clean |
| `tests/common/mod.rs` | IEEE-754 fix | 89fdbca | ✅ Clean |
| `tests/differential_exec.rs` | Unignored dlog test | 89fdbca | ✅ Clean |
| `src/builtins.rs` | None recent | - | ✅ Clean |

---

## Test Status

### Passing Tests
- ✅ All clippy warnings resolved (commit cddf5d1)
- ✅ `cargo test` passes with 15 tests ignored (documented)
- ✅ `diff_small_unary_dlog` unignored and passing (IEEE-754 fix)
- ✅ GLD_NUM.csv validates (6826 rows)

### Ignored Tests (15 total)
Documented in `IGNORED_TESTS.md` with categories:
- **SEM (Semantic):** 7 tests - OFS vs OBS expectation mismatch
- **NUM (Numeric):** 3 tests - inf vs NaN edge cases (1 fixed, 2 remain)
- **NA (Missing Data):** 3 tests - NA propagation edge cases
- **MSK (Mask/Monotonicity):** 2 tests - Invalid under OBS semantics

**CI Tripwire:** Count must not exceed 15 (enforced in `.github/workflows/ci.yml`)

---

## Key Technical Decisions Made

### 1. IEEE-754 Numeric Policy (User Approved)
**Decision:** Adopt IEEE-754 behavior, NOT coercion to NaN.

**Rules:**
- `log(0.0)` → `-inf` (NOT NaN)
- `1.0 / 0.0` → `+inf` (NOT NaN)
- `0.0 / 0.0` → `NaN`
- `dlog(0→pos)` → `+inf`
- `dlog(pos→0)` → `-inf`
- `dlog(0→0)` → `NaN`

**Key Principle:** inf/NaN are numeric concepts, NA is missingness concept (keep distinct).

**Implementation:**
- Removed `if prev > 0.0 && x > 0.0` guards in dlog operations
- Let `ln()` return natural IEEE-754 results
- Updated test comparators with explicit `is_nan()`/`is_infinite()` checks

**Files Changed:**
- `src/exec.rs:1673-1680` - IR dlog_obs_column
- `tests/common/mod.rs:750-757` - AST dlog_obs_column
- `tests/common/mod.rs:699-716` - AST log_column
- `tests/common/mod.rs:127-146` - Test comparator

### 2. OBS Semantics is Source of Truth (Reaffirmed)
- Observation-based (skip NAs when looking back)
- Validated by GLD_NUM.csv (6826 rows)
- IR implementation is reference
- AST must match IR for differential testing

### 3. Reproducible Builds (User Approved)
- Rust toolchain pinned to 1.93.1
- All `cargo` commands use `--locked`
- blawktrust pinned to git tag v0.1.1-orientation-stable

---

## Open Issues and Blockers

### 1. GOLDEN_2 Pipeline (Secondary Golden Test)
**Status:** Started but incomplete

**What Exists:**
- ✅ Test data generated (500 rows, irregular timestamps, 33% NA)
- ✅ Data generator script (`create_golden2_data.py`)
- ⚠️ Pipeline script started (`GOLDEN_2.sh`)

**Blocker Discovered:**
- `rolling-mean` and `rolling-std` are **IR-only** operations (per BLISP_DISPATCH_MAP.md)
- NOT available as builtins
- NOT available in HYBRID mode (default)
- **Solution:** Use `BLISP_IR_ONLY=1` or `--ir-only` flag

**User's Approved Pipeline:**
```lisp
dlog → rolling-mean 5 → rolling-std 10
```

**Correct Usage (IR-only mode):**
```bash
BLISP_IR_ONLY=1 ./target/release/blisp -e '
  (save "output.csv"
    (rolling-std 10
      (rolling-mean 5
        (dlog (file "data.csv")))))'
```

**Decision Needed:**
- Complete GOLDEN_2 or defer?
- Part of reproducibility plan (step 2 of 7) but user questioned priority

### 2. Orientation System Bug (Documented, Not Fixed)
**File:** Memory file: `blisp_orientation_bug.md`

**Issue:** `(o 'Z table)` doesn't work - `layout` and `axis` fields are disconnected.

**Status:** Investigated but not fixed (noted in auto-memory 2026-02-27)

### 3. Git Cleanliness
**Status:** 4 untracked files related to GOLDEN_2

**Options:**
1. Complete GOLDEN_2 and commit
2. Delete untracked files (abandon GOLDEN_2)
3. Stash for later (git add + git stash)

---

## Reproducibility Plan Status (User Approved 7-Step Plan)

User gave this plan earlier in session:

| Step | Task | Status |
|------|------|--------|
| 1 | Implement numeric policy (IEEE-754) | ✅ **DONE** (89fdbca) |
| 2 | Add secondary golden pipeline (GOLDEN_2) | 🔄 **IN PROGRESS** |
| 3 | Add AST≡IR differential fuzz (safe ops subset) | ⏸️ **PENDING** |
| 4 | Add metamorphic tests (zscore + dlog scaling) | ⏸️ **PENDING** |
| 5 | Add performance benchmarks (pre-fusion) | ⏸️ **PENDING** |
| 6 | Add Docker clean-room bootstrap test | ⏸️ **PENDING** |
| 7 | Verify CI green on all pushed commits | ⏸️ **PENDING** |

**User's Revised Priorities (End of Session):**
1. GLD_NUM.csv test ✅
2. Orientations (bug exists but not fixed)
3. Clean git

**Question:** Continue reproducibility plan OR switch to orientations + clean git?

---

## Important Context for Tomorrow

### User Preferences
- **Always verify GLD_NUM.csv** at each step (user emphasized: "whatever you do. don't forget that we need to replicate GLD_NUM.csv at each point")
- User wants to be asked before git push
- User catches incorrect statements quickly - verify claims
- User has full dispatch map knowledge - check BLISP_DISPATCH_MAP.md before making assumptions

### Key Commands

**Verify GLD_NUM:**
```bash
cd /home/ubuntu/blisp
./GLD_NUM_BLISP.sh
wc -l GLD_NUM_BLISP.csv  # Should be 6826
```

**Check operation routing:**
```bash
cd /home/ubuntu/blisp
grep "operation-name" BLISP_DISPATCH_MAP.md
```

**Build and test:**
```bash
cd /home/ubuntu/blisp
cargo build --locked --release
cargo test --locked
cargo clippy --locked -- -D warnings --all-targets --all-features
```

### Files to Never Modify Without Approval
- `GLD_NUM_BLISP.sh` - SACRED SCRIPT (marked in file)
- `BLISP_DISPATCH_MAP.md` - AUTHORITATIVE reference
- `NUMERIC_POLICY.md` - Policy decisions

### The Dispatch Map is Law
**BLISP_DISPATCH_MAP.md** (v1.2) is the **single source of truth** for:
- Which operations are available where (IR, builtins, legacy)
- Argument order and signatures
- Mode behavior (HYBRID, IR-only, Legacy-only)

**Key Lessons:**
- `rolling-mean`, `rolling-std` are **IR-only** (use `--ir-only` flag)
- In HYBRID mode (default), IR tries first and shadows builtins
- Check dispatch map BEFORE assuming an operation exists as a builtin

---

## Files Created This Session (Untracked)

### Keep for Future Work
- `create_golden2_data.py` - Test data generator (well-formed, useful)
- `GOLDEN_2_DATA.csv` - Test data (500 rows, irregular timestamps, 33% NA)

### Review/Decide
- `GOLDEN_2.sh` - Pipeline script (incomplete, uses IR-only ops)
- `OPERATION_NAMING_MAP.md` - May be redundant with BLISP_DISPATCH_MAP.md

---

## What to Do Tomorrow - Decision Points

### Option A: Continue Reproducibility Plan
1. Complete GOLDEN_2 (fix to use `--ir-only` mode)
2. Validate output
3. Commit all GOLDEN_2 files
4. Move to step 3: Differential fuzz

### Option B: Pivot to Orientations + Clean Git
1. Delete/stash GOLDEN_2 untracked files
2. Fix orientation system bug (`(o 'Z table)`)
3. Get to clean git state
4. Decide on reproducibility plan later

### Option C: Hybrid Approach
1. Commit what's useful from GOLDEN_2 (data generator + data)
2. Delete incomplete pipeline script
3. Fix orientations
4. Return to reproducibility plan later

**User should decide which path to take.**

---

## Quick Reference

### Repository Locations
- **BLISP:** `/home/ubuntu/blisp` (branch: `reconstruct/tableview-only`)
- **CLISPI:** `/home/ubuntu/clispi_dev`
- **blawktrust:** `/home/ubuntu/blawktrust`

### Current Branch Status
```
Branch: reconstruct/tableview-only
Status: Up to date with origin
Commits ahead: 0
Uncommitted changes: 0 (working tree clean)
Untracked files: 4 (GOLDEN_2 related)
```

### Last Working State
- All tests passing (15 ignored, documented)
- Zero clippy warnings
- GLD_NUM.csv validates
- CI should be green (last push was 73d5857)

### Auto-Memory Notes
Stored in `/home/ubuntu/.claude/projects/-home-ubuntu-clispi-dev/memory/MEMORY.md`:
- Orientation bug documented
- Recent dispatch system work logged
- User preferences captured

---

## Session End Checklist

- [x] GLD_NUM.csv validates (6826 rows)
- [x] Git status checked (clean except untracked)
- [x] Last commits reviewed (89fdbca is latest)
- [x] Uncommitted changes discarded (builtins.rs restored)
- [x] Untracked files documented
- [x] Open issues identified
- [x] Decision points for tomorrow outlined
- [x] Status document created

**Ready for tomorrow's session.**
