# Token Refactoring Complete ✅

## Summary

Successfully refactored canonical token names in blisp codebase.

## Changes Made

### 1. IR Enum (src/ir.rs)
- `WKD` → `Wkd`
- `ShiftObs` → `LagObs`
- `RollMeanPartial` → `RollMeanMin2`
- `RollStdPartial` → `RollStdMin2`
- `RollMeanPartialExclCurrent` → `RollMeanMin2ExclCurrent`
- `RollStdPartialExclCurrent` → `RollStdMin2ExclCurrent`

### 2. Parser Tokens (src/planner.rs)
- **Removed all aliases:**
  - `"w5"` (was alias for WKD)
  - `"shiftm"` (was alias for shift-obs)
  - `"wavp"` (was alias for rolling-mean-partial)
  - `"wstp"` (was alias for rolling-std-partial)

- **New canonical tokens:**
  - `"wkd"` (lowercase)
  - `"lag-obs"` (replaces shift-obs)
  - `"rolling-mean-min2"` (replaces rolling-mean-partial)
  - `"rolling-std-min2"` (replaces rolling-std-partial)

### 3. Executor Updates (src/exec.rs)
- Updated all pattern matches to use new enum names
- Updated comments and documentation

### 4. IR Fusion Updates (src/ir_fusion.rs)
- Updated fusion optimizer to recognize new enum names

### 5. Main Updates (src/main.rs)
- Updated --dic dictionary output
- Changed note: "wkd is the canonical weekend mask operation"

### 6. Builtin Registration (src/builtins.rs)
- Registration already used lowercase "wkd"
- Updated all WKD references in comments

## Final Canonical Vocabulary

### Time/Mask:
- `wkd` - Weekend mask
- `shift` - Calendar lag
- `lag-obs` - Observation lag (mask-aware)
- `keep` - Downsample
- `locf` - Last observation carried forward
- `with-mask` - Activate mask

### Rolling:
- `rolling-mean` - Strict (requires exactly w observations)
- `rolling-std` - Strict std deviation
- `rolling-mean-min2` - Relaxed (min 2 observations)
- `rolling-std-min2` - Relaxed std deviation
- `rolling-zscore` - Z-score

## Verification

✅ Compilation successful
✅ Tests pass: `wkd` works correctly (masks weekends)
✅ Tests pass: `rolling-mean-min2` works (relaxed windows)
✅ No residual old token references in source
✅ All enum variants updated consistently

## Breaking Changes

**Users must update their scripts:**
- `WKD` → `wkd`
- `w5` → `wkd`
- `shift-obs` → `lag-obs`
- `shiftm` → `lag-obs`
- `rolling-mean-partial` → `rolling-mean-min2`
- `rolling-std-partial` → `rolling-std-min2`
- `wavp` → `rolling-mean-min2`
- `wstp` → `rolling-std-min2`

## Date
2026-02-22
