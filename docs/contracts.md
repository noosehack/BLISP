# BLADE Semantic Contracts (Non-Negotiable)

**Status:** Frozen as of Phase 2 (2026-02-20)
**Purpose:** Prevent alignment layer from becoming "whatever the last caller expected"

These are the **immutable contracts** that all future phases must preserve.

---

## Frame Invariants

### Tags Are Immutable (Arc-Shared)

```rust
// REQUIRED: Tags always behind Arc
struct Frame {
    tags: Arc<Tags>,  // Never Option, never mut
    cols: Vec<ColData>,
    nrows: usize,
}
```

**Contract:**
- Tags are **immutable** once created
- Tags are **Arc-shared** (zero-copy propagation)
- Numeric operations **preserve tag Arcs** (unless explicitly schema-transforming)

**Verification:**
```rust
// Any numeric op MUST preserve Arc pointers:
let result = map_numeric_preserve_tags(&frame, f);
assert!(Arc::ptr_eq(&frame.tags.index, &result.tags.index));
assert!(Arc::ptr_eq(&frame.tags.colnames, &result.tags.colnames));
```

### Numeric Ops Preserve Tags

**Rule:** All operations using `map_numeric_preserve_tags()` MUST preserve:
- **I1:** `output.tags.index == input.tags.index` (Arc pointer equality)
- **I2:** `output.tags.colnames == input.tags.colnames` (Arc pointer equality)
- **I3:** `output.nrows == input.nrows`

**Schema-Transforming Ops** (explicitly documented as exceptions):
- `xminus` / `xdiv` / `xplus` / `xmult` - Change colnames (pairwise combinations)
- `select` / `project` / `rename` - Explicit column manipulation
- Future: `groupby` / `pivot` - Aggregation reshaping

---

## Index Equality

### No Implicit Coercion

```rust
// FORBIDDEN: These are DIFFERENT types
Date(18000)      ≠ Timestamp(1546300800000000000)
Date(18000)      ≠ String("2020-01-01")
Timestamp(...)   ≠ String("2020-01-01 00:00:00")
```

**Contract:**
- `IndexColumn::Date` vs `Timestamp` vs `String` are **incompatible**
- No automatic conversion in joins/alignment
- **Error** if types mismatch (not silent NA)

### Exact Match Join Only

**Contract for `mapr` / `reindex_by`:**
- Matching uses **exact equality** (via `IndexKey` hash/eq)
- **NO asof semantics** (no "at-or-before" matching)
- **NO fuzzy matching** (no tolerance windows)

**For asof:** Create separate `asofr()` operation (Phase 4+)

---

## Duplicate Index Handling

### **BOLD RULE: Duplicate keys in source → LAST WINS**

```rust
// Source: [(A, 100), (A, 200), (B, 300)]
// Target: [A, B, C]
// Result: [A: 200, B: 300, C: NA]  // Last A wins (200)
```

**Contract:**
- If source has duplicate index values, **last occurrence wins**
- HashMap insertion overwrites previous value
- This is **deterministic** given input order
- **No error** on duplicates (silent last-wins)

**Rationale:** Financial time-series often have corrections/amendments

**Alternative (rejected):** Error on duplicates - too strict for real data

---

## NA Semantics

### NA Representation (Phase 1-3)

**Contract:**
- `NA = f64::NAN` sentinel for `F64` columns
- `NA = NULL_DATE` (i32::MIN) for `Date` columns
- `NA = NULL_TIMESTAMP` (i64::MIN) for `Timestamp` columns
- **Never use direct NaN checks** - always use `is_na()` helper

```rust
// FORBIDDEN:
if x.is_nan() { ... }

// REQUIRED:
if is_na(x) { ... }  // Abstracts sentinel representation
```

### Aggregation with NA

**Contract:**
- **Ignore NA in aggregations** (sum, mean, std, etc.)
- Empty after NA removal → Result is NA
- **Never silently propagate NA to non-NA values**

```rust
// Examples:
sum([1.0, NA, 3.0])    = 4.0   // Ignore NA
sum([NA, NA, NA])      = NA    // All NA → NA
mean([1.0, NA, 3.0])   = 2.0   // Ignore NA
```

