# BLISP Double-Fail Audit: Builtin-Only Computational Tokens

**Date**: 2026-02-27
**Source**: AUDIT_VERIFICATION_SESSION_REPORT.md (corrected token counts)

---

## Audit Criteria

Tokens that satisfy ALL of the following:
1. ✓ Registered in builtins.rs
2. ✓ NOT recognized in planner.rs
3. ✓ Perform Frame/table computation (not side effects, not introspection)
4. ✓ Would cause double-fail if nested inside IR-only op

---

## P0: Critical - Comparison Operators (5 tokens)

### 1. `<` (less than)
- **Builtin function**: `builtin_lss` (builtins.rs:125)
- **Why computational**: Element-wise comparison of two Frames, returns boolean Frame
- **Double-fail example**:
  ```lisp
  (dlog (< PRC 100))
  ;; IR: planner sees "dlog" (OK) → plans child → sees "<" → Unknown function: <
  ;; Legacy: eval sees "<" (OK) → evaluates args → sees "dlog" → Unknown function: dlog
  ;; BOTH PATHS FAIL
  ```
- **Planner mapping**: Add `BinaryFunc::LSS` to ir.rs, map in planner.rs line ~520

### 2. `>=` (greater than or equal)
- **Builtin function**: `builtin_gte` (builtins.rs:126)
- **Why computational**: Element-wise comparison, returns boolean Frame
- **Double-fail example**:
  ```lisp
  (shift 1 (>= VOL 1000000))
  ;; IR: planner sees "shift" (OK) → plans child → sees ">=" → Unknown function: >=
  ;; Legacy: eval sees ">=" (OK) → evaluates args → sees "shift" → Unknown function: shift
  ```
- **Planner mapping**: Add `BinaryFunc::GTE` to ir.rs, map in planner.rs

### 3. `<=` (less than or equal)
- **Builtin function**: `builtin_lte` (builtins.rs:127)
- **Why computational**: Element-wise comparison, returns boolean Frame
- **Double-fail example**:
  ```lisp
  (locf (<= VOL 0))
  ;; IR: planner sees "locf" (OK) → plans child → sees "<=" → Unknown function: <=
  ;; Legacy: eval sees "<=" (OK) → evaluates args → sees "locf" → Unknown function: locf
  ```
- **Planner mapping**: Add `BinaryFunc::LTE` to ir.rs, map in planner.rs

### 4. `==` (equal)
- **Builtin function**: `builtin_eql` (builtins.rs:128)
- **Why computational**: Element-wise equality test, returns boolean Frame
- **Double-fail example**:
  ```lisp
  (cs1 (== SECTOR "TECH"))
  ;; IR: planner sees "cs1" (OK) → plans child → sees "==" → Unknown function: ==
  ;; Legacy: eval sees "==" (OK) → evaluates args → sees "cs1" → Unknown function: cs1
  ```
- **Planner mapping**: Add `BinaryFunc::EQL` to ir.rs, map in planner.rs

### 5. `!=` (not equal)
- **Builtin function**: `builtin_neq` (builtins.rs:129)
- **Why computational**: Element-wise inequality test, returns boolean Frame
- **Double-fail example**:
  ```lisp
  (ur 250 (!= COUNTRY "US"))
  ;; IR: planner sees "ur" (OK) → plans child → sees "!=" → Unknown function: !=
  ;; Legacy: eval sees "!=" (OK) → evaluates args → sees "ur" → Unknown function: ur
  ```
- **Planner mapping**: Add `BinaryFunc::NEQ` to ir.rs, map in planner.rs

---

## P1: High - Dangerous Aliases (5 tokens)

### 6. `w5` (weekday mask alias)
- **Builtin function**: `builtin_wkd` (builtins.rs:187)
- **Why computational**: Applies weekday mask to Frame (Mon-Fri filter)
- **Double-fail example**:
  ```lisp
  (dlog (w5 20 PRC))
  ;; IR: planner sees "dlog" (OK) → plans child → sees "w5" → Unknown function: w5
  ;; Legacy: eval sees "w5" (OK) → evaluates args → sees "dlog" → Unknown function: dlog
  ;; CANONICAL DOUBLE-FAIL EXAMPLE
  ```
