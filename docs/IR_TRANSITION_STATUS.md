# IR Transition Status — BLISP

**Date:** 2026-03-04
**Branch:** `pr-ir-trace-planerror`
**Authoritative reference for the BLISP legacy-to-IR migration.**

---

## 1. Architectural Goal

BLISP is transitioning from a legacy AST evaluator to a fused IR execution layer.
The final architecture separates concerns cleanly:

**IR layer — financial operations:**
- `locf` — last observation carried forward
- `dlog` — log returns
- `rolling-mean` / `rolling-std` — rolling statistics
- `wzs` — rolling z-score (composite: wz0 → keep-shape → locf)
- `ur` — unit ratio (risk-adjusted returns)
- `shift` — lag/lead
- `cs1` — cumulative sum
- `xminus` — pairwise subtraction (spread)
- `>` — threshold mask
- `w5` / `wkd` — weekend mask
- `*`, `/`, `+`, `-` — arithmetic
- `mapr` — align/join

**Legacy evaluator — language glue:**
- `let` / `let*` — variable binding
- `save` — CSV file output
- `progn` — sequential evaluation
- `print` — console output
- `mapr` — map with resampling (legacy dispatch)
- `defmacro` / macro expansion
- `lambda`, `if`, `define`, `setf`

### Key invariant

Financial pipelines must execute through IR and be eligible for fusion.
Legacy execution exists only for language glue forms.

### Compat macro rule

Compat macros (`stdlib/compat_clispi.cl`) must **never** override canonical operation
names (`dlog`, `cs1`, `ur`, `shift`, `>`, `locf`, `wkd`, `xminus`, `keep`). These names
belong to the IR planner. Compat may only define:

- Legacy spelling aliases (`dlog-cols` → `dlog`) for the legacy fallback path
- Genuinely different sugar (`avg` → `mean`, `std_dev` → `std`)
- Composite macros (`ecs1`, `wq`, `ir`)

When `BLISP_SEGMENT=1` is active, compat macros for canonical names are bypassed
entirely — hybrid_eval routes finance subtrees through IR before macro expansion occurs.

---

## 2. Current Execution Model

BLISP operates in HYBRID mode by default. There are two HYBRID variants:

### Default HYBRID (current default)

```
Parse → Normalize → Canonicalize → try_ir_eval(whole expression)
  ├─ Success → IR result
  └─ PlanError → rt.eval(whole expression)  [legacy fallback]
```

The entire expression is attempted through IR. If the planner cannot handle it
(e.g., `save` wraps the expression), the **entire** expression falls back to legacy.
This preserves CLISPI-compatible output.

### Segmented HYBRID (`BLISP_SEGMENT=1`)

```
Parse → Normalize → Canonicalize → hybrid_eval()
  ├─ Glue form (save, progn, print)?
  │     → Peel glue, recurse on children
  └─ Everything else?
        → try_ir_eval(subtree)
            ├─ Success → IR result
            └─ PlanError → rt.eval(subtree)  [legacy fallback]
```

`hybrid_eval()` recursively walks the AST. Glue forms are handled directly;
finance subtrees are routed to the IR planner. This means a pipeline inside
`(save ...)` executes through IR even though `save` itself is not IR-plannable.

### Conceptual example

```lisp
(save "GLD_NUM.csv"
  (let* ((s (-> (stdin) (w5) (dlog) (x- 1) (cs1) (wzs 25 1) (> -1) (shift 2))))
    (-> (file "../GC1C.csv") (mapr s) (dlog) (ur 250 5) (* s) (cs1))))
```

In segmented hybrid mode:
- `save` is peeled by `hybrid_eval` (glue)
- `let*` is canonicalized to `let` and planned through IR
- The entire `let` body — both the signal pipeline `s` and the application pipeline —
  plans and executes as a single fused IR graph
- The resulting `Value::Frame` is converted to `Table` (index prepended) and written
  via the existing `save_csv` serializer

### Normalization pipeline

`normalize.rs` applies two passes:

1. **Thread expansion** — `(-> x (f a) (g b))` → `(g (f x a) b)` (data-first)
2. **Canonicalization** — alias rewrite + arg-order normalization

Alias table (`canonical_name`):
```
dlog-cols / dlog-col  →  dlog
cs1-cols / cs1-col    →  cs1
shift-cols / shift-col →  shift
>-cols / >-col        →  >
ur-cols / ur-col      →  ur
locf-cols             →  locf
rolling-mean-cols     →  rolling-mean
rolling-std-cols      →  rolling-std
rolling-zscore-cols   →  rolling-zscore
x-                    →  xminus
let*                  →  let
```

