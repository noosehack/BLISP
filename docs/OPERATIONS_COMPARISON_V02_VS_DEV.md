# BLISP Operations Comparison: v0.2.0 Release vs Dev Branch

**Date:** 2026-03-05
**v0.2.0 Release:** branch `reconstruct/tableview-only` (commit 5bb68ff)
**Dev Branch:** branch `pr-ir-trace-planerror` (uncommitted changes on top of master)

---

## TL;DR

- **IR enums: IDENTICAL** — same 20 NumericFunc, 10 BinaryFunc, 2 JoinOp, 3 SchemaOp
- **Planner: IDENTICAL** — same 42 operation names handled
- **Fusion: IDENTICAL and DEAD on both** — `ir_fusion::optimize()` exists but is NEVER CALLED
- **Normalizer: DIFFERENT** — v0.2 only does `->` threading; dev adds alias canonicalization + arg-order rewrite
- **Builtins: Dev adds 16** — word-form aliases (add, sub, log, ln, exp, abs, sqrt, inv, etc.)
- **Dev adds:** `dic.rs` module, `PlanError` typed errors, `hybrid_eval`, `BLISP_SEGMENT` gate, `--trace-plan`

---

## 1. IR Operations (src/ir.rs) — IDENTICAL

### NumericFunc (20 variants)

| # | IR Canonical Name | Description | v0.2.0 | Dev |
|---|---|---|---|---|
| 1 | `SHF_PTW_OBS_NLN_DLOG` | Diff-log (observation-based, NA-skipping) | YES | YES |
| 2 | `SHF_PTW_OFS_NLN_DLOG` | Diff-log (offset-based, positional) | YES | YES |
| 3 | `RET` | Simple return: x/x[-1] - 1 | YES | YES |
| 4 | `LOG` | Natural logarithm | YES | YES |
| 5 | `EXP` | Exponential (e^x) | YES | YES |
| 6 | `SQRT` | Square root | YES | YES |
| 7 | `ABS` | Absolute value | YES | YES |
| 8 | `INV` | Inverse (1/x) | YES | YES |
| 9 | `SHF_REC_NLN_LOCF` | Last observation carried forward | YES | YES |
| 10 | `MSK_WKE` | Weekday mask (weekends → NA) | YES | YES |
| 11 | `SHF_PFX_LIN_SUM` | Cumulative sum starting at 1.0 | YES | YES |
| 12 | `SHF_PTW_LIN_SHF{k}` | Shift/lag by k rows | YES | YES |
| 13 | `LAG_OBS{k}` | Mask-aware shift by k observations | YES | YES |
| 14 | `KEEP{k}` | Keep every k-th row | YES | YES |
| 15 | `SHF_WIN_LIN_AVG{w}` | Rolling mean (strict min_periods=w) | YES | YES |
| 16 | `SHF_WIN_NLN_SDV{w}` | Rolling std (strict min_periods=w) | YES | YES |
| 17 | `SHF_WIN_MIN2_LIN_AVG{w}` | Rolling mean (min 2 obs, relaxed) | YES | YES |
| 18 | `SHF_WIN_MIN2_NLN_SDV{w}` | Rolling std (min 2 obs, relaxed) | YES | YES |
| 19 | `SHF_WIN_MIN2_LIN_AVG_EXCL{w}` | Rolling mean excl. current (for ft-zscore) | YES | YES |
| 20 | `SHF_WIN_MIN2_NLN_SDV_EXCL{w}` | Rolling std excl. current (for ft-zscore) | YES | YES |

### BinaryFunc (10 variants)

| # | IR Canonical | Description | v0.2.0 | Dev |
|---|---|---|---|---|
| 1 | `ADD` | Addition | YES | YES |
| 2 | `SUB` | Subtraction | YES | YES |
| 3 | `MUL` | Multiplication | YES | YES |
| 4 | `DIV` | Division | YES | YES |
| 5 | `GTR` | Greater than | YES | YES |
| 6 | `LSS` | Less than | YES | YES |
| 7 | `LTE` | Less than or equal | YES | YES |
| 8 | `GTE` | Greater than or equal | YES | YES |
| 9 | `EQL` | Equal | YES | YES |
| 10 | `NEQ` | Not equal | YES | YES |