- **Planner mapping**: Add alias `"w5"` → delegate to existing `wkd` logic (planner.rs:132)
  ```rust
  "w5" => {
      eprintln!("Warning: 'w5' is deprecated, use 'wkd' instead");
      // Delegate to wkd implementation
  }
  ```

### 7. `dlog-col` (dlog alias)
- **Builtin function**: `builtin_dlog` (builtins.rs:133)
- **Why computational**: Logarithmic differentiation (dlog = diff(log(x)))
- **Double-fail example**:
  ```lisp
  (shift 1 (dlog-col PRC))
  ;; IR: planner sees "shift" (OK) → plans child → sees "dlog-col" → Unknown function: dlog-col
  ;; Legacy: eval sees "dlog-col" (OK) → evaluates args → sees "shift" → Unknown function: shift
  ```
- **Planner mapping**: Add alias `"dlog-col"` → delegate to `dlog` logic (planner.rs:123)

### 8. `shift-col` (shift alias)
- **Builtin function**: `builtin_shift` (builtins.rs:134)
- **Why computational**: Time-series lag/lead operation on Frame
- **Double-fail example**:
  ```lisp
  (ret (shift-col 5 PRC))
  ;; IR: planner sees "ret" (OK) → plans child → sees "shift-col" → Unknown function: shift-col
  ;; Legacy: eval sees "shift-col" (OK) → evaluates args → sees "ret" → Unknown function: ret
  ```
- **Planner mapping**: Add alias `"shift-col"` → delegate to `shift` logic (planner.rs:114)

### 9. `cs1-col` (cs1 alias)
- **Builtin function**: `builtin_cs1` (builtins.rs:163)
- **Why computational**: Cumulative sum across observations
- **Double-fail example**:
  ```lisp
  (dlog (cs1-col VOL))
  ;; IR: planner sees "dlog" (OK) → plans child → sees "cs1-col" → Unknown function: cs1-col
  ;; Legacy: eval sees "cs1-col" (OK) → evaluates args → sees "dlog" → Unknown function: dlog
  ```
- **Planner mapping**: Add alias `"cs1-col"` → delegate to `cs1` logic (planner.rs:120)

### 10. `ur-col` (ur alias)
- **Builtin function**: `builtin_ur` (builtins.rs:173)
- **Why computational**: Unstandardized rolling (reverse z-score normalization)
- **Double-fail example**:
  ```lisp
  (locf (ur-col 250 STD_RET))
  ;; IR: planner sees "locf" (OK) → plans child → sees "ur-col" → Unknown function: ur-col
  ;; Legacy: eval sees "ur-col" (OK) → evaluates args → sees "locf" → Unknown function: locf
  ```
- **Planner mapping**: Add alias `"ur-col"` → delegate to `ur` logic (planner.rs:121)

---

## P2: Medium - Frame Transform Operations (9 tokens)

### 11. `diff` (difference)
- **Builtin function**: `builtin_diff` (builtins.rs:132)
- **Why computational**: Calculates X[t] - X[t-1], standard time-series differencing
- **Double-fail example**:
  ```lisp
  (cs1 (diff PRC))
  ;; IR: planner sees "cs1" (OK) → plans child → sees "diff" → Unknown function: diff
  ;; Legacy: eval sees "diff" (OK) → evaluates args → sees "cs1" → Unknown function: cs1
  ```
- **Planner mapping**: Add `NumericFunc::SHF_PTW_OBS_LIN_DIFF` to ir.rs
  - Note: Similar to `shift` but computes delta directly
  - Could implement as `(- X (shift 1 X))` composite or dedicated kernel

### 12. `zscore` (z-score standardization)
- **Builtin function**: `builtin_zscore` (builtins.rs:197)
- **Why computational**: Standardizes Frame to mean=0, std=1 (global statistics)
- **Double-fail example**:
  ```lisp
  (ret (zscore PRC))
  ;; IR: planner sees "ret" (OK) → plans child → sees "zscore" → Unknown function: zscore
  ;; Legacy: eval sees "zscore" (OK) → evaluates args → sees "ret" → Unknown function: ret
  ```