Arg-order rewrite (ambiguity-safe):
- 2-param ops: if `elements[1]` is Int and `elements[2]` is not → swap to data-first
- 3-param ops: if `elements[1..2]` are Int and `elements[3]` is not → rotate to data-first
- Ambiguous cases (both Int, neither Int) → no rewrite

---

## 3. PR-A (Plumbing) Changes

All changes are on branch `pr-ir-trace-planerror`.

### Error handling

- **`PlanError` enum** (`src/planner.rs:21`): 4 typed variants replace ~50 ad-hoc string errors
  - `Unsupported { op, reason }` — op exists but IR can't handle this usage
  - `BadArgs { op, detail }` — wrong argument count or type
  - `NonLiteral { op, which_arg, expected }` — parameter must be literal integer
  - `Unknown { op }` — unrecognized function name
- **`IrError` enum** (`src/main.rs:707`): separates plan-time errors (fallback-eligible)
  from execution errors (hard failures)
  - `Plan(PlanError)` — planner couldn't handle expression → HYBRID falls back
  - `Exec(String)` — execution failed → propagate as real error
- **String-based fallback removed** — no more pattern-matching on error message text

### Diagnostics

- `--trace-plan` flag or `BLISP_TRACE_PLAN=1` env var
- After normalize: `[TRACE] canonical= <AST>`
- After plan: `[TRACE] planned ops= [op1, op2, ...]`
- On fallback: `[TRACE] fallback reason= <PlanError>`
- On success: `[TRACE] result= IR`

### Canonicalization (`src/normalize.rs`)

- `x-` → `xminus` (reader limitation workaround, now handled at AST level)
- `let*` → `let` (single canonical symbol; legacy eval updated to accept both)
- Legacy `-cols` spellings → canonical names (see alias table above)
- Ambiguity-safe arg-order rewrite for 12 two-param ops and 2 three-param ops

### Planner (`src/planner.rs`)

- All parameterized ops converted to **data-first** semantics:
  - 2-arg: `elements[1]` = data (recurse), `elements[2]` = param (Int literal)
  - 3-arg: `elements[1]` = data (recurse), `elements[2]` = w, `elements[3]` = step
- Affected ops: `rolling-mean`, `rolling-std`, `rolling-mean-min2`, `rolling-std-min2`,
  `rolling-zscore`, `shift`, `keep`, `lag-obs`, `shift-obs`, `ft-mean`, `ft-std`,
  `ft-zscore`, `wzs`, `ur`

### Legacy eval (`src/eval.rs`)

- Added `"let"` alongside `"let*"` in special forms match (2 locations: macro-guard
  and dispatch). Both route to `eval_let_star`.

### Hybrid execution (`src/main.rs`)

- `hybrid_eval()` (~50 lines): peels glue forms, recurses on children, tries IR for
  everything else
- Glue forms handled: `save`, `progn`, `print`
- Gated behind `BLISP_SEGMENT=1` env var — default HYBRID mode unchanged

### Save support (`src/builtins.rs`)

- `builtin_save` extended with `Value::Frame` arm
- Conversion path: `ensure_tableview` extracts data columns → prepend index column
  (Date or Timestamp from `Frame.tags.index`) → build `Table` → call existing `save_csv`
- Reuses the exact same CSV serializer — no new formatting path
- Index column is always first (Blueprint I3 invariant)

### Compat layer (`stdlib/compat_clispi.cl`)

- Section 1 macros (dlog, cs1, shift, locf, >, ur) **retained** for legacy fallback path
- Documented as IR-transparent: only expand during `rt.eval()`, never seen by IR planner
- `x-` macro retained for legacy path (also aliased in normalizer for IR path)
- Sugar macros unchanged: `avg`, `std_dev`, `wavg`, `wq`, `ir`, `ir2`, `ecs1`, `dump`

### Result

- All 326 tests pass, 0 failures
- `cargo fmt` clean, `cargo clippy` clean
- GLD_NUM runs successfully in both modes:
  - Default HYBRID: 6828 rows, byte-identical to baseline (legacy path)
  - `BLISP_SEGMENT=1`: 9560 rows, full IR execution (new architecture)
- With and without `--load compat`: segmented mode produces identical output
  (compat macros for canonical names are bypassed by IR)

---

## 4. Current Known Semantics Gaps

Full analysis with repro commands: `SEMANTICS_GAP_REPORT.md`

### A — Weekend handling (`w5` / `MSK_WKE`)

| | Legacy | IR |
|---|---|---|
| **Behavior** | Delete weekend rows | Mask weekend values as NaN |
| **Row count** | 6828 (GLD_NUM) | 9560 (GLD_NUM) |
| **Style** | CLISPI (filter) | kdb (nullify) |