### JoinOp (2 variants)

| # | IR Canonical | Description | v0.2.0 | Dev |
|---|---|---|---|---|
| 1 | `ALIGN` | RIGHT OUTER JOIN (mapr) | YES | YES |
| 2 | `ASOF_ALIGN` | AS-OF JOIN (asofr) | YES | YES |

### SchemaOp (3 variants)

| # | IR Canonical | Description | v0.2.0 | Dev |
|---|---|---|---|---|
| 1 | `SHF_PTW_LIN_SPR` | Pairwise spreads (xminus) | YES | YES |
| 2 | `MSK_WKE_DEF` | Define weekend mask | YES | YES |
| 3 | `WTH_MSK` | Activate mask | YES | YES |

### Fused Variants (6 + 1 base)

| # | UnaryOp Variant | Description | v0.2.0 | Dev |
|---|---|---|---|---|
| 1 | `MapNumeric{func}` | Single numeric op (base) | YES | YES |
| 2 | `FusedElementwise{ops}` | Chain of pure elementwise (PR4.1) | YES | YES |
| 3 | `FusedCs1Elementwise{ops}` | cs1 + elementwise chain (PR4.2a) | YES | YES |
| 4 | `FusedCs1DlogOfs{lag}` | cs1 + dlog-ofs (PR4.2b) | YES | YES |
| 5 | `FusedCs1DlogObs` | cs1 + dlog-obs (PR4.2b) | YES | YES |
| 6 | `FusedDlogObsElementwise{ops}` | dlog-obs + elementwise (future) | YES | YES |
| 7 | `FusedDlogOfsElementwise{lag,ops}` | dlog-ofs + elementwise (future) | YES | YES |

---

## 2. Planner Coverage (src/planner.rs) — IDENTICAL

Both branches handle the same 42 user-facing operation names:

### Sources (3)

| User Name | IR Target | v0.2.0 | Dev |
|---|---|---|---|
| `file`, `load`, `read-csv` | `Source::File` | YES | YES |
| `stdin` | `Source::Stdin` | YES | YES |

### Simple Unary → NumericFunc (22)

| User Name | IR Target | v0.2.0 | Dev |
|---|---|---|---|
| `dlog` | `SHF_PTW_OBS_NLN_DLOG` | YES | YES |
| `dlog-ofs` | `SHF_PTW_OFS_NLN_DLOG` | YES | YES |
| `dlog-col` | → redirects to `dlog` | YES | YES |
| `ret` | `RET` | YES | YES |
| `log` | `LOG` | YES | YES |
| `exp` | `EXP` | YES | YES |
| `sqrt` | `SQRT` | YES | YES |
| `abs` | `ABS` | YES | YES |
| `inv` | `INV` | YES | YES |
| `locf` | `SHF_REC_NLN_LOCF` | YES | YES |
| `wkd` | `MSK_WKE` | YES | YES |
| `w5` | `MSK_WKE` (deprecated) | YES | YES |
| `cs1` | `SHF_PFX_LIN_SUM` | YES | YES |
| `cs1-col` | → redirects to `cs1` | YES | YES |
| `shift`, `shift-col` | `SHF_PTW_LIN_SHF{k}` | YES | YES |
| `lag-obs`, `shift-obs` | `LAG_OBS{k}` | YES | YES |
| `keep` | `KEEP{k}` | YES | YES |
| `rolling-mean` | `SHF_WIN_LIN_AVG{w}` | YES | YES |
| `rolling-mean-min2` | `SHF_WIN_MIN2_LIN_AVG{w}` | YES | YES |
| `rolling-std` | `SHF_WIN_NLN_SDV{w}` | YES | YES |
| `rolling-std-min2` | `SHF_WIN_MIN2_NLN_SDV{w}` | YES | YES |
| `ft-mean` | `SHF_WIN_MIN2_LIN_AVG_EXCL{w}` | YES | YES |
| `ft-std` | `SHF_WIN_MIN2_NLN_SDV_EXCL{w}` | YES | YES |

