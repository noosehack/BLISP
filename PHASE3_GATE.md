# Phase 3 Gate Checklist

**DO NOT START Phase 3 (Macro Normalization + IR) until ALL boxes checked.**

This gate prevents optimizing wrong semantics and backwards-compat hell.

---

## Checklist

- [x] **docs/contracts.md exists**
  - Frame invariants documented
  - Index equality rules frozen
  - Duplicate handling (last-wins) specified
  - NA semantics formalized
  - mapr contracts explicit
  - Stable API surface locked (3 primitives)

- [x] **Property tests exist**
  - [x] mapr idempotence: `mapr(mapr(x,y), y) == mapr(x,y)`
  - [x] mapr identity: `if x.index == y.index then mapr(x,y) == x`
  - [x] mapr monotonicity: `mapr(x,y).nrows == y.nrows` always
  - [x] No forward-looking bias: mapr never invents non-NA data
  - [x] Arc preservation: I1-I3 for all numeric ops
  - **Status:** 5/5 tests passing

- [x] **Performance benchmarks exist**
  - [x] dlog on 5M cells (some NA)
  - [x] reindex_by sorted (best case)
  - [x] reindex_by unsorted (worst case)
  - [x] reindex_by sparse (50% hit)
  - **Location:** `benches/perf_guardrails.rs`
  - **Contract:** No >20% regression without justification

- [x] **NA policy formalized**
  - [x] Short-term: `NA = f64::NAN` sentinel
  - [x] Contract: Use `is_na()` helper (no direct NaN checks)
  - [x] Aggregation: Ignore NA (empty → NA)
  - [x] Future: Arrow-style validity bitmaps reserved
  - **Documented:** `docs/contracts.md` § NA Semantics

- [x] **API surface locked**
  - [x] 3 stable primitives only:
    - `map_numeric_preserve_tags()`
    - `reindex_by()`
    - `mapr()`
  - [x] No near-duplicates allowed (mapr2, mapr_loose, etc.)
  - [x] Schema-changing ops explicitly marked
  - **Enforced:** Code review + contracts.md

---

## Gate Status: ✅ PASSED

**All boxes checked.** Phase 3 may proceed.

**Important:**
- Run `cargo test property_` before ANY Phase 3 commit
- Run `cargo bench perf_guardrails` weekly to detect regressions
- Update contracts.md for ANY semantic changes (requires explicit amendment)

---

## Post-Gate: Next Valuable Feature

**Recommended after gate:** Implement `asofr()` (asof join)

**Why:**
- Financial time-series need "value at-or-before"
- Separate operation (never overload mapr)
- Explicitly marked "no forward-looking"

**Contract:**
```rust
/// asofr(x, y) - Right asof join
/// Returns x values at-or-before y timestamps
/// Output index = y.index (like mapr)
/// Explicitly Ft-measurable (no forward-looking)
asofr(x: Frame, y: Frame) -> Frame
```

**DO NOT:** Add asof semantics to mapr. Keep semantics separate.

---

**Gate passed:** 2026-02-20
**Enforcement:** CI checks + code review
