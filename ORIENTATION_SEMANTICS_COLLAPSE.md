# BLISP Orientation Semantics - Intentional Collapse

**Date**: 2026-02-28
**Status**: ⚠️ **SEMANTIC SIMPLIFICATION IN EFFECT**
**Why**: Fixed critical bug by collapsing D4 orientation → 2-state axis selector

---

## What the Fix Actually Does

### Before Fix
- **8 orientation states** (D4 dihedral group):
  - `H`, `N`, `S`, `Z` (4 canonical)
  - `NSWE`, `SNWE`, `EWNS`, `WENS` (4-letter codes)
  - Plus underscore variants (not supported)
- **Problem**: All symbols set dead `layout` field → **zero behavior change**

### After Fix (Current State)
- **2 orientation states** (axis selector only):
  - `'H`, `'N`, `'NSWE`, `'SNWE` → `Axis::Col` (aggregate down rows per column)
  - `'Z`, `'S`, `'WENS`, `'EWNS` → `Axis::Row` (aggregate across columns per row)
- **Result**: Symbols now change behavior for **5 legacy builtins only**

---

## Semantic Collapse: What Was Lost

### D4 Directionality (Not Implemented)

| Symbol | Original Intended Meaning | Current Actual Behavior |
|--------|---------------------------|------------------------|
| `H` | Horizontal (NSWE) | `Axis::Col` ✅ |
| `N` | North start (time-reversed, tac) | `Axis::Col` ❌ (same as H) |
| `Z` | Zigzag / row-major (WENS) | `Axis::Row` ✅ |
| `S` | South start (column-reversed) | `Axis::Row` ❌ (same as Z) |

**Lost semantics**:
- ❌ **Time reversal** (`H` → `N` should be `tac`)
- ❌ **Column reversal** (`Z` → `S` should reverse column order)
- ❌ **Diagonal traversals** (underscore variants like `H_`, `Z_`)
- ❌ **All 8 D4 orientations** → collapsed to 2

**Why this is acceptable for now**:
- The original D4 semantics were **never implemented** (dead code)
- Current fix makes symbols **actually do something** (better than nothing)
- Aggregation axis (col vs row) is the **only semantics anyone uses today**

---

## Scope of Fix: Legacy Builtins Only

### Operations That NOW Respond to `(o 'Z ...)`

**✅ Legacy builtins** (5 functions that check `tv.axis`):

| Function | File:Line | Axis::Col | Axis::Row |
|----------|-----------|-----------|-----------|
| `sum` | builtins.rs:2681 | Down rows → 1×N | Across cols → M×1 |
| `mean` | builtins.rs:2782 | Down rows → 1×N | Across cols → M×1 |
| `std` | builtins.rs:2907 | Down rows → 1×N | Across cols → M×1 |
| `cs1-cols` | builtins.rs:1543 | Down rows per col | Across cols per row |
| `ecs1-cols` | builtins.rs:1667 | Down rows per col | Across cols per row |

### Operations That IGNORE Axis

**❌ IR operations** (do NOT consult `axis` field):

All IR planner operations work on **Frame** or direct **TableView** and do not check the BLISP `axis` metadata:

| IR Operation | Path | Checks axis? |
|--------------|------|--------------|
| `dlog` | planner.rs:123 → exec.rs:157 | ❌ No |
| `shift` | planner.rs:130 → exec.rs:164 | ❌ No |
| `cs1` | planner.rs:131 → exec.rs:165 | ❌ No |
| `locf` | planner.rs:132 → exec.rs:166 | ❌ No |
| `wkd` | planner.rs:135 → exec.rs:169 | ❌ No |

**Why**: IR executor (`exec.rs`) operates directly on `blawktrust::TableView` without consulting BLISP's `TableViewWithMetadata.axis` field.

**Example showing the gap**:

```lisp
(defparameter df (file "data.csv"))

;; This pipeline:
(-> df
    (o 'Z)      ;; Sets axis=Row
    (dlog 1)    ;; ❌ IR path - ignores axis
    (sum))      ;; ✅ Legacy builtin - uses axis

;; Result: dlog ignores 'Z, sum respects it
```

**Implication**: Axis only affects **final aggregation**, not intermediate transforms.