- **Planner mapping**: Add to planner, but note: `ft-zscore` already exists in planner.rs:302
  - Check if `zscore` is global vs `ft-zscore` is feature-wise
  - If same semantics, add as alias: `"zscore"` → delegate to `ft-zscore`

### 13. `chop` (trim NAs)
- **Builtin function**: `builtin_chop` (builtins.rs:198)
- **Why computational**: Removes leading/trailing NA values from Frame
- **Double-fail example**:
  ```lisp
  (dlog (chop PRC))
  ;; IR: planner sees "dlog" (OK) → plans child → sees "chop" → Unknown function: chop
  ;; Legacy: eval sees "chop" (OK) → evaluates args → sees "dlog" → Unknown function: dlog
  ```
- **Planner mapping**: Add `NumericFunc::SHF_PTW_OBS_XXX_CHOP` or SchemaOp
  - This modifies shape, might be better as SchemaOp rather than NumericFunc

### 14. `keep-shape` (preserve shape with NAs)
- **Builtin function**: `builtin_keep_shape` (builtins.rs:156)
- **Why computational**: Pads result with NAs to maintain original Frame length
- **Double-fail example**:
  ```lisp
  (shift 1 (keep-shape (/ A B)))
  ;; IR: planner sees "shift" (OK) → plans "/" (OK) → plans "keep-shape" → Unknown function
  ;; Legacy: eval sees "keep-shape" (OK) but "/" may not work → depends on context
  ```
- **Planner mapping**: Note that planner already has `keep` (planner.rs:108)
  - Check if `keep-shape` is alias for `keep`
  - If same: add alias `"keep-shape"` → delegate to `keep`
  - If different: add new SchemaOp

### 15. `ecs1` (exponential cumulative sum)
- **Builtin function**: `builtin_ecs1` (builtins.rs:168)
- **Why computational**: Exponentially-weighted cumulative sum (decay parameter)
- **Double-fail example**:
  ```lisp
  (ur 250 (ecs1 0.9 RET))
  ;; IR: planner sees "ur" (OK) → plans child → sees "ecs1" → Unknown function: ecs1
  ;; Legacy: eval sees "ecs1" (OK) → evaluates args → sees "ur" → Unknown function: ur
  ```
- **Planner mapping**: Add `NumericFunc::SHF_PTW_OBS_XXX_ECS1` (exponential variant of cs1)
  - Requires decay parameter in addition to Frame input

### 16. `wstd` (windowed standard deviation)
- **Builtin function**: `builtin_wstd` (builtins.rs:191)
- **Why computational**: Rolling window standard deviation
- **Double-fail example**:
  ```lisp
  (dlog (wstd 20 PRC))
  ;; IR: planner sees "dlog" (OK) → plans child → sees "wstd" → Unknown function: wstd
  ;; Legacy: eval sees "wstd" (OK) → evaluates args → sees "dlog" → Unknown function: dlog
  ```
- **Planner mapping**: Check if `rolling-std` (planner.rs:278) is equivalent
  - If same: add alias `"wstd"` → delegate to `rolling-std`
  - If different (e.g., ddof or NA handling): add as separate NumericFunc

### 17. `wstd0` (windowed std with zero-fill)
- **Builtin function**: `builtin_wstd0` (builtins.rs:192)
- **Why computational**: Rolling window std, treats NAs as zeros
- **Double-fail example**:
  ```lisp
  (shift 1 (wstd0 50 VOL))
  ;; IR: planner sees "shift" (OK) → plans child → sees "wstd0" → Unknown function: wstd0
  ;; Legacy: eval sees "wstd0" (OK) → evaluates args → sees "shift" → Unknown function: shift
  ```
- **Planner mapping**: Add as variant of `rolling-std` with NA-as-zero semantics

