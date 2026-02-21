# GLD_NUM Parity Gap Analysis

**Golden Test Case**: The most complex real-world pipeline in lastcode.sh

## The GLD_NUM Expression

```lisp
(let* ((s (-> (stdin) (w5) (dlog) (x- 1) (cs1) (wzs 25 1) (> -1) (shift 2))))
  (-> (file "GC1C.csv") (mapr s) (dlog) (ur 250 5) (* s) (cs1)))
```

---

## IR Planner Coverage Analysis

### ✅ Operations SUPPORTED in IR (7/15)

| Operation | IR Function | Status |
|-----------|-------------|--------|
| `dlog` | NumericFunc::Dlog | ✅ |
| `shift 2` | NumericFunc::Shift { k: 2 } | ✅ |
| `file "GC1C.csv"` | Source::File | ✅ |
| `mapr s` | JoinOp::MapR | ✅ |
| `* s` | BinaryFunc::Mul (frame-frame) | ✅ |
| `rolling-zscore` | Derived (rolling-mean + rolling-std) | ✅ |
| `let*` | Planner binding context | ✅ |

### ❌ Operations MISSING from IR (8/15)

| Operation | Macro/Builtin | Purpose | Priority |
|-----------|---------------|---------|----------|
| `stdin` | builtin | Read CSV from stdin | **HIGH** |
| `w5` | macro → `locf` | Fill forward (last value) | **HIGH** |
| `x- 1` | macro → `xminus` | Pairwise spread (col subtraction) | **HIGH** |
| `cs1` | builtin | Cross-sectional z-score (row-wise) | **HIGH** |
| `ecs1` | builtin | Exponential CS (with decay) | MEDIUM |
| `wzs 25 1` | macro → `wzscore` | Windowed z-score (exists but name mismatch) | MEDIUM |
| `> -1` | builtin | Filter/threshold | **HIGH** |
| `ur 250 5` | builtin | Rolling regression/beta | MEDIUM |

---

## Macro → Builtin Mapping

From `stdlib/finance_short.cl`:

```lisp
(defmacro wzs (x w l) `(wzscore ,x ,w ,l))  ; → rolling-zscore in IR
(defmacro x- (x half) `(xminus ,x ,half))   ; → needs IR support
(defmacro wq (x w l) `(inv (wv ,x ,w ,l)))  ; → inverse of variance
```

**wzs** already exists in IR as `rolling-zscore`, just needs macro bridge.

---

## Missing IR Primitives

### 1. **stdin** (I/O Source)

**Current IR**: Has `Source::File { path }`
**Needed**: `Source::Stdin`

**Implementation**:
```rust
// src/ir.rs
pub enum Source {
    File { path: String },
    Stdin,  // NEW
    Variable { name: SymbolId },
}

// src/planner.rs
"stdin" => {
    let node_id = NodeId(plan.nodes.len());
    let node = Node {
        id: node_id,
        op: Operation::Source(Source::Stdin),
        schema: SchemaInfo::unknown(),
    };
    Ok(plan.add_node(node))
}

// src/exec.rs
Source::Stdin => {
    let stdin = std::io::stdin();
    let reader = BufReader::new(stdin.lock());
    io::read_csv_from_reader(reader, &rt.interner)?
}
```

---

### 2. **locf** (Last Observation Carried Forward)

**Purpose**: Fill NA values with last valid value (forward fill)

**Implementation**:
```rust
// src/ir.rs
pub enum NumericFunc {
    // ... existing ...
    Locf,  // NEW: fill forward
}

// Kernel: src/exec.rs
fn locf_column(col: &Column) -> Column {
    let Column::F64(data) = col;
    let mut result = Vec::with_capacity(data.len());
    let mut last_valid = f64::NAN;

    for &val in data {
        if !val.is_nan() {
            last_valid = val;
        }
        result.push(last_valid);
    }

    Column::F64(result)
}
```

---

### 3. **xminus** (Pairwise Spread)

**Purpose**: Subtract one column from all others (spread calculation)

**Example**: `(x- data 1)` → `col2 - col1, col3 - col1, ...`

**Implementation**:
```rust
// This is a SHAPE-CHANGING operation (reduces ncols by 1)
// Needs special handling in IR

// src/ir.rs
pub enum UnaryOp {
    MapNumeric { ... },
    PairwiseSpread { half: usize },  // NEW: subtract column `half` from others
}

// Executor builds new frame with (ncols - 1) columns
```

---

### 4. **Cross-Sectional Z-Score** (`cs1`)

**Purpose**: Z-score across columns at each timestamp (row-wise standardization)

**Formula**: For each row `i`: `zscore[i,j] = (x[i,j] - mean(row i)) / std(row i)`

**Implementation**:
```rust
// src/ir.rs
pub enum NumericFunc {
    // ... existing ...
    CrossSectionalZScore,  // NEW: row-wise zscore
}