**Exception:** Comparison ops propagate NA (like SQL NULL):
```rust
[1.0, NA, 3.0] > 2.0  →  [0.0, NA, 1.0]  // NA stays NA
```

### Future-Proofing (Phase 4+)

**Reserved:** Move to Arrow-style validity bitmaps
```rust
trait Column {
    fn is_valid(&self, i: usize) -> bool;  // Future API
    fn get(&self, i: usize) -> Option<T>;  // Future API
}
```

**Contract:** All kernel code MUST use abstraction layer (no direct NaN checks) to enable future migration.

---

## mapr Semantics (RIGHT OUTER JOIN)

### Core Contract

```rust
mapr(x: Frame, y: Frame) -> Frame
```

**Invariants (MUST hold for all inputs):**

1. **Output index = y's index** (Arc pointer preserved)
   ```rust
   let result = mapr(x, y);
   // REQUIRED (if index type matches):
   Arc::ptr_eq(&result.tags.index, &y.tags.index)
   ```

2. **Output colnames = x's colnames** (Arc pointer preserved)
   ```rust
   Arc::ptr_eq(&result.tags.colnames, &x.tags.colnames)
   ```

3. **Output nrows = y's nrows** (always)
   ```rust
   assert_eq!(result.nrows(), y.nrows());
   ```

4. **Missing rows → NA row**
   ```rust
   // If y.index[i] not in x.index → result.row[i] = all NA
   ```

### Semantics

**SQL equivalent:**
```sql
SELECT y.date, x.*
FROM y
LEFT JOIN x ON x.date = y.date
```

**kdb equivalent:**
```q
x lj y  // where y is keyed table
```

### Properties (MUST pass property tests)

1. **Idempotence:**
   ```rust
   mapr(mapr(x, y), y) == mapr(x, y)  // Numeric equality + Arc preservation
   ```

2. **Identity:**
   ```rust
   // If x.index == y.index (same values):
   mapr(x, y) == x  // Numeric equal, Arcs preserved
   ```

3. **Monotonicity:**
   ```rust
   mapr(x, y).nrows == y.nrows  // Always, regardless of x
   ```

4. **No forward-looking bias:**
   ```rust
   // mapr NEVER invents non-NA data
   // All non-NA values in result exist in source x
   ```

---

## asofr Semantics (RIGHT OUTER ASOF JOIN)

### Core Contract

```rust
asofr(x: Frame, y: Frame) -> Frame
```

**Definition:** Reindex x onto y using "last observation carried backward in time" (at-or-before).

**Invariants (MUST hold for all inputs):**

1. **Output index = y's index** (Arc pointer preserved)
   ```rust
   let result = asofr(x, y);
   Arc::ptr_eq(&result.tags.index, &y.tags.index)
   ```

2. **Output colnames = x's colnames** (Arc pointer preserved)
   ```rust
   Arc::ptr_eq(&result.tags.colnames, &x.tags.colnames)
   ```

3. **Output nrows = y's nrows** (always)
   ```rust
   assert_eq!(result.nrows(), y.nrows());
   ```

4. **At-or-before semantics**
   ```rust
   // For each t in y.index, pick t' = max{x.index ≤ t}
   // If no such t', output row is NA
   ```

### Semantics

**SQL equivalent:**
```sql
SELECT y.date, x.*
FROM y
LEFT JOIN LATERAL (
  SELECT * FROM x
  WHERE x.date <= y.date
  ORDER BY x.date DESC
  LIMIT 1
) ON true
```

**kdb equivalent:**
```q
aj[`date; y; x]  // asof join on date column
```

### Non-Negotiable Rules

1. **Right side dominates:** Output has exactly y.nrows rows
2. **At-or-before only:** For each `t` in `y.index`, pick `t' = max{x.index ≤ t}`
3. **No forward-looking:** NEVER use `x.index > t` (bias-free by construction)
4. **Missing → NA:** If no `x.index ≤ t`, entire row is NA
5. **Duplicates in x:** Last wins (consistent with mapr)
6. **Duplicates in y:** Each row resolved independently
7. **Monotonicity:** If y.index sorted, selected x pointer is monotone nondecreasing
8. **Index type must match:** `Date` only joins `Date`, etc. (no coercion)

