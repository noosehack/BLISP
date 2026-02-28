# BLISP Orientation - Quick Reference Card

**Last Updated**: 2026-02-28
**Status**: ✅ Phase 1 Complete - Full D4 Support with blawktrust Integration

---

## TL;DR

**All 10 orientations supported**: H, N, _N, _H, Z, S, _Z, _S, X, R

```lisp
;; Absolute orientation (set)
(o 'H table)   ;; Column-major normal
(o 'Z table)   ;; Row-major (transpose)
(o 'X table)   ;; Elementwise mode
(o 'R table)   ;; Scalar reduction mode

;; Relative orientation (compose)
(ro 'Z table)  ;; Apply Z transformation to current orientation
```

---

## All 10 Orientations

### Column-Major Family (ColwiseLike)

| Symbol | Name | Behavior | Display |
|--------|------|----------|---------|
| `'H` | Normal | Default, columns contiguous | `ori=H` |
| `'N` | Rows reversed | Time-reversed (tac) | `ori=N` |
| `'_N` | Columns reversed | Column-reversed | `ori=_N` |
| `'_H` | Both reversed | Both flipped | `ori=_H` |

### Row-Major Family (RowwiseLike)

| Symbol | Name | Behavior | Display |
|--------|------|----------|---------|
| `'Z` | Transpose | Row-major normal | `ori=Z` |
| `'S` | Transpose (synonym) | Same as Z | `ori=Z` ⚠️ |
| `'_Z` | Transpose + flip rows | Rows reversed | `ori=_Z` |
| `'_S` | Transpose + flip cols | Columns reversed | `ori=_S` |

### Special Modes

| Symbol | Name | Behavior | Display |
|--------|------|----------|---------|
| `'X` | Elementwise | Each cell independent | `ori=X` |
| `'R` | Real/Scalar | Reduce to scalar | `ori=R` |

---

## Operation Semantics

### Aggregation Operations (sum, mean, std)

**Column-major (H, N, _N, _H)**:
```lisp
(sum (o 'H table))  ;; → Column sums (1×N)
```

**Row-major (Z, S, _Z, _S)**:
```lisp
(sum (o 'Z table))  ;; → Row sums (M×1)
```

**Scalar reduction (R)**:
```lisp
(sum (o 'R table))  ;; → Total sum (single value)
```

**Elementwise (X)**:
```lisp
(sum (o 'X table))  ;; → Panic (X is for broadcast, not aggregation)
```

### Sequence Operations (dlog, shift, cs1)

**Respect orientation class** (ColwiseLike vs RowwiseLike):
```lisp
(dlog 1 (o 'H df))  ;; → dlog down columns
(dlog 1 (o 'Z df))  ;; → dlog across rows
```

---

## D4 Composition with `(ro ...)`

**Relative orientation** composes transformations:

```lisp
(ro 'Z (ro 'Z df))  ;; → ori=H (Z∘Z = identity)
(ro 'N (o 'H df))   ;; → ori=N (time reversal)
```

**Rules**:
- Only works with 8 D4 orientations (H, N, _N, _H, Z, S, _Z, _S)
- Rejects X and R modes (not composable)
- Returns Result (error on invalid composition)

---

## Shape Display

**What `shape=M×N` means**:
- Shows **input** dimensions (underlying data)
- NOT output dimensions for operations

**Examples**:
```lisp
(o 'R table)  ;; shape=2×2 (input has 2×2 data)
(sum (o 'R table))  ;; → [10] (output is single scalar!)

(o 'X table)  ;; shape=2×2 (input has 2×2 data)
(sum (o 'X table))  ;; → [1,3,2,4] (output is 4 values)
```

This is **intentional** - shape shows what you're operating on, not what you'll get.

---

## Common Patterns

### Transpose for row-wise operations
```lisp
(-> data
    (o 'Z)      ;; Switch to row-major
    (sum))      ;; Sum across columns per row
```

### Scalar total
```lisp
(sum (o 'R data))  ;; Grand total
```

### Elementwise operations
```lisp
(o 'X data)  ;; Each cell independent (for broadcasting)
```

---

## Known Limitations

### 1. S displays as Z

**Why**: S and Z have identical D4 values (both `swap=true, flip_i=false, flip_j=false`)

**Impact**: `(o 'S ...)` shows `ori=Z` in output

**Status**: By design - S is a synonym for Z, Z is the canonical name

### 2. Compass notation not fully documented

While `'NSWE`, `'WENS`, etc. work, the quick reference uses short names (H, Z, etc.) for clarity.

---

## Verification

**All 10 orientations tested**:
- ✅ Display names correct (9/10, S→Z expected)
- ✅ Semantic behavior verified (sum, dlog, composition)
- ✅ D4 composition works (Z∘Z = H)
- ✅ R mode reduces to scalar
- ✅ X mode returns identity

**Test script**: `test_orientation_truth_table.lisp`

---

## Architecture

**Single source of truth**: `blawktrust::TableView::ori`

**BLISP layer**:
- `builtin_o`: Maps symbols → blawktrust orientations
- `builtin_ro`: Composes D4 transformations
- Display uses `ori.canonical_name()`

**blawktrust layer**:
- 10 orientation constants (ORI_H, ORI_Z, etc.)
- D4 composition logic
- Orientation-aware operations

---

## Files

- `ORIENTATION_REFACTOR_COMPLETE.md` - Full refactor details
- `ORIENTATION_INVESTIGATION_COMPLETE.md` - Red flag investigation
- `ORIENTATION_SEMANTICS_VERIFIED.md` - Test results
- `ARCHITECTURE_BLAWKTRUST_VS_BLISP.md` - Layer separation

---

**Status**: ✅ Production-ready with full D4 support

---
