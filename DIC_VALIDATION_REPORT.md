# Dictionary Validation Report

**Date:** 2026-03-03
**blisp Version:** 0.2.0
**Status:** ⚠️ Critical Issues Found

---

## Summary

Implemented comprehensive validation checks for the operation dictionary as recommended:

1. ✅ **Integrity checks** (fast, deterministic)
2. ✅ **Reality checks** (does CLI actually accept these names?)
3. ⏳ **Routing checks** (TODO: is it IR or legacy at runtime?)

**Key Finding:** 55 out of 136 names (40%) listed in dictionary **fail to resolve** in runtime.

---

## 1. Integrity Checks ✅

### 1.1 YAML ↔ Enum Validation ✅

**Test:** `test_ir_names_match_code`
**Status:** PASS
**Result:** All `ir:` values in YAML match actual enum variants in code.

```
✅ All IR operations validated against src/ir.rs enums
✅ NumericFunc: 20 operations
✅ BinaryFunc: 10 operations
✅ Source: 2 operations
```

### 1.2 No Alias Duplication ✅

**Test:** `test_no_alias_collisions`
**Status:** PASS
**Result:** No alias maps to multiple canonicals.

### 1.3 No Legacy Token Duplication ✅

**Test:** (covered by `test_no_alias_collisions`)
**Status:** PASS
**Result:** No legacy token maps to multiple canonicals.

### 1.4 JSON Round-Trip ✅

**Test:** `test_json_round_trip_exposed_aliases`
**Status:** PASS
**Result:** JSON serialization/deserialization preserves all 84 aliases.

```
✅ JSON round-trip: 84 aliases verified
```

### 1.5 Cross-Layer Duplication ⚠️

**Test:** `test_no_duplicate_alias_legacy_tokens`
**Status:** WARNING (not enforced yet)
**Result:** 33 operations have tokens appearing in BOTH aliases and legacy_tokens.

**Examples of duplication:**
- Operators: `+`, `-`, `*`, `/`, `<`, `>`, `<=`, `>=`, `==`, `!=`
- Operations: `dlog`, `wkd`, `file`, `stdin`, `sum`, `mean`, `std`, `diff`, `col`, `ur`, `wzs`, `type-of`

**Recommendation:** Each name should belong to exactly one layer unless intentionally duplicated for transition period. Document which duplicates are intentional.

---

## 2. Reality Checks ⚠️

### 2.1 blisp dic --check-resolve ❌

**Command:** `blisp dic --check-resolve`
**Status:** **CRITICAL FAILURES**
**Result:** 55 out of 136 names (40.4%) fail to resolve.

#### Names That FAIL to Resolve (Not Registered in Runtime)

**Arithmetic (word forms):**
- `add`, `sub`, `mul`, `div` → Only operators `+`, `-`, `*`, `/` are registered

**Comparison (word forms):**
- `eq`, `neq`, `gt`, `lt`, `lte`, `gte` → Only operators `==`, `!=`, `>`, `<`, `<=`, `>=` are registered

**Math operations:**
- `abs`, `exp`, `inv`, `log`, `ln`, `ret`, `sqrt` → NOT registered

**Shift/lag operations:**
- `dlog`, `dlog-obs`, `dlog-ofs` → Only `dlog-col` registered
- `lag`, `lag-obs` → NOT registered
- `shift` → Only `shift-col` registered
- `cumsum`, `cs1` → Only `cs1-col`, `cs1-cols` registered

**Rolling operations:**
- `locf`, `ffill` → Only `locf-cols` registered
- `roll-mean`, `rolling-mean`, `rolling-mean-min`, `rolling-mean-excl` → NOT registered
- `roll-std`, `rolling-std`, `rolling-std-min`, `rolling-std-excl` → NOT registered
- `roll-z`, `rolling-zscore` → NOT registered

**Table operations:**
- `demean` → NOT registered
- `left-join` → NOT registered
- `orient` → NOT registered
- `nrow` → NOT registered
- `length` → NOT registered

**I/O operations:**
- `load`, `read-csv` → Only `file` registered

**Finance:**
- `sharpe` → NOT registered (alias for `ur`)

**Masks:**
- `keep` → NOT registered

#### Names That DO Resolve ✅

**Operators (81 successful resolutions):**
- `+`, `-`, `*`, `/`, `<`, `>`, `<=`, `>=`, `==`, `!=` ✅
- Vectorized forms: `>-col`, `>-cols` ✅

**Column operations:**
- `diff`, `diff-col`, `diff-cols` ✅
- `dlog-col` ✅
- `shift-col` ✅
- `cs1-col`, `cs1-cols` ✅
- `locf-cols` ✅

**Aggregations:**
- `sum`, `sum0`, `mean`, `mean0`, `std`, `std0` ✅

**Table operations:**
- `col`, `select`, `setcol`, `withcol`, `w` ✅
- `mapr` (left join) ✅
- `xminus` ✅
- `o` (orient) ✅
- `ro` (relative orient) ✅

**I/O:**
- `file`, `stdin`, `save`, `print` ✅

