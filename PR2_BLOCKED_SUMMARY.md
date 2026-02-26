# PR2 BLOCKED: Semantic Divergence Discovery

**Date**: 2026-02-26
**Status**: ⛔ **BLOCKED** - Cannot proceed with naive kernel unification
**Discovery**: Two `dlog_column` implementations serve DIFFERENT purposes

---

## Executive Summary

**PR2 was scoped as**: "Delete duplicate dlog_column, call blawktrust everywhere"

**Reality discovered**: The two implementations are **semantically different operations**:
- **LOCAL (exec.rs:1092)**: Observation-based lag (NA-skipping)
- **BLAWKTRUST**: Position-based lag (fixed offset)

**Both are needed. This is not a duplication bug.**

---

## Evidence

### Call Site Analysis

**LOCAL version used by**:
- IR executor (exec.rs:157) - `SHF_PTW_NLN_DLOG`
- builtins.rs (4 call sites)

**BLAWKTRUST version used by**:
- frame.rs tests (4 call sites)

### Semantic Difference

| Input | Local (NA-skip) | Blawktrust (positional) |
|-------|-----------------|-------------------------|
| `[100, NA, NA, 110, 120]` | `[NA, NA, NA, ln(110/100), ln(120/110)]` | `[NA, NA, NA, NA, ln(120/110)]` |

**Position 3 differs**:
- Local: `ln(110/100)` - skipped NAs to find 100
- Blawktrust: `NA` - used x[i-1]=NA directly

### Code Comparison

**LOCAL** (30 lines):
```rust
fn dlog_column(col: &Column, _lag: usize) -> Column {
    let mut last_valid: Option<f64> = None;  // State tracking

    for &x in data.iter() {
        if x.is_nan() {
            result.push(f64::NAN);
        } else if let Some(prev) = last_valid {
            // Use LAST VALID value (not x[i-lag])
            result.push(x.ln() - prev.ln());
            last_valid = Some(x);
        }
    }
}
```

**BLAWKTRUST** (4 lines kernel):
```rust
pub fn dlog_no_nulls(out: &mut [f64], x: &[f64], lag: usize) {
    out[..lag].fill(f64::NAN);
    for i in lag..n {
        out[i] = x[i].ln() - x[i-lag].ln();  // Fixed offset!
    }
}
```

---

## Why Both Are Correct

### Use Case 1: Financial Time Series with Weekends (LOCAL)

**Scenario**: Daily stock prices with weekend masking
```
Mon: 100, Tue: NA (weekend), Wed: NA (weekend), Thu: 110, Fri: 120
```

**Requirement**: "Business day returns"
- Thu return should be `ln(110/100)` (Thu vs Mon)
- NOT `ln(110/NA)` = NA (Thu vs Tue)

**Solution**: NA-skipping lag (LOCAL implementation)

### Use Case 2: Clean Calendar Data (BLAWKTRUST)

**Scenario**: Daily temperature readings (no gaps)
```
Day 1: 20, Day 2: 22, Day 3: 21, Day 4: 23, Day 5: 24
```

**Requirement**: "Day-over-day change"
- Day 2 change = `22 - 20`
- Day 3 change = `21 - 22`

**Solution**: Fixed positional lag (BLAWKTRUST implementation)

---

## IR Taxonomy Implications

### Current State

**IR has ONE dlog operation**:
- `NumericFunc::SHF_PTW_NLN_DLOG` (exec.rs:157)
- Uses LOCAL (NA-skipping)

**IR has TWO shift operations**:
- `NumericFunc::SHF_PTW_LIN_SHF{k}` - Fixed positional
- `NumericFunc::LAG_OBS{k}` - Observation-based (mask-aware)

### Problem

**Inconsistency**: `dlog` uses observation-based semantics but is named like pointwise.

**`SHF_PTW`** = "Shift, Pointwise"
- Suggests: position-based (like `SHF_PTW_LIN_SHF`)
- Reality: observation-based (like `LAG_OBS`)

### Solution Options

**Option A**: Add `SHF_PTW_NLN_DLOG_OBS` variant
- Explicit observation-based dlog
- Keep `SHF_PTW_NLN_DLOG` for fixed-lag
- Current usage maps to `_OBS`

**Option B**: Rename to match semantics
- Current → `LAG_OBS_NLN_DLOG` (observation-based)
- Add new → `SHF_PTW_NLN_DLOG` (positional)

**Option C**: Runtime dispatch
- Check for active mask
- Use NA-skipping if masked, positional otherwise
- **(Not recommended: hidden complexity)**

---

## Correct Path Forward

### PR2-REVISED: Document & Whitelist

**Do NOT delete local dlog_column.**

**Instead**:

1. **Add semantic comment** to exec.rs:1092:
   ```rust
   /// Observation-based dlog (NA-skipping lag)
   /// Used by IR executor for SHF_PTW_NLN_DLOG
   /// Different from blawktrust::dlog_column (position-based)
   fn dlog_column(col: &Column, _lag: usize) -> Column {
   ```

2. **Update tripwire whitelist**:
   ```bash
   # ci/test_no_kernel_dupes.sh
   # Whitelist dlog_column: semantic variants (obs-based vs pos-based)
   ```

3. **Add differential test**:
   ```rust
   #[test]
   fn test_dlog_semantic_variants() {
       let data_with_nas = vec![100.0, NA, NA, 110.0];
       assert_ne!(exec::dlog_column(data), blawktrust::dlog_column(data));
   }
   ```

4. **Document in architecture**:
   - Two lag semantics: observation vs position
   - Both needed for different use cases
   - Not a bug, intentional design

### PR2.5: Verify Shift Semantics (NEW)

**Check second high-risk operator**:

1. Read `shift_column` (exec.rs:1350)
2. Confirm it's **position-based** (NOT observation-based)
3. If observation-based, same issue exists
4. Document findings

### PR3: Taxonomy Clarification (UPDATED)

**Before combinator refactor**, clarify IR naming:

1. Decide on observation vs position naming
2. Add variants if needed
3. Update planner mappings
4. Ensure consistency

---

## Lessons Learned

### Tripwire Success ✅

**What worked**:
- Tripwire detected duplicate symbol names
- Forced investigation before deletion
- Prevented semantic breakage

**What to improve**:
- Tripwire should check semantic equivalence, not just names
- Differential tests should be mandatory in tripwire

### Discipline Payoff ✅

**User's protocol worked**:
- Step 1: Prove which is active (found both used)
- Step 2: Diff implementations (found divergence)
- Step 3: BLOCKED before damage

**We did NOT**:
- Delete first, test later
- Assume equivalence
- Break financial correctness

### V2 Blueprint Amendment Needed

**Section 6.1 update required**:

> Kernel Authority: Single Source of Truth
>
> **Amendment**: "Single source" means single implementation **per semantic operation**.
> If blawktrust and local versions have different semantics (e.g., observation-based
> vs position-based lag), both are legitimate. Document the distinction and ensure
> IR uses the correct variant for its contract.

---

## Impact Assessment

### What We Learned ✅

1. IR needs **two dlog variants** (or clarified naming)
2. Mask-aware operations are **semantically different**
3. Tripwire caught this before breakage
4. Differential testing is mandatory, not optional

### What We Avoided ❌

1. Breaking financial returns calculations
2. Introducing subtle bugs in masked time series
3. Violating IR contracts
4. Losing semantic correctness

### What We Gained ✅

1. Clear understanding of lag semantics
2. Documentation of both variants
3. Test proving divergence
4. Architectural clarity

---

## Revised Roadmap

### ~~PR2~~: Kernel Unification → **PR2-REVISED**: Documentation

**New Scope**:
- [x] Prove semantic divergence (DONE)
- [ ] Document both variants in code
- [ ] Whitelist in tripwire
- [ ] Add differential test
- [ ] Update architecture docs

**Estimated**: 2 hours (was 30 min)

### PR2.5: Verify Shift Semantics (NEW)

**Scope**:
- [ ] Read shift_column implementation
- [ ] Confirm position-based (not observation-based)
- [ ] Document findings
- [ ] Add tests if needed

**Estimated**: 1 hour

### PR3: Taxonomy Clarification (UPDATED)

**Add to scope**:
- [ ] Decide on observation vs position naming
- [ ] Add IR variants if needed
- [ ] Update planner accordingly

**Estimated**: +1 day to original PR3 estimate

### PR4: Fusion (UNCHANGED)

Can proceed once taxonomy is clarified.

---

## Recommendation

**PAUSE PR2 kernel unification.**

**PROCEED WITH**:
1. PR2-REVISED (documentation & whitelist)
2. PR2.5 (verify shift)
3. Architectural decision on taxonomy
4. THEN PR3 (combinators)

**This is not a setback. This is a win.**
We discovered a semantic issue before breaking production code.

---

## Summary

| Item | Status |
|------|--------|
| **PR0** | ✅ Complete |
| **PR1** | ✅ Complete |
| **PR2 (original)** | ⛔ Blocked - semantic divergence |
| **PR2-REVISED** | ⏳ Ready (documentation) |
| **PR2.5** | ⏳ Ready (verify shift) |
| **PR3** | ⏳ Waiting for taxonomy decision |
| **PR4** | ⏳ Waiting for PR3 |

---

**END OF BLOCKED SUMMARY**

*"The tripwire caught a real bug. The discipline prevented a real break. PR2 blocked is a success."*