### Composite Unary — Planner Rewrites (4)

| User Name | IR Decomposition | v0.2.0 | Dev |
|---|---|---|---|
| `rolling-zscore`, `wzs` | rolling-mean-min2 + rolling-std-min2 + SUB + DIV | YES | YES |
| `ft-zscore` | ft-mean + ft-std + SUB + DIV | YES | YES |
| `ur`, `ur-col` | rolling-std-min2 + KEEP + LOCF + MUL + DIV | YES | YES |

### Binary (10)

| User Name | IR Target | v0.2.0 | Dev |
|---|---|---|---|
| `+` | `ADD` | YES | YES |
| `-` | `SUB` | YES | YES |
| `*` | `MUL` | YES | YES |
| `/` | `DIV` | YES | YES |
| `>` | `GTR` | YES | YES |
| `<` | `LSS` | YES | YES |
| `<=` | `LTE` | YES | YES |
| `>=` | `GTE` | YES | YES |
| `==` | `EQL` | YES | YES |
| `!=` | `NEQ` | YES | YES |

### Joins (2)

| User Name | IR Target | v0.2.0 | Dev |
|---|---|---|---|
| `mapr` | `ALIGN` | YES | YES |
| `asofr` | `ASOF_ALIGN` | YES | YES |

### Schema (3)

| User Name | IR Target | v0.2.0 | Dev |
|---|---|---|---|
| `xminus` | `SHF_PTW_LIN_SPR` | YES | YES |
| `mask-weekend` | `MSK_WKE_DEF` | YES | YES |
| `with-mask` | `WTH_MSK` | YES | YES |

### Special Forms (1)

| User Name | Notes | v0.2.0 | Dev |
|---|---|---|---|
| `let` | Variable bindings (let* semantics) | YES | YES |

---

## 3. Fusion Optimizer (src/ir_fusion.rs) — IDENTICAL, DEAD ON BOTH

| Pass | Fusible Ops | v0.2.0 | Dev | CALLED? |
|---|---|---|---|---|
| PR4.1 Elementwise | ABS, LOG, EXP, SQRT, INV chains | YES | YES | **NO** |
| PR4.2a cs1+elem | SHF_PFX_LIN_SUM + elementwise chain | YES | YES | **NO** |
| PR4.2b cs1+dlog | SHF_PFX_LIN_SUM + DLOG (obs or ofs) | YES | YES | **NO** |

**`ir_fusion::optimize()` is compiled but never invoked in the runtime pipeline on either branch.**

Grep results:
- `grep "ir_fusion" src/main.rs` → 0 matches (both branches)
- `grep "optimize" src/main.rs` → only in comments/strings (both branches)

---

## 4. Normalizer (src/normalize.rs) — DIFFERENT

### v0.2.0 Release

Only one transformation:
- **Thread-first expansion:** `(-> a (f x) (g y))` → `(g (f a x) y)`

No alias canonicalization. No arg-order rewrite.

### Dev Branch (NEW)

Three transformations:
1. **Thread-first expansion** (same as v0.2.0)
2. **Alias canonicalization** (`canonical_name` function):

| User Input | Canonical Output |
|---|---|
| `dlog-cols`, `dlog-col` | `dlog` |
| `cs1-cols`, `cs1-col` | `cs1` |
| `shift-cols`, `shift-col` | `shift` |
| `>-cols`, `>-col` | `>` |
| `ur-cols`, `ur-col` | `ur` |
| `locf-cols` | `locf` |
| `rolling-mean-cols` | `rolling-mean` |
| `rolling-std-cols` | `rolling-std` |
| `rolling-zscore-cols` | `rolling-zscore` |
| `x-` | `xminus` |
| `let*` | `let` |

3. **Arg-order rewrite** (data-first normalization):
   - 2-param ops: if arg1 is Int and arg2 is not → swap (e.g., `(shift 2 x)` → `(shift x 2)`)
   - 3-param ops: if arg1-2 are Int and arg3 is not → rotate
   - Affected: `rolling-mean`, `rolling-std`, `rolling-mean-min2`, `rolling-std-min2`, `rolling-zscore`, `shift`, `keep`, `lag-obs`, `shift-obs`, `ft-mean`, `ft-std`, `ft-zscore`, `wzs`, `ur`

