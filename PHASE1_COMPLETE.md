# Phase 1 Complete - GLD_NUM Quick Wins

**Date**: 2026-02-21
**Status**: ✅ All Phase 1 operations implemented and tested

---

## What Was Implemented

### 1. stdin Source (30 min)
**Commit**: `fc4144e` - Add stdin source to IR executor

**Implementation**:
- Added `Source::Stdin` variant to `ir.rs`
- Planner support in `planner.rs` for `(stdin)` syntax
- Executor reads from stdin pipe with CSV parsing
- Made `parse_csv_to_frame` public in `io.rs`

**Tests**:
```bash
echo "DATE;val
2020-01-01;100
2020-01-02;-50" | ./blisp -e '(stdin)'
```

**Status**: ✅ Working perfectly with pipes

---

### 2. wzs (Windowed Z-Score) (15 min)
**Commit**: `28e2328` - Add wzs for CLISPI compatibility

**Implementation**:
- Extended planner pattern matching: `"rolling-zscore" | "wzs"`
- CLISPI signature: `(wzs window step x)` (step ignored for now)
- Expands to: `(/ (- x (rolling-mean w x)) (rolling-std w x))`
- Same as rolling-zscore but with 3-arg signature

**Tests**:
```bash
./blisp -e '(wzs 3 1 (read-csv "data.csv"))'
echo "..." | ./blisp -e '(let ((x (stdin))) (wzs 20 1 x))'
```

**Status**: ✅ Works with files and stdin (via let binding)

---

### 3. > (Greater Than Comparison) (1 hour)
**Commit**: `0429dc2` - Add > comparison operator to IR

**Implementation**:
- Added `BinaryFunc::Gt` to `ir.rs`
- Executor implementation in `exec.rs` (scalar and frame-frame)
- Fusion support in `ir_fusion.rs`
- Planner support in `planner.rs` for `(> x threshold)` syntax

**Semantics**:
- Returns **numeric mask**: 1.0 (true), 0.0 (false), NA (if input is NA)
- **Shape-preserving**: I1-I3 invariants hold
- **Arithmetic filtering**: `(* value (> value threshold))`

**Tests**:
```bash
# Basic comparison
echo "..." | ./blisp -e '(> (stdin) 0)'

# Threshold comparison
./blisp -e '(> (stdin) 60)'

# Arithmetic filtering (zeros out negatives)
./blisp -e '(let ((x (stdin))) (* x (> x 0)))'

# Frame-frame comparison
./blisp -e '(let ((data (stdin))) (> data data))'  # All 0s (x == x)

# NA propagation
echo "DATE;val
...;
..." | ./blisp -e '(> (stdin) 0)'  # NA → NA
```

**Status**: ✅ All tests passing, NA handling correct

---

## Integration Test

**Full Phase 1 pipeline** (stdin → wzs → > filter):

```bash
echo "DATE;val
2020-01-01;100
2020-01-02;105
2020-01-03;102
2020-01-04;108
2020-01-05;106
2020-01-06;110
2020-01-07;104
2020-01-08;112" | ./blisp -e '(let ((x (stdin)))
                                 (let ((z (wzs 3 1 x)))
                                   (* z (> z 0))))'
```

**Result**:
```
DATE;val
2020-01-01;NA        # Not enough data for window=3
2020-01-02;NA        # Not enough data
2020-01-03;-0        # z < 0, filtered out
2020-01-04;1.224...  # z > 0, kept
2020-01-05;0.267...  # z > 0, kept
2020-01-06;1.224...  # z > 0, kept
2020-01-07;-0        # z < 0, filtered out
2020-01-08;0.980...  # z > 0, kept
```

**Status**: ✅ All operations working together perfectly

---

## Impact

### GLD_NUM Coverage (7/15 → 10/15)

**Before Phase 1**:
- ✅ dlog, shift, mapr, *, file, read-csv, let*
- ❌ stdin, wzs, >, locf, cs1, x-, ur, ecs1

**After Phase 1**:
- ✅ **stdin** (new!)
- ✅ **wzs** (new!)
- ✅ **>** (new!)
- ✅ dlog, shift, mapr, *, file, read-csv, let*
- ❌ locf, cs1, x-, ur, ecs1

**Remaining**: 5 operations (Phase 2 + Phase 3)

---

## Performance

All Phase 1 operations run on the **IR executor** with:
- **6-102x speedup** over legacy AST evaluator
- **Hybrid mode**: Auto-fallback for unsupported ops
- **Shape-preserving**: I1-I3 invariants enforced at compile time

---

## Next Steps: Phase 2

**Target**: GLD_NUM blockers (~6 hours)

1. **locf** (1-2 hours) - Last observation carried forward
   - `w5` macro maps to `locf`
   - Fill NA values forward
   - Idempotent: `locf(locf(x)) == locf(x)`

2. **cs1** (2 hours) - Cumulative sum starting at 1.0
   - Running total: `cs1[i] = cs1[i-1] + x[i]`
   - Starts at 1.0 (not 0.0!)
   - Used in index reconstruction

3. **x-** (2-3 hours) - Pairwise spread (schema-transforming)
   - `(x- data half)` → subtract col[half] from all others
   - Reduces ncols by 1
   - New colnames: col2-col1, col3-col1, ...
   - **First schema-transforming op** (breaks I2)

**After Phase 2**: GLD_NUM runs end-to-end on IR! 🎉

---

## Key Learnings

### Semantic Corrections
The user corrected three critical misunderstandings:

1. **> is comparison (mask), NOT row filtering**
   - ✅ Shape-preserving: returns 1.0/0.0/NA
   - ❌ NOT row filtering (which breaks I1-I3)

2. **cs1 is cumulative sum, NOT cross-sectional z-score**
   - ✅ Running total: `cs1[i] = cs1[i-1] + x[i]`
   - ❌ NOT standardization across columns

3. **ur is unit ratio, NOT rolling regression**
   - ✅ Simple formula: `value / (100 * √252 * rolling_std)`
   - ❌ NOT beta/regression calculation

These corrections simplified implementation significantly (~9 hours vs 13 hours estimate).

---

## Commits

1. `fc4144e` - Add stdin source to IR executor
2. `28e2328` - Add wzs for CLISPI compatibility
3. `0429dc2` - Add > comparison operator to IR

**Total Phase 1 time**: ~2 hours (as estimated)

---

*Phase 1 complete: 2026-02-21*
*Ready for Phase 2 (GLD_NUM blockers)*
