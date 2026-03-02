# X Mode Aggregation Decision

**Date**: 2026-02-28
**Issue**: Should sum/mean/std work on X (Each) mode?
**Decision**: **NO** - Aggregations reject X mode (panic/error)

---

## Problem

User asked: "why sum cannot work element wise? we can sum one number no?"

Initially implemented: sum(X) returns identity (all values flattened)

**But**: This created inconsistency:
- `sum(X)` → Returns all values
- `mean(X)` → Error "not defined for this orientation"
- `std(X)` → Error "not defined for this orientation"

---

## Semantic Analysis

### What is X (Each) mode?

**Purpose**: Elementwise broadcast context for **binary operations**
- `table + scalar` with X mode → broadcast scalar to each element
- **NOT** for unary aggregations

### Why aggregations reject X mode

**X mode = "no vector structure"**
- Aggregations require vector structure (sum down column, sum across row)
- X mode explicitly says "each element independently"
- Therefore: aggregation is undefined

**Consistency**:
- dlog(X) → Panic "requires sequence"
- w5(X) → Panic "requires sequence"  
- mean(X) → Error "not defined for this orientation"
- std(X) → Error "not defined for this orientation"
- sum(X) → **Should also reject**

---

## Decision

**Revert sum(X) to panic** for consistency.

**Message**: "sum not defined for Each (X) orientation - use for broadcast context only"

**Rationale**:
1. Consistent with all other aggregations
2. X mode is for broadcast (binary ops), not aggregations
3. Clear semantics: aggregations require vector structure

---

## Alternative Considered (Rejected)

**Option**: Make all aggregations return identity for X mode
- sum(X) → All values
- mean(X) → All values  
- std(X) → ???

**Problem**: 
- std(scalar) is undefined (no variance)
- Semantically unclear what "aggregate each cell independently" means
- X mode loses its meaning as "broadcast context"

---

## Current Behavior (Final)

| Operation | X Mode Behavior |
|-----------|----------------|
| **Aggregations** | | 
| sum | Panic |
| mean | Error |
| std | Error |
| **Sequence Ops** | |
| dlog | Panic |
| w5 | Panic |
| cs1 | ? (TBD) |
| **Binary Ops** | |
| + - * / | Works (broadcast) |

---

## Future: Result-Based Errors

Currently sum() panics because signature is `-> Column`.

**Better**: Change to `-> Result<Column, String>` and return error instead of panic.

**Blocked by**: Would be breaking change to blawktrust API.

**Workaround**: BLISP can check ori_class() before calling sum() and return graceful error.

---

## Conclusion

**X mode is for broadcast context, not aggregations.**

All aggregations consistently reject X mode.

User's question was brilliant - it revealed an inconsistency that needed resolution.

---