Weekend NaN values propagate through all downstream rolling operations,
changing both which rows produce output and the numerical values of those outputs.

**Decision required:** mask, delete, or expose both as distinct operators.

### B — `ur` implementation

| | Legacy | IR |
|---|---|---|
| **Step handling** | `i % step` modulo — recompute vol every step-th row | Step param ignored entirely |
| **Pattern** | Bespoke (unique among all ops) | Incomplete |
| **Min periods** | Full window required | `min2` (relaxed, after 2 obs) |
| **NaN/zero** | Skips both NaN and zero in window | Kernel-dependent |

Legacy `ur` is the **only** operation using modulo stepping. Every other stepped op
follows the composition pattern:

```
compute_all(data, w) → keep-shape(result, step) → locf(result)
```

This is how `wzs` works (`builtins.rs:2546-2553`). The correct IR `ur` should follow
the same pattern:

```
ur(data, w, step) = data / (scale * locf(keep-shape(rolling-std(data, w), step)))
```

Where `scale = 100 * sqrt(252)`.

### C — Rolling window NA propagation

| | Legacy | IR |
|---|---|---|
| **Min periods** | Full window (`w`) | 2 observations (`min2`) |
| **NaN handling** | Skip NaN (and zero in `ur`) | Kernel-dependent |
| **Window counting** | Count only valid observations | TBD |

With masked weekends, IR rolling windows contain NaN entries. A window of 250
calendar rows may have only ~150 valid weekday observations. Legacy never sees
this because weekends are deleted before rolling computation.

---

## 5. GLD_NUM Status

### Gating mechanism

```bash
# Default — legacy behavior preserved, 6828 rows, byte-identical baseline
./GLD_NUM_BLISP.sh

# Segmented IR — new architecture, 9560 rows, different values
BLISP_SEGMENT=1 ./GLD_NUM_BLISP.sh
```

The `BLISP_SEGMENT` env var controls which HYBRID variant is used:

| `BLISP_SEGMENT` | Mode | GLD_NUM rows | Values match baseline |
|---|---|---|---|
| unset (default) | Legacy fallback | 6828 | Byte-identical |
| `1` | Segmented hybrid | 9560 | Different (semantics gaps) |

This allows PR-A to merge safely without changing any production output.

### Baseline files

- `GLD_NUM_BLISP_baseline.csv` — legacy-path output (6828 rows), current golden reference
- `GLD_NUM_BLISP_ir_baseline.csv` — IR-path output (9560 rows), for PR-B comparison

---

## 6. Tomorrow's Plan (PR-B)

### Step 1 — Fix `ur` in IR

Implement correct composition semantics in `src/planner.rs`:

```
rolling-std(data, w)
  → keep-shape(step)
  → locf
  → scale multiplication (100 * sqrt(252))
  → division (data / scaled_vol)
```

Remove the modulo stepping from the legacy `ur` implementation or document it as
deprecated. The composition pattern is the canonical architecture.

### Step 2 — Standardize weekend semantics

Options:
- **(A)** Mask weekends only (kdb-style) — simpler, all rows preserved
- **(B)** Delete weekends only (CLISPI-style) — fewer rows, legacy-compatible
- **(C)** Two operators: `wkd` (mask) and `wkd-del` (filter) — explicit, no ambiguity

Recommendation: **(C)** — expose both. Map `w5` to `wkd` (mask) with deprecation warning.
Users who need CLISPI-style deletion use `wkd-del` explicitly.

### Step 3 — Define rolling window contract

Document and enforce consistently across all rolling ops:

1. **Min periods**: `rolling-std` requires full window; `rolling-std-min2` starts at 2.
   All IR rolling ops must declare which variant they use.
2. **NaN propagation**: NaN values are **skipped** (effective window shrinks). A window
   of 250 with 100 NaN entries has 150 valid observations.
3. **Zero policy**: Zeros are valid observations. Legacy `ur`'s zero-skipping is a bug
   specific to that implementation, not a system-wide rule.

### Step 4 — Remove `BLISP_SEGMENT` gate

Once:
- `ur` uses composition pattern
- Weekend semantics are decided and implemented
- Rolling window contract is defined and tested
- GLD_NUM baseline is updated to reflect IR-native output
- `SEMANTICS_CHANGELOG.md` documents all intentional changes

Then: make segmented hybrid the default, remove the env var gate, and remove
compat Section 1 macros (no longer needed when IR handles all finance ops).

---

## 7. Test Plan

### IR planning tests

Verify that finance pipelines plan through IR (not fall back to legacy):

