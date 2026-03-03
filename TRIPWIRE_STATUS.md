# Tripwire Test Status

**Date:** 2026-03-03
**Status:** ✅ Integrity Checks Pass, ⚠️ Reality Check Shows Gaps

---

## Tripwire Tests (All Passing)

### 1. ✅ YAML ↔ Enum Validation
**Test:** `test_ir_names_match_code`
**Status:** PASS
- All `ir:` values in YAML match actual enum variants in src/ir.rs
- No invented canonical names possible

### 2. ✅ No Alias Collisions
**Test:** `test_no_alias_collisions`
**Status:** PASS
- No alias maps to multiple canonical operations

### 3. ✅ No Cross-Layer Duplicates (STRICT)
**Test:** `test_no_duplicate_alias_legacy_tokens`
**Status:** PASS (enforced - test fails on violation)
- Policy: Each token belongs to exactly ONE layer (L1 or L2)
- **Fixed:** Removed 33 duplicates from YAML
- Examples fixed: `+`, `-`, `dlog`, `wkd`, `file`, `stdin`, `sum`, etc.

### 4. ✅ No Placeholder Legacy Tokens (STRICT)
**Test:** `test_no_placeholder_legacy_tokens`
**Status:** PASS (enforced - test fails on violation)
- Never use `-` or empty string as placeholder
- Display shows empty instead of `-` for operations with no legacy tokens

### 5. ✅ JSON Round-Trip
**Test:** `test_json_round_trip_exposed_aliases`
**Status:** PASS
- 84 aliases serialize/deserialize correctly

---

## Reality Check: blisp dic --check-resolve

### Current Status: ⚠️ 53 out of 103 names fail to resolve (51%)

**Command:** `./target/release/blisp dic --check-resolve`

### High-Risk Entries (All VERIFIED ✅):
- `>-col`, `>-cols` → OK(BUILTIN) ✅
- `cs1-col`, `cs1-cols` → OK(BUILTIN) ✅
- `diff-col`, `diff-cols` → OK(BUILTIN) ✅
- `wzs`, `wz0`, `wz0-cols` → OK(BUILTIN) ✅
- `ur-col`, `ur-cols` → OK(BUILTIN) ✅
- `shift-col` → OK(BUILTIN) ✅
- `locf-cols` → OK(BUILTIN) ✅

**Conclusion:** All `-col` and `-cols` suffixed operations work correctly.

### Names That FAIL to Resolve (53 total)

These are aliases listed in YAML but not registered in runtime:

#### Category 1: Word Forms of Operators (12 failures)
**YAML says they work, but only symbols are registered:**
- `add`, `sub`, `mul`, `div` → Only `+`, `-`, `*`, `/` work
- `eq`, `neq`, `gt`, `lt`, `lte`, `gte` → Only `==`, `!=`, `>`, `<`, `<=`, `>=` work

#### Category 2: Math Functions (7 failures)
**IR enums exist but not registered as builtins:**
- `abs`, `exp`, `inv`, `log`, `ln`, `ret`, `sqrt`

**Explanation:** These are IR operations but have no builtin registration.
They can only be used via IR planner, not directly.

#### Category 3: Shift/Lag Friendly Names (8 failures)
**Only -col/-cols variants registered:**
- `dlog`, `dlog-obs`, `dlog-ofs` → Only `dlog-col` works
- `lag`, `lag-obs` → Not registered
- `shift` → Only `shift-col` works
- `cs1`, `cumsum` → Only `cs1-col`, `cs1-cols` work

#### Category 4: Rolling Operations Friendly Names (14 failures)
**Only underlying implementations registered:**
- `locf`, `ffill` → Only `locf-cols` works
- `roll-mean`, `rolling-mean`, `rolling-mean-min`, `rolling-mean-excl` → Not registered
- `roll-std`, `rolling-std`, `rolling-std-min`, `rolling-std-excl` → Not registered
- `roll-z`, `rolling-zscore` → Not registered

**Explanation:** The YAML lists user-friendly names, but only the internal `-cols`
versions or underlying primitives are registered.

#### Category 5: Other Missing (12 failures)
- `demean` → Not registered (probably planned)
- `left-join` → Not registered (planned)
- `orient` → Not registered (but `o` works)
- `nrow` → Not registered
- `length` → Not registered (but `len` works)
- `keep` → Not registered
- `load`, `read-csv` → Not registered (but `file` works)
- `sharpe` → Not registered (planned)
- Plus a few others

---

## Policy Decision Required

**Question:** What should happen with the 53 unregistered names?

### Option A: Register Missing Names (Make YAML True)
**Pros:** Dictionary becomes accurate, user-friendly names work
**Cons:** More code changes, need to implement dispatchers

**Example:**
```rust
rt.register_builtin("add", builtin_add);  // Alias for +
rt.register_builtin("log", builtin_log);  // Math function
rt.register_builtin("rolling-mean", builtin_rolling_mean);  // Friendly name
```

### Option B: Remove From YAML (Make YAML Accurate)
**Pros:** Dictionary shows only what actually works
**Cons:** Removes aspirational/planned names

**Example:** Remove `log`, `rolling-mean`, `cs1` from aliases

### Option C: Hybrid Approach (Recommended)
**Pros:** Clear separation of working vs planned
**Cons:** More complex schema

**Actions:**
1. **Keep** operator word forms in YAML, register them (low cost)
2. **Keep** math functions in YAML, register them (medium cost)
3. **Move** unregistered rolling/table ops to `planned:` field or notes
4. **Document** which names are IR-only vs builtin

### Option D: Status Quo (Not Recommended)
Keep YAML as-is, accept that 51% don't resolve.
**Risk:** Users trust dictionary, get confused when names don't work.

---

## Acceptance Criteria

**Current Goal:** Zero failures in `blisp dic --check-resolve`

**How to achieve:**
1. Choose policy (A/B/C above)
2. Either register missing names OR remove from YAML
3. Run `blisp dic --check-resolve` until all show OK
4. Add CI test that enforces zero FAIL

**Stretch Goal:** Add routing checks (`blisp run --explain`) to verify
IR-marked operations actually use IR backend.

---

## Test Summary

**Tripwire Tests:** 5/5 passing (strict enforcement)
**Reality Check:** 50/103 resolve (49% success rate)
**Target:** 103/103 resolve (100% success rate)

**Next Action:** Choose policy for 53 unregistered names.