---

## Keyword Syntax Issue (Still Unresolved)

### Current Reality

| Syntax | Works? | Why |
|--------|--------|-----|
| `(o 'Z df)` | ✅ Yes | Symbol, resolved to string "Z" |
| `(o ":row" df)` | ✅ Yes | String literal, starts with ':' |
| `(o :row df)` | ❌ No | Keyword - not implemented in parser |

**Problem**: Lisp keywords (`:row`, `:col`) are not fully supported in BLISP parser.

**Workaround**: Use **string literals** for keywords:
```lisp
(o ":row" table)   ;; ✅ Works
(o ":col" table)   ;; ✅ Works
```

**Long-term fix**: Add proper keyword support to parser (separate issue).

---

## What "Orientation" Means Now (Accurate Definition)

### Current Semantics (Phase 0/1)

**"Orientation" in BLISP is NOT physical layout or D4 direction.**

**"Orientation" = Aggregation Axis Selector**

- **Column-wise** (`Axis::Col`): Aggregate down rows, per column (default)
  - Set by: `'H`, `'N`, `'NSWE`, `'SNWE`, `":col"`
  - Behavior: `sum(M×N) → 1×N` (one value per column)

- **Row-wise** (`Axis::Row`): Aggregate across columns, per row
  - Set by: `'Z`, `'S`, `'WENS`, `'EWNS`, `":row"`
  - Behavior: `sum(M×N) → M×1` (one value per row)

### What It Is NOT

- ❌ NOT physical memory layout (column-major vs row-major storage)
- ❌ NOT data transposition (no data movement happens)
- ❌ NOT D4 dihedral group orientation (no direction/corner semantics)
- ❌ NOT consulted by IR operations (only legacy builtins)

### Terminology Recommendation

**Stop saying**: "orientation" (too broad, causes confusion)

**Start saying**: "aggregation axis" (precise, accurate)

**Code**: Already uses `Axis` enum (good)

**Docs**: Should say "axis selector" not "orientation"

---

## Risk: Semantic Expectation Mismatch

### Who Might Be Confused?

**Users expecting D4 semantics**:
- "I did `(o 'N df)` to time-reverse, why doesn't it work?"
  - Answer: `N` just sets axis=Col (same as `H`). No time-reversal implemented.

- "I did `(o 'S df)` to reverse columns, why doesn't it work?"
  - Answer: `S` just sets axis=Row (same as `Z`). No column-reversal implemented.

**Users expecting IR to respect axis**:
- "I did `(o 'Z df) (dlog 1)` but dlog still works column-wise?"
  - Answer: IR operations ignore axis. Only legacy builtins respect it.

### Mitigation

**Document explicitly**:
1. ✅ This document (ORIENTATION_SEMANTICS_COLLAPSE.md)
2. TODO: Update README.md with current axis semantics
3. TODO: Add warning in `builtin_o` docstring
4. TODO: Consider deprecation warning when IR + axis mismatch detected

---

## What's Next (If D4 Semantics Are Ever Needed)

### Option 1: Add Explicit View Operators (Recommended)

**Don't overload orientation symbols.** Instead, add explicit view operators:

```lisp
(rev-rows table)   ;; Time-reversal (tac) - new rows view
(rev-cols table)   ;; Column-reversal - new columns view
(transpose table)  ;; True transpose (swap axes)
```

**Why**: Orthogonal to axis selection, no semantic confusion.

### Option 2: Resurrect Layout + ORI Field (Complex)

**If true D4 is needed**:
1. Add `Layout` enum back (8 states: NSWE, SNWE, NSEW, SNEW, WENS, EWNS, EWSN, WESN)
2. Make `(o ...)` modify **underlying `TableView.ori`** field (requires blawktrust API)
3. Make operations consult **both** `axis` and `layout`
4. Test all 8×2=16 combinations

**Cost**: High complexity, unclear benefit (nobody uses D4 today).

### Option 3: Keep Current State Forever (Simplest)

**Accept**:
- Axis is a 2-state selector (col/row)
- Symbols `H/N` are aliases for col, `Z/S` are aliases for row
- D4 directionality is out of scope for BLISP

**Benefit**: Simple, correct, sufficient for actual use cases.