**Rolling:**
- `wstd`, `wstd0` ✅
- `wz0`, `wz0-cols`, `wzs` ✅

**Masks:**
- `wkd`, `w5` ✅

**Utility:**
- `len`, `type-of` ✅

---

## 3. Routing Checks ⏳

### 3.1 blisp run --explain (TODO)

**Status:** Not yet implemented
**Recommendation:** Add `--explain` or `--trace` flag to show planned graph:
- For each op: token, resolved canonical, backend (IR / BUILTIN / LEGACY_FALLBACK)

### 3.2 Automated Routing Smoke Tests (TODO)

**Status:** Not yet implemented
**Recommendation:** Create micro test suite:

```lisp
(+ 1 2)          → must show IR: ADD
(log 1)          → must show IR: LOG
(> 1 2)          → must show IR: GTR
(wkd (file ...)) → must show IR: MSK_WKE or BUILTIN (design choice)
```

---

## 4. Spot-Check Anomalies

### 4.1 Operators with "-" as legacy token ⚠️

**Issue:** Operators like `-` appear in BOTH aliases and legacy_tokens
**Example:** `SUB` has `aliases: ["-", sub]` AND `legacy_tokens: ["-"]`
**Recommendation:** Remove duplication - operators should only be in aliases layer.

### 4.2 wkd/w5 redundancy ⚠️

**Issue:** `wkd` appears as both alias and legacy token
**Current:**
- `aliases: [wkd, weekday]`
- `legacy_tokens: [wkd, w5]`

**Recommendation:** Keep `wkd` in aliases only, keep `w5` in legacy tokens only.

### 4.3 >-col, >-cols resolution ✅

**Status:** VERIFIED - both resolve correctly
**Result:**
- `>-col` → OK(BUILTIN)
- `>-cols` → OK(BUILTIN)

### 4.4 cs1 → SHF_PFX_LIN_SUM semantics ⚠️

**Issue:** YAML maps `cs1` to `SHF_PFX_LIN_SUM` but `cs1` doesn't resolve
**Actual:** Only `cs1-col` and `cs1-cols` are registered
**Recommendation:** Either:
1. Register `cs1` as alias for `cs1-cols` (table version), OR
2. Update YAML to mark `cs1` as unregistered/planned

---

## Recommendations

### Priority 1: Fix Critical Resolution Failures ❗

55 names claim to exist but don't resolve. Options:

**A. Register Missing Names (recommended for user-facing aliases):**
- Register word forms: `add`, `sub`, `mul`, `div`, `eq`, `neq`, `gt`, `lt`, `lte`, `gte`
- Register math: `abs`, `exp`, `inv`, `log`, `ln`, `ret`, `sqrt`
- Register rolling: `rolling-mean`, `rolling-std`, etc.

**B. Update YAML (recommended for planned/future operations):**
- Mark unimplemented operations clearly
- Move unregistered names to `planned:` field or remove from aliases

**C. Hybrid Approach (recommended):**
- Core operations (math, comparison): Register word forms as aliases to operators
- Rolling operations: Register canonical names that dispatch to correct kernels
- Planned operations: Remove from aliases, document in separate section

### Priority 2: Remove Cross-Layer Duplicates

33 operations have tokens in both aliases and legacy_tokens:
- Review each case: is duplication intentional (transition period)?
- If not, move to single layer
- Document intentional duplicates

### Priority 3: Add Routing Checks

Implement `blisp run --explain` or `blisp run --trace`:
- Show which backend handles each operation (IR / BUILTIN)
- Verify IR operations actually use IR backend
- Add smoke tests for critical operations

---

## Test Coverage

**New tests added:**

1. `test_json_round_trip_exposed_aliases` - JSON serialization integrity
2. `test_no_duplicate_alias_legacy_tokens` - Cross-layer duplication check
3. **Reality check via CLI:** `blisp dic --check-resolve`

**Test status:** 158 tests passing, 0 failures

---

## Implementation Details

### New CLI Command

```bash
# Check if all names in dictionary actually resolve
blisp dic --check-resolve

# Output format
Name                           Expected (YAML)                Actual (Runtime)
----------------------------------------------------------------------------------------------------
abs                            ABS                            FAIL(unknown)
+                              ADD                            OK(BUILTIN)
dlog-col                       SHF_PTW_OBS_NLN_DLOG           OK(BUILTIN)
```

### New Functions in src/dic.rs

- `check_resolve(name: &str) -> ResolveStatus` - Test if name resolves
- `print_resolution_check()` - Generate resolution report
- View::CheckResolve - New view type

---

## Conclusion

The dictionary validation successfully identified critical issues:

1. ✅ **Internal consistency verified** - YAML matches code enums
2. ❌ **External consistency broken** - 40% of names don't resolve in runtime
3. ⚠️ **Layer confusion** - 33 tokens appear in multiple layers

**Next steps:**
1. Decide registration policy for missing names (A/B/C above)
2. Clean up cross-layer duplicates
3. Add routing checks to verify IR backend usage
4. Update documentation to reflect actual vs. planned operations

The `blisp dic --check-resolve` command should be run regularly in CI to prevent dictionary drift.