```bash
BLISP_TRACE_PLAN=1 BLISP_SEGMENT=1 blisp -e '(-> (file "data.csv") (dlog) (rolling-mean 250))' 2>&1 | grep "result= IR"
```

Expressions to verify:
- `(-> x (dlog))` — plans as `SHF_PTW_OBS_NLN_DLOG`
- `(-> x (rolling-mean 250))` — plans as `SHF_WIN_MIN2_LIN_AVG { w: 250 }`
- `(-> x (ur 250 5))` — plans as composition (after PR-B fix)
- `(-> x (wzs 25 1))` — plans as rolling-zscore composite
- `(let ((s (-> ...))) (-> y (mapr s)))` — `let` plans through IR, `mapr` plans as ALIGN

### GLD_NUM golden test

```bash
./GLD_NUM_BLISP.sh
diff GLD_NUM_BLISP.csv GLD_NUM_BLISP_baseline.csv  # must exit 0
```

After PR-B, update baseline and repeat.

### Semantic unit tests

- **Weekend mask**: `w5` on data with known weekend dates → verify NaN positions
- **Rolling with NaN**: rolling-mean on `[1, NaN, 3, 4, 5]` w=3 → verify NaN handling
- **`ur` composition**: `ur(data, 5, 2)` → verify matches `data / (scale * locf(keep-shape(rolling-std(data, 5), 2)))`
- **`keep-shape`**: `keep-shape([10, 11, 12, 13, 14], 3)` → `[10, NaN, NaN, 13, NaN]`
- **Frame save round-trip**: save Frame to CSV, reload, verify byte-identical columns

---

## 8. Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Rolling semantics drift between IR and legacy | High | GLD_NUM values wrong | Define contract (Section 6 Step 3), add regression tests |
| CSV serialization differences (Frame vs Table path) | Low | Byte-level diff in output | Frame→Table conversion reuses exact same `save_csv` serializer |
| Compat macros interfere with IR planning | Medium | Finance ops fall back to legacy | Compat macros are IR-transparent; segmented mode bypasses them entirely |
| Legacy scripts break when gate is removed | Medium | User disruption | Keep `--legacy` flag as escape hatch; document migration in SEMANTICS_CHANGELOG.md |
| `ur` fix changes GLD_NUM values intentionally | Certain | Baseline update required | Commit new baseline with explanation; old baseline preserved for reference |

---

## 9. Summary

**PR-A completed the infrastructure migration:**
- Typed error handling (`PlanError`, `IrError`) replaces string-based fallback
- Canonicalization pass normalizes all legacy spellings to canonical IR names
- Planner enforces data-first semantics for all parameterized ops
- `hybrid_eval` peels glue forms and routes finance subtrees to IR
- `builtin_save` handles IR-produced Frames via Table conversion
- All 326 tests pass; GLD_NUM byte-identical in default mode

**PR-B will complete the semantic alignment:**
- Fix `ur` to use composition pattern (rolling-std → keep-shape → locf → divide)
- Standardize weekend handling (mask vs delete → two explicit operators)
- Define rolling window NA propagation contract
- Update GLD_NUM baseline to reflect IR-native output
- Remove `BLISP_SEGMENT` gate, make segmented hybrid the default

### Unfinished items checklist

- [ ] Fix IR `ur` to use composition pattern instead of ignoring step
- [ ] Add `MSK_WKE_DEL` (weekend filter) alongside `MSK_WKE` (weekend mask)
- [ ] Define and document rolling window NA/min-periods contract
- [ ] Add semantic unit tests for each gap
- [ ] Update GLD_NUM baseline after semantic convergence
- [ ] Remove `BLISP_SEGMENT` gate
- [ ] Remove compat Section 1 macros (canonical→legacy)
- [ ] Write `SEMANTICS_CHANGELOG.md` documenting all intentional output changes

### Critical files

| File | Role |
|---|---|
| `src/main.rs` | `hybrid_eval`, `try_ir_eval`, `IrError`, `BLISP_SEGMENT` gate |
| `src/planner.rs` | `PlanError` enum, data-first op planning, `ur` decomposition |
| `src/normalize.rs` | Canonicalization: alias table, arg-order rewrite |
| `src/eval.rs` | Legacy special forms (`let` / `let*`) |
| `src/builtins.rs` | `builtin_save` Frame handling, `ensure_tableview` |
| `src/exec.rs` | IR executor, `Source::Variable` resolution |
| `src/ir.rs` | IR node types, `NumericFunc` enum |
| `stdlib/compat_clispi.cl` | Compat macros (Section 1 retained for legacy path) |
| `SEMANTICS_GAP_REPORT.md` | Detailed gap analysis with repro commands |
| `GLD_NUM_BLISP.sh` | Sacred golden test script |