---

## 5. Builtins (src/builtins.rs) — Dev adds 16

### v0.2.0: 74 builtins

| Category | Names |
|---|---|
| Arithmetic | `+`, `-`, `*`, `/` |
| Comparison | `>`, `<`, `>=`, `<=`, `==`, `!=`, `>-cols`, `>-col` |
| Table dlog/shift/diff | `dlog-col`, `dlog-cols`, `shift-col`, `shift-cols`, `diff`, `diff-col`, `diff-cols` |
| Aggregates | `sum`, `sum0`, `mean`, `mean0`, `std`, `std0` |
| I/O | `file`, `file-head`, `stdin`, `save`, `print` |
| Column ops | `col`, `w`, `setcol`, `withcol`, `make-col`, `cols`, `select`, `select-num` |
| Table ops | `map-cols`, `apply-cols` |
| Fill/Filter | `locf-cols`, `keep-shape`, `keep-shape-cols` |
| Mask | `mask-on`, `mask-off`, `mask-list`, `mask-stats`, `mask-define`, `wkd`, `w5` |
| Pairwise | `xminus` |
| Cumulative | `cs1-cols`, `cs1-col`, `ecs1`, `ecs1-cols`, `ecs1-col` |
| Joins | `mapr` |
| Rolling | `ur-cols`, `ur-col`, `wz0`, `wz0-cols`, `wzs`, `wstd`, `wstd0`, `wstd-cols`, `wstd0-cols`, `wv`, `wv-cols` |
| Stats | `zscore`, `chop` |
| Debug | `type-of`, `len` |
| Orientation | `o`, `ro` |

### Dev: 90 builtins (+16 new)

All 74 above, plus:

| # | New Name | Handler | Category |
|---|---|---|---|
| 75 | `add` | builtin_add | Arithmetic word alias |
| 76 | `sub` | builtin_sub | Arithmetic word alias |
| 77 | `mul` | builtin_mul | Arithmetic word alias |
| 78 | `div` | builtin_div | Arithmetic word alias |
| 79 | `log` | builtin_log | Math function |
| 80 | `ln` | builtin_log | Math function alias |
| 81 | `exp` | builtin_exp | Math function |
| 82 | `abs` | builtin_abs | Math function |
| 83 | `sqrt` | builtin_sqrt | Math function |
| 84 | `inv` | builtin_inv | Math function |
| 85 | `gt` | builtin_gt | Comparison word alias |
| 86 | `lt` | builtin_lt | Comparison word alias |
| 87 | `gte` | builtin_gte | Comparison word alias |
| 88 | `lte` | builtin_lte | Comparison word alias |
| 89 | `eq` | builtin_eq | Comparison word alias |
| 90 | `neq` | builtin_neq | Comparison word alias |

---

## 6. Compat Macros (stdlib/compat_clispi.cl) — IDENTICAL

| Macro | Expansion | Both Branches |
|---|---|---|
| `(dlog x)` | `(dlog-cols x)` | YES |
| `(shift x lag)` | `(shift-cols x lag)` | YES |
| `(avg x)` | `(mean x)` | YES |
| `(std_dev x)` | `(std x)` | YES |
| `(cs1 x)` | `(cs1-cols x)` | YES |
| `(> x threshold)` | `(>-cols x threshold)` | YES |
| `(wavg x w)` | `(wmean-cols x w)` | YES |
| `(x- x half)` | `(xminus x half)` | YES |
| `(keep x step)` | `(keep-shape x step)` | YES |
| `(wq x w)` | `(/ 1 (wv x w))` | YES |
| `(ir x)` | info-ratio composite | YES |
| `(ir2 x)` | `(ir x)` | YES |
| `(ur x w step)` | `(ur-cols w step x)` | YES |
| `(ecs1 x)` | `(exp (cs1 (dlog x)))` | YES |
| `(dump fn x)` | save + return | YES |

---

## 7. Dev-Only Changes (NOT in v0.2.0)