---

## If You Want IR to Respect Axis (Optional)

### Problem

IR operations (`dlog`, `shift`, `locf`, `wkd`, etc.) work on raw `blawktrust::TableView` and don't see BLISP's `axis` metadata.

### Solution Sketch

1. **Propagate axis into IR context**:
   - When IR planner receives `Value::TableView(tv)`, extract `tv.axis`
   - Store in IR planning context: `ctx.current_axis`
   - Pass to executor

2. **Executor kernels branch on axis** (where meaningful):
   - **Reductions** (`sum`, `mean`, `std`): Yes, branch on axis
   - **Transforms** (`dlog`, `shift`, `locf`): Probably no (column operations)
   - **Rolling** (`wkd`, `wstd`): Maybe (row-rolling is expensive)

3. **Decide semantics**:
   - What does `(dlog 1)` mean with `axis=Row`? (Lag across columns per row?)
   - Is that useful? (Probably not for time-series)

**Recommendation**: **Defer this.** Current separation (IR = transforms, builtins = aggregations) is clean.

---

## Bottom Line: Current State is COHERENT

### What Works ✅

1. **Bug fixed**: `(o 'Z table)` now changes behavior (axis flip)
2. **Predictable**: `H/N` = colwise, `Z/S` = rowwise (simple 2-state)
3. **Correct for aggregations**: `sum`, `mean`, `std` flip shapes as expected
4. **No breaking changes**: Existing code continues to work (enhanced behavior)

### What's Simplified ⚠️

1. **D4 collapsed**: 8 orientations → 2 axis states (intentional)
2. **IR ignores axis**: Only legacy builtins respect it (acceptable)
3. **Keyword syntax awkward**: Use strings (`":row"`) not keywords (workaround)

### What's Documented 📝

1. ✅ `ORIENTATION_AUDIT_REPORT.md` - Full forensic analysis
2. ✅ `ORIENTATION_FIX_COMPLETE.md` - Implementation details
3. ✅ **This document** - Semantic collapse explanation

### What to Do Next

**STOP**: No more code changes yet.

**DO**:
1. **Freeze semantics** - Document that H/N/Z/S are axis aliases, not D4
2. **Update README** - Add "Aggregation Axis" section explaining current semantics
3. **Test real workload** - Verify actual finance pipelines work with axis
4. **Decide IR + axis** - Later, if needed (probably not)

---

## Explicit Semantic Contract (Don't Break This)

### Current Contract (Phase 0/1)

**BLISP Orientation Symbols**:
- `'H`, `'N`, `'NSWE`, `'SNWE` → Set `axis = Axis::Col`
- `'Z`, `'S`, `'WENS`, `'EWNS` → Set `axis = Axis::Row`
- `":col"`, `":row"` → Direct axis keywords (use strings)

**Affected Operations** (5 legacy builtins):
- `sum`, `mean`, `std`, `cs1-cols`, `ecs1-cols`

**Behavior**:
- `Axis::Col` → Aggregate down rows per column (default)
- `Axis::Row` → Aggregate across columns per row

**NOT Supported**:
- ❌ D4 directionality (time-reversal, column-reversal)
- ❌ Diagonal traversals (underscore variants)
- ❌ IR axis awareness (IR ignores axis field)
- ❌ Lisp keyword syntax (`:row` without quotes)

**This is the contract. Don't change it without updating this document.**

---

## Final Word: This Is Safe and Correct

The fix:
- ✅ Removes dead code (`Layout` enum)
- ✅ Makes symbols work (they change behavior now)
- ✅ Simplifies semantics (2 states instead of 8)
- ✅ Documents what was lost (D4 directionality)
- ✅ Identifies scope limits (legacy builtins only)

**It's a good Phase 0/1 fix.** Don't regret the simplification - embrace it as intentional scope reduction.

When someone asks "why doesn't `(o 'N)` time-reverse?", point them to this document:

> "In BLISP Phase 0/1, orientation symbols are **aggregation axis selectors**, not D4 direction operators. `N` sets axis=Col, same as `H`. Time-reversal is not implemented. If you need it, use `(rev-rows table)` when we add view operators later."

---

**End of Semantic Collapse Documentation**