### 18. `wv` (windowed variance)
- **Builtin function**: `builtin_wv` (builtins.rs:193)
- **Why computational**: Rolling window variance
- **Double-fail example**:
  ```lisp
  (cs1 (wv 20 RET))
  ;; IR: planner sees "cs1" (OK) → plans child → sees "wv" → Unknown function: wv
  ;; Legacy: eval sees "wv" (OK) → evaluates args → sees "cs1" → Unknown function: cs1
  ```
- **Planner mapping**: Add `NumericFunc::SHF_ROL_XXX_LIN_VAR` (variance = std²)
  - Can implement as composite or dedicated kernel

### 19. `wz0` (windowed z-score with zero-fill)
- **Builtin function**: `builtin_wz0` (builtins.rs:195)
- **Why computational**: Rolling z-score standardization, NAs as zeros
- **Double-fail example**:
  ```lisp
  (locf (wz0 250 0 1 RET))
  ;; IR: planner sees "locf" (OK) → plans child → sees "wz0" → Unknown function: wz0
  ;; Legacy: eval sees "wz0" (OK) → evaluates args → sees "locf" → Unknown function: locf
  ```
- **Planner mapping**: Add to planner
  - Note: `wzs` (rolling-zscore) is dual-routing (planner.rs:320)
  - Check if `wz0` differs only in NA handling (zero-fill vs skip)
  - If same semantics: add alias, if different: add variant

---

## Tokens Excluded (Not Meeting All Criteria)

### Aggregations (6 tokens) - EXCLUDED
- `sum`, `sum0`, `mean`, `mean0`, `std`, `std0`
- **Why excluded**: These are scalar aggregations (Frame → Scalar)
  - Nesting in IR tree like `(dlog (sum A))` is nonsensical (dlog expects Frame, gets Scalar)
  - Not realistic double-fail scenario
  - These are correctly builtin-only (terminal operations, not compositional)

### Column-wise Variants (2 tokens) - EXCLUDED
- `>-col`, `>-cols`
- **Why excluded**: Legacy batch syntax, use `map-cols` instead
  - Low migration priority
  - Better to deprecate in favor of modern composition pattern

### Multi-Column Suffixed Ops (12 tokens) - EXCLUDED
- `dlog-cols`, `shift-cols`, `diff-cols`, `locf-cols`, `cs1-cols`, `ecs1-cols`, `ur-cols`, `wz0-cols`, `wstd-cols`, `wstd0-cols`, `wv-cols`, `keep-shape-cols`
- **Why excluded**: Legacy batch syntax
  - Modern approach: `(map-cols (lambda (x) (dlog x)) frame)`
  - Shouldn't migrate these, use functional composition instead

### Schema/Table Ops (10 tokens) - EXCLUDED
- `col`, `cols`, `setcol`, `withcol`, `w`, `make-col`, `select`, `select-num`, `map-cols`, `apply-cols`
- **Why excluded**: Not computational Frame operations
  - These are table manipulation primitives (extract, bind, filter)
  - Correctly excluded from IR planner (schema layer, not numeric layer)

### Mask Operations (5 tokens) - EXCLUDED
- `mask-on`, `mask-off`, `mask-list`, `mask-stats`, `mask-define`
- **Why excluded**: Side effects (mutate global state)
  - Should NOT be in IR planner (IR should be pure functional)

### I/O Operations (3 tokens) - EXCLUDED
- `file-head`, `save`, `print`
- **Why excluded**: Side effects (I/O)
  - Correctly excluded from IR

### Introspection (3 tokens) - EXCLUDED
- `type-of`, `len`, `o`
- **Why excluded**: Meta-operations (reflect on values)
  - Not computational Frame operations

---

## Summary Table