### New: `PlanError` enum (src/planner.rs)

| Variant | Purpose |
|---|---|
| `Unsupported { op, reason }` | Op exists but IR can't handle this usage |
| `BadArgs { op, detail }` | Wrong argument count or type |
| `NonLiteral { op, which_arg, expected }` | Param must be literal integer |
| `Unknown { op }` | Unrecognized function name |

Replaces ~50 ad-hoc string errors from v0.2.0.

### New: `IrError` enum (src/main.rs)

| Variant | Purpose |
|---|---|
| `Plan(PlanError)` | Planner can't handle → eligible for legacy fallback |
| `Exec(String)` | Execution failed → hard error |

### New: `hybrid_eval()` (src/main.rs)

Segmented hybrid evaluation (gated behind `BLISP_SEGMENT=1`):
- Peels glue forms: `save`, `progn`, `print`
- Recurses on children
- Routes finance subtrees to IR planner

### New: `--trace-plan` / `BLISP_TRACE_PLAN=1`

Diagnostic output:
- `[TRACE] canonical= <AST>` (after normalize)
- `[TRACE] planned ops= [...]` (after plan)
- `[TRACE] fallback reason= <PlanError>` (on legacy fallback)
- `[TRACE] result= IR` (on success)

### New: `dic.rs` module

- Operation dictionary with YAML validation
- `OPS_CURRENT.yml` / `OPS_PLANNED.yml` compiled into binary
- Anti-invention guardrail: validates YAML `ir:` fields against actual enum variants
- `blisp dic` subcommand for introspection

### Modified: `eval.rs`

- Added `"let"` alongside `"let*"` in special forms (2 locations)

### Modified: `builtins.rs`

- `builtin_save` extended with `Value::Frame` arm for IR-produced frames
- 16 new word-form alias builtins (see Section 5)

---

## 8. The Pipeline — Both Branches

### v0.2.0 Release
```
Parse → Normalize(-> only) → Plan(AST→IR) → Execute
                                    ↓ PlanError (string match)
                              Legacy rt.eval()
```

### Dev Branch
```
Parse → Normalize(-> + aliases + arg-rewrite) → Plan(AST→IR) → Execute
                                                      ↓ PlanError (typed enum)
                                                Legacy rt.eval()

With BLISP_SEGMENT=1:
Parse → Normalize → hybrid_eval()
                      ├─ Glue? → peel, recurse on children
                      └─ Finance? → Plan → Execute
                                      ↓ PlanError
                                    Legacy rt.eval()
```

### NEITHER branch calls `ir_fusion::optimize()`.

The correct pipeline (not yet implemented):
```
Parse → Normalize → Plan → ir_fusion::optimize() → Execute
```

---

## 9. Operations NOT in IR (legacy only, both branches)

| Operation | Category | Notes |
|---|---|---|
| `sum`, `sum0` | Aggregation (reduce) | Needs IR reduce type |
| `mean`, `mean0` | Aggregation (reduce) | Needs IR reduce type |
| `std`, `std0` | Aggregation (reduce) | Needs IR reduce type |
| `diff`, `diff-cols` | Diff | Not in IR |
| `col`, `w` | Column select | Table manipulation |
| `select`, `select-num` | Column filter | Table manipulation |
| `setcol`, `withcol` | Column mutation | Table manipulation |
| `map-cols`, `apply-cols` | Higher-order | Table manipulation |
| `o`, `ro` | Orientation | Structural |
| `zscore` | Static zscore | Not rolling |
| `chop` | Truncation | Utility |
| `ecs1` | Composite | `exp(cs1(dlog(x)))` — could be planner rewrite |
| `wz0`, `wz0-cols` | Rolling zscore base | Superseded by IR rolling-zscore |
| `wstd`, `wstd0`, `wstd-cols`, `wstd0-cols` | Rolling std variants | Superseded by IR rolling-std |
| `wv`, `wv-cols` | Rolling variance | Not in IR |
| `mask-on`, `mask-off`, etc. | Mask mgmt | Partially in IR (MSK_WKE_DEF, WTH_MSK) |

---

*Generated: 2026-03-05*
