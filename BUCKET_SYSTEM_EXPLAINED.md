# BLISP Bucket Classification System

**What you see in `blisp dic`:**

```
Alias        IR / Builtin      Bucket                    Legacy Tokens
--------------------------------------------------------------------------------
+            ADD               A1_fusion_critical
dlog-col     SHF_PTW_OBS_...   A1_fusion_critical
col          <builtin>         A2_planner_structural     w
file         File              A3_edge_io
```

---

## What the Buckets Mean

The bucket system classifies operations by **migration priority** and **execution layer**.

### A1_fusion_critical (33 operations)

**Operations that benefit from IR fusion optimization**

**Characteristics:**
- Elementwise operations (can be fused into single pass)
- Temporal operations (shift, dlog, lag, locf)
- Rolling window operations (mean, std, zscore)
- Mask operations (weekday filters)
- Math operations (log, exp, sqrt, abs, inv)
- Binary arithmetic (+, -, *, /)
- Comparisons (<, >, ==, !=, <=, >=)

**Why "fusion_critical":**
- These operations compose into chains: `(log (sqrt (abs x)))`
- IR fusion can execute the entire chain in ONE pass (no intermediate allocations)
- Massive performance gains from avoiding intermediate Frame objects
- Conservative fusion rules proven safe by differential testing

**Examples:**
```lisp
;; Without fusion (3 passes, 2 intermediate frames):
(log (sqrt x))  ; pass 1: sqrt, alloc frame1
                ; pass 2: log, alloc frame2

;; With IR fusion (1 pass, 0 intermediate frames):
FusedUnary[sqrt, log](x)  ; One traversal, direct result
```

**Current status:**
- Some already in IR (ADD, LOG, GTR, etc.)
- Some still builtin-only (diff, wkd)
- Migration priority: HIGHEST (performance critical)

---

### A2_planner_structural (7 operations)

**Operations that transform table structure**

**Characteristics:**
- Table manipulation (col, o, mapr, asofr)
- Composite operations (require multi-phase planning)
- Schema-changing operations
- Not fusible (each requires its own pass)

**Why "planner_structural":**
- These operations don't fuse (they reshape tables)
- They need IR planner for schema validation
- They coordinate multiple sub-operations
- Still benefit from IR (type safety, schema checks)

**Examples:**
```lisp
(col table 'C1)           ; Extract column (structural)
(o 'Z table)              ; Reorient table (structural)
(mapr join-fn t1 t2)      ; Map-reduce join (structural)
```

**Current status:**
- Mix of builtin and IR implementations
- Migration priority: MEDIUM (correctness, not fusion)

---

### A3_edge_io (6 operations)

**I/O and utility operations at system boundaries**

**Characteristics:**
- File I/O (file, stdin)
- Utility operations (len, list, etc.)
- Edge of the system (not data-plane)
- Not performance-critical

**Why "edge_io":**
- These touch external systems (filesystem, stdin)
- No benefit from IR fusion (I/O is the bottleneck)
- May stay builtin forever (simple enough)

**Examples:**
```lisp
(file "data.csv")         ; Read from filesystem
(stdin)                   ; Read from stdin
(len frame)               ; Simple utility
```

**Current status:**
- Mostly builtin, some IR (File, Stdin)
- Migration priority: LOWEST (works fine as-is)

---

## The 3-Layer Architecture

```
┌─────────────────────────────────────────┐
│ Layer 1: Macros (normalize.rs)         │  Surface syntax
│  - Thread-first (->)                    │
└─────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│ Layer 2: Legacy Builtins (eval.rs)     │  General Lisp
│  - AST evaluator                        │  (defparameter, if, let*)
│  - Direct function calls                │  No fusion
│  - Handles: A3, some A2, some A1        │
└─────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────┐
│ Layer 3: IR Planner (planner.rs)       │  Frame operations
│  - Schema validation                    │  WITH FUSION
│  - Fusion optimizer                     │
│  - Handles: A1 (fused!), A2 (validated) │
└─────────────────────────────────────────┘
```

**Hybrid mode (default):**
- Try IR first (for Frame ops with fusion)
- Fall back to builtin (for general Lisp)

---

## Why the Bucket System Matters

### For Performance (A1 → IR)

**Before fusion:**
```lisp
(+ (* (log x) 2.0) 5.0)
```
- 3 operations × 1 million rows = 3 million function calls
- 2 intermediate Frame allocations
- 2 Arc::clone operations for tags

**After fusion:**
```rust
FusedScalarBinary {
    input: x,
    ops: [(Log, 1.0), (Mul, 2.0), (Add, 5.0)]
}
```
- 1 pass over data (1 million iterations)
- 0 intermediate allocations
- 1 output frame

**Result:** 3x faster on elementwise chains

---

### For Migration Planning

**Priority order:**

1. **A1 first** - Maximum ROI (fusion gains)
2. **A2 second** - Schema safety (no perf benefit)
3. **A3 last** - Low priority (already fast enough)

**Current PLANNED operations by bucket:**
```bash
./blisp dic --planned | grep -E "A1|A2|A3" | sort | uniq -c
```

Shows which high-value operations are still missing.

---

## Fusion Examples (A1 Operations)

### Unary Chain Fusion
```lisp
(log (sqrt (abs x)))

;; Builtin path: 3 passes
;; IR fused: 1 pass, computes all 3 inline
```

### Scalar Binary Chain Fusion
```lisp
(+ (* x 2.0) 5.0)

;; Builtin path: 2 passes
;; IR fused: 1 pass, computes both ops inline
```

### Temporal + Unary Fusion
```lisp
(log (dlog x))

;; Builtin path: 2 passes
;; IR fused: 1 pass (dlog is temporal but fusible with downstream ops)
```

**Safety guarantee:**
- All fusions proven equivalent via differential testing
- Property tests verify fused = unfused results
- No semantic changes, only performance

---

## Bucket Counts (Current State)

```
A1_fusion_critical:     33 operations (57% of CURRENT)
A2_planner_structural:   7 operations (15% of CURRENT)
A3_edge_io:              6 operations (13% of CURRENT)
```

**In PLANNED (not yet registered):**
- Mostly A1 operations (rolling-*, shift family, etc.)
- Some A2 operations (joins, composite ops)
- Few A3 operations (already working)

---

## Summary

**A1 = Fusion-critical** → Performance (can fuse into chains)
**A2 = Planner-structural** → Correctness (schema validation)
**A3 = Edge I/O** → Utility (no perf benefit from IR)

**Migration strategy:**
- A1 → IR gives massive speedup (3-10x on chains)
- A2 → IR gives safety (type checking)
- A3 → can stay builtin (already fast enough)

**The bucket in `blisp dic` tells you:**
- Which operations are fusion targets (A1)
- Which need schema validation (A2)
- Which are low-priority (A3)

This guides the roadmap: **Migrate A1 first for maximum impact.**