### Properties (MUST pass property tests)

1. **Identity with matching indices:**
   ```rust
   // If x.index == y.index (same values):
   asofr(x, y) == mapr(x, y) == x  // Numerically equal
   ```

2. **No forward-looking bias (STRONGER than mapr):**
   ```rust
   // Construct x with "future spike" at t+100
   // Ensure it NEVER appears at earlier y times
   ```

3. **Monotonicity (for sorted y):**
   ```rust
   // For sorted y.index, selected source pointer never decreases
   ```

4. **Idempotence:**
   ```rust
   asofr(asofr(x, y), y) == asofr(x, y)
   ```

5. **Equivalence to naive scan (small sizes):**
   ```rust
   // Reference O(n²) implementation matches optimized version
   ```

### Performance Contract

**Fast path (sorted indices):** O(nx + ny) two-pointer merge
**Fallback (unsorted):** O(nx log nx + ny log ny) or hashmap

**Benchmarks MUST NOT regress by >20%:**
- asofr sorted x, sorted y (5M cells)
- asofr unsorted x, sorted y
- asofr sparse x, dense y (intraday grid)
- asofr with heavy duplicates in x

---

## Stable API Surface (Frozen)

### Core Primitives (Public API)

These are **stable** and will not change semantics:

```rust
/// Apply function to numeric columns, preserve tags (I1-I3)
pub fn map_numeric_preserve_tags<F>(frame: &Frame, f: F) -> Frame
where F: Fn(&Column) -> Column;

/// Reindex source onto target index (RIGHT OUTER JOIN primitive)
pub fn reindex_by(source: &Frame, target_index: &IndexColumn) -> Frame;

/// Map x onto y's index (exact match, RIGHT OUTER JOIN)
pub fn mapr(x: &Frame, y: &Frame) -> Frame;

/// Asof join: reindex x onto y (at-or-before, RIGHT OUTER ASOF JOIN)
pub fn asofr(x: &Frame, y: &Frame) -> Frame;
```

### Everything Else

**Phase 3+ operations** must either:
1. Be expressed via these primitives, OR
2. Be explicitly marked as "schema-changing"

**Forbidden:** Adding near-duplicate operations (e.g., `mapr2`, `mapr_loose`, `mapr_with_tolerance`)

---

## Performance Contracts

### No Accidental Regressions

**Benchmarks MUST NOT regress by >20% without explicit justification:**

1. `dlog` on 5M cells (some NA)
2. `reindex_by`:
   - Sorted indices (best case)
   - Unsorted indices (worst case)

**Gate:** Phase 3 starts ONLY after benchmarks pass

---

## Phase 3+ Gate Checklist

**Phase 3 (Macro IR) starts ONLY when:**
- [ ] `docs/contracts.md` exists (this file)
- [ ] Property tests exist (mapr idempotence, identity, monotonicity)
- [ ] Performance benchmarks exist (dlog, reindex_by)
- [ ] NA policy formalized (is_na() used everywhere)
- [ ] API surface locked (no sprawl)

**Rationale:** IR optimization on wrong semantics = backwards-compat hell

---

## Non-Negotiable Rules Summary

1. **Tags = Arc (always)**
2. **No index coercion** (Date ≠ Timestamp ≠ String)
3. **Exact match only** (no asof in mapr)
4. **Duplicates = last wins** (deterministic, no error)
5. **NA = sentinel** (use is_na() abstraction)
6. **mapr = RIGHT OUTER JOIN** (y index authority)
7. **Arc preservation** (I1-I3 for numeric ops)
8. **Stable API** (3 primitives only)
9. **No regressions** (>20% = justification required)
10. **Gate before Phase 3** (checklist must pass)

---

**Last Updated:** 2026-02-20 (Phase 2 complete)
**Enforcement:** Property tests + CI checks
**Violations:** Immediate rollback or explicit amendment with rationale