// Kernel: row-wise mean and std
fn cs_zscore_frame(frame: &Frame) -> Frame {
    let nrows = frame.nrows();
    let ncols = frame.ncols();

    let mut result_cols = vec![Vec::new(); ncols];

    for row in 0..nrows {
        // Extract row values
        let mut row_vals = Vec::new();
        for col in 0..ncols {
            let val = frame.get_col(col).unwrap()[row];
            if !val.is_nan() {
                row_vals.push(val);
            }
        }

        // Compute row stats
        let mean = row_vals.iter().sum::<f64>() / row_vals.len() as f64;
        let variance = row_vals.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / row_vals.len() as f64;
        let std = variance.sqrt();

        // Standardize row
        for col in 0..ncols {
            let val = frame.get_col(col).unwrap()[row];
            let zscore = if std > 1e-10 {
                (val - mean) / std
            } else {
                f64::NAN
            };
            result_cols[col].push(zscore);
        }
    }

    // Build result frame (preserves tags)
    map_numeric_preserve_tags(frame, ...)
}
```

---

### 5. **Filtering** (`> -1`)

**Purpose**: Keep only rows where condition is true

**Implementation**:
```rust
// This is ROW-FILTERING (changes nrows!)
// Breaks I1-I3 invariants (index changes)

// Needs new IR operation category: Filter
pub enum Operation {
    // ... existing ...
    Filter(FilterOp),  // NEW
}

pub struct FilterOp {
    input: NodeId,
    condition: FilterCondition,
}

pub enum FilterCondition {
    GreaterThan(f64),  // > value
    LessThan(f64),     // < value
    // ... etc
}
```

**Challenge**: Filtering breaks Arc preservation (I1-I3) since it changes the index!

**Solution**: Filtering returns a **new index** (subset of input index).

---

### 6. **Rolling Regression** (`ur 250 5`)

**Purpose**: Rolling linear regression (probably rolling beta or rolling correlation)

**Example**: `(ur x 250 5)` → window=250, step=5

**Implementation**: Complex (requires OLS, multiple passes). Lower priority.

---

## Priority Implementation Order

### Phase 1: I/O + Basic Transforms (HIGH)
1. **stdin** - Enable pipeline from bash
2. **locf** - Common data cleaning operation
3. **wzs macro** - Map to existing `rolling-zscore`

**Impact**: Enables simple pipelines like GLD_NUM_SIG

---

### Phase 2: Cross-Sectional (HIGH)
4. **cs1** - Cross-sectional standardization
5. **x-** - Pairwise spreads

**Impact**: Enables strategy construction

---

### Phase 3: Filtering (HIGH)
6. **> threshold** - Row filtering

**Challenge**: Breaks I1-I3 invariants, needs new operation type

**Impact**: Enables signal generation

---

### Phase 4: Advanced (MEDIUM)
7. **ecs1** - Exponential cross-sectional
8. **ur** - Rolling regression

---

## Current GLD_NUM Test Status

### With Legacy Evaluator (--legacy)
```bash
cd /home/ubuntu
./blisp/target/release/blisp --legacy --load stdlib/finance_short.cl \
  -e '(let* ((s (-> (stdin) (w5) (dlog) (x- 1) (cs1) (wzs 25 1) (> -1) (shift 2))))
        (-> (file "GC1C.csv") (mapr s) (dlog) (ur 250 5) (* s) (cs1)))'
```
**Status**: Should work (if all builtins exist)

### With IR Executor (hybrid mode)
```bash
# Same command without --legacy
```
**Status**: Falls back to legacy (missing ops)

**Goal**: Make IR handle the entire pipeline for 6-102x speedup on rolling ops!

---

## Next Steps

To achieve GLD_NUM parity with IR:

1. **Add stdin to planner** (~30 min)
2. **Add locf to IR** (~1 hour)
3. **Map wzs macro to rolling-zscore** (~15 min)
4. **Add cs1 (cross-sectional)** (~2 hours)
5. **Add xminus (pairwise)** (~2 hours)
6. **Add filtering** (~3 hours, breaks I1-I3)
7. **Add ur (regression)** (~4 hours, complex)

**Estimated total**: ~13 hours of work to full GLD_NUM parity.

**Quick win**: Items 1-3 enable simpler pipelines in ~2 hours.

---

*Analysis created: 2026-02-21*
*Goal: Full IR coverage for real-world finance pipelines*