| Priority | Token | Category | Builtin Function | Canonical Name | Action |
|----------|-------|----------|------------------|----------------|--------|
| **P0** | `<` | Comparison | builtin_lss | — | Add BinaryFunc::LSS |
| **P0** | `>=` | Comparison | builtin_gte | — | Add BinaryFunc::GTE |
| **P0** | `<=` | Comparison | builtin_lte | — | Add BinaryFunc::LTE |
| **P0** | `==` | Comparison | builtin_eql | — | Add BinaryFunc::EQL |
| **P0** | `!=` | Comparison | builtin_neq | — | Add BinaryFunc::NEQ |
| **P1** | `w5` | Alias | builtin_wkd | wkd | Add alias → wkd |
| **P1** | `dlog-col` | Alias | builtin_dlog | dlog | Add alias → dlog |
| **P1** | `shift-col` | Alias | builtin_shift | shift | Add alias → shift |
| **P1** | `cs1-col` | Alias | builtin_cs1 | cs1 | Add alias → cs1 |
| **P1** | `ur-col` | Alias | builtin_ur | ur | Add alias → ur |
| **P2** | `diff` | Transform | builtin_diff | — | Add NumericFunc::DIFF |
| **P2** | `zscore` | Transform | builtin_zscore | ft-zscore? | Check if alias |
| **P2** | `chop` | Transform | builtin_chop | — | Add SchemaOp::CHOP |
| **P2** | `keep-shape` | Transform | builtin_keep_shape | keep? | Check if alias |
| **P2** | `ecs1` | Transform | builtin_ecs1 | — | Add NumericFunc::ECS1 |
| **P2** | `wstd` | Rolling | builtin_wstd | rolling-std? | Check if alias |
| **P2** | `wstd0` | Rolling | builtin_wstd0 | — | Add variant |
| **P2** | `wv` | Rolling | builtin_wv | — | Add NumericFunc::VAR |
| **P2** | `wz0` | Rolling | builtin_wz0 | wzs? | Check if variant |

**Total**: 19 tokens need migration to prevent double-fail

---

## Verification

### Confirm token is builtin-only:
```bash
cd /home/ubuntu/blisp

# Check token is registered builtin
rg 'register_builtin.*"TOKEN"' src/builtins.rs

# Verify token NOT in planner
rg '"TOKEN"' src/planner.rs  # should return empty
```

### Test double-fail scenario:
```bash
cd /home/ubuntu/blisp

# Example: test w5 in IR tree
echo '(dlog (w5 20 PRC))' | BLISP_MODE=hybrid ./blisp
# Expected: Error on both IR and legacy paths

# After fix: should succeed
echo '(dlog (w5 20 PRC))' | BLISP_MODE=ir_only ./blisp
# Expected: Success after adding w5 alias to planner
```

---

## Implementation Priority

### Phase 1 (P0): Critical - Comparison Operators
**Impact**: BLOCKS all IR expressions using comparisons
**Effort**: Medium (need BinaryFunc variants + exec kernels)
**Timeline**: Implement first (1-2 days)

Add to planner.rs (~line 520):
```rust
"<" | ">=" | "<=" | "==" | "!=" => {
    // Map to BinaryFunc::LSS, GTE, LTE, EQL, NEQ
}
```

### Phase 2 (P1): High - Dangerous Aliases
**Impact**: BREAKS nested expressions with legacy names
**Effort**: Low (add 5 alias mappings)
**Timeline**: Implement second (1 day)

Add to planner.rs (near canonical definitions):
```rust
"w5" => { /* delegate to wkd */ }
"dlog-col" => { /* delegate to dlog */ }
// ... etc
```

### Phase 3 (P2): Medium - Frame Transforms
**Impact**: Performance optimization + better IR coverage
**Effort**: High (need kernel implementations)
**Timeline**: Implement incrementally (1-2 weeks)

Prioritize by usage frequency:
1. `diff` (very common in time-series)
2. `zscore` (common in feature engineering)
3. Rolling ops (`wstd`, `wv`, `wz0`) if not aliasable
4. Shape ops (`chop`, `keep-shape`) if needed
5. `ecs1` (less common, lower priority)

---

## Next Steps

1. ✅ **Verification complete** - 19 tokens identified
2. ⏳ **Phase 1**: Add 5 comparison operators to planner
3. ⏳ **Phase 2**: Add 5 dangerous aliases to planner
4. ⏳ **Phase 3**: Evaluate P2 tokens case-by-case
5. ⏳ **Testing**: Add regression tests for double-fail scenarios
6. ⏳ **Cleanup**: Remove redundant builtin registrations (11 dual-routing tokens)

---

**Status**: Audit complete, ready for implementation.
