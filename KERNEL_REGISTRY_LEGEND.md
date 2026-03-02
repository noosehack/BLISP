# BLISP Kernel Registry Taxonomy Legend

## Column Definitions

### kernel_id
Unique identifier for the kernel operation (uppercase, no spaces)

### description
Brief description of what the operation does

### inv (Invariants)
Describes which frame invariants are preserved:
- **I1**: Index Arc preserved (pointer equality)
- **I2**: Colnames Arc preserved (pointer equality)
- **I3**: Nrows unchanged
- **I123**: All three invariants preserved (index, colnames, nrows)
- **I13**: Index and nrows preserved, colnames may change
- **I123_LHS**: For binary ops, LHS invariants preserved
- **I1_Y_I2_X_I3_Y**: For joins, index from Y, colnames from X, nrows from Y
- **I13_I2SCHEMA**: Index and nrows preserved, colnames rebuilt deterministically

### mem (Memory Characteristics)
- **ALLOC**: Allocates new memory for result
- **ZC**: Zero-copy (returns view or reuses existing data)
- **ARC**: Arc preservation (uses Arc::clone, no data copy)
- **TAGS_MODIFIED**: Only modifies Tags metadata, data Arc unchanged
- **INPLACE**: Modifies data in place (not currently used in BLISP)

### alg (Algorithm Type)
- **POINT**: Pointwise operation (element-by-element)
- **POINT_LAG**: Pointwise with lag dependency
- **ROLL_STRICT**: Rolling window with strict min_periods=window_size
- **ROLL_PARTIAL**: Rolling window with relaxed min_periods (≥2)
- **ROLL_FT**: Rolling window excluding current observation (Ft-measurable)
- **CUM**: Cumulative operation
- **LAG_CALENDAR**: Calendar-based lag (shift by k positions)
- **LAG_OBS**: Observation-based lag (skip masked rows)
- **FILL**: NA filling operation
- **MASK_CALENDAR**: Calendar-based masking (weekends)
- **MASK_CREATE**: Creates a mask in Tags.masks
- **MASK_ACTIVATE**: Activates a mask in Tags.active_mask
- **DOWNSAMPLE**: Downsample by keeping every k-th row
- **JOIN_EXACT**: Exact match join
- **JOIN_ASOF**: As-of join (at-or-before)
- **XSEC_SPREADS**: Cross-sectional pairwise differences

### opc (Operation Code)
Short mnemonic for the operation (uppercase)

### canonical_old
The canonical name in format: `INV_MEM_ALG_OPC`
Combines all taxonomy dimensions into a single identifier

### params_schema
Parameter specifications for parameterized operations:
- `w:int` - Window size
- `k:int` - Lag/shift/keep parameter
- `half:bool` - Boolean flag (for xminus)
- `name:string` - String parameter (for mask naming)
- `mask_expr:expr` - Mask expression (for with-mask)

### shape_in / shape_out
Input and output shapes:
- `scalar` - Single value (int/float)
- `vec` - Column vector (f64 array)
- `frame` - Table/Frame (multiple columns with index)
- `vec+scalar` - Column and scalar (broadcast operations)
- `frame+frame` - Two frames (joins)

## IR Source Locations

- **NumericFunc enum**: `src/ir.rs:159-302` (unary operations)
- **BinaryFunc enum**: `src/ir.rs:334-345` (binary operations)
- **JoinOp enum**: `src/ir.rs:351-375` (join operations)
- **SchemaOp enum**: `src/ir.rs:384-435` (schema operations)

## Contract Documentation

See `src/ir.rs` for detailed contract specifications for each operation.

## Total Kernel Count

**29 unique kernel operations** (deduplicated across user-facing spellings)

## Notes

- User-facing spellings like `wstd-cols`, `>-col`, etc. are **NOT** included
- Only the underlying kernel operation is listed once
- Operations are extracted from the IR layer, not the builtin registration layer
- This ensures we're documenting the actual computational kernels, not syntax sugar
