# BLISP Execution Path Instrumentation

## Summary of Execution Paths

When user types `(dlog ...)`, there are **TWO possible paths**:

### Path 1: IR Path (via Planner)
```
main.rs:574 try_ir_eval()
  → main.rs:548 planner::plan()
    → planner.rs:123 "dlog" => NumericFunc::SHF_PTW_NLN_DLOG
  → main.rs:551 exec::execute()
    → exec.rs:157 NumericFunc::SHF_PTW_NLN_DLOG => dlog_column(col, 1)
```

### Path 2: Legacy Builtin Path
```
main.rs:587 rt.eval()
  → eval.rs:82 self.is_builtin(*head_sym)
  → eval.rs:90 self.call_builtin(*head_sym, &arg_vals)
    → builtins.rs:74 "dlog" → builtin_dlog_cols
    → builtins.rs:3471-3495 fn builtin_dlog_cols() calls dlog_column()
```

---

## Which Path is Taken?

**In HYBRID mode (default):**
- `(dlog (file "data.csv") 1)` → **IR Path** ✅
  - Because it's a Frame operation, planner succeeds
- `(dlog some-variable 1)` → **Legacy Path** if variable is not Frame
  - IR planner might fail, falls back to legacy

**In LEGACY mode (`--legacy`):**
- Always uses **Legacy Builtin Path**

**In IR-ONLY mode (`--ir-only`):**
- Always tries **IR Path** (fails if can't plan)

---

## Instrumentation Points

Add these logging statements to trace execution:

### 1. In `src/main.rs` (HYBRID mode decision point)

**Line 575 (IR success):**
```rust
Ok(val) => {
    eprintln!("🎯 [EXEC PATH] IR executor (planner → NumericFunc::SHF_PTW_NLN_DLOG → exec)");
    result = val;
}
```

**Line 587 (Legacy fallback):**
```rust
eprintln!("🎯 [EXEC PATH] Legacy evaluator (builtin_dlog_cols)");
result = rt.eval(&expr)?;
```

### 2. In `src/exec.rs:157` (IR executor)

```rust
NumericFunc::SHF_PTW_NLN_DLOG => {
    eprintln!("  ↳ [IR EXEC] NumericFunc::SHF_PTW_NLN_DLOG @ src/exec.rs:157");
    dlog_column(col, 1)
}
```

### 3. In `src/builtins.rs:3471` (Builtin entry)

```rust
fn builtin_dlog_cols(_rt: &mut Runtime, args: &[Value]) -> Result<Value, String> {
    eprintln!("  ↳ [BUILTIN] builtin_dlog_cols @ src/builtins.rs:3471");
    let lag = match args.len() {
        ...
```

### 4. In `src/planner.rs:123` (Planning decision)

```rust
"dlog" => {
    eprintln!("  ↳ [PLANNER] Mapping 'dlog' → NumericFunc::SHF_PTW_NLN_DLOG @ src/planner.rs:123");
    plan_unary(NumericFunc::SHF_PTW_NLN_DLOG, &elements[1..], plan, ctx, interner)
}
```

---

## Test Commands

```bash
cd /home/ubuntu/blisp

# Test IR path (should use planner)
echo '(dlog (file "test.csv") 1)' | cargo run --quiet -- -e '(dlog (file "test.csv") 1)'

# Test legacy path
echo '(dlog (file "test.csv") 1)' | cargo run --quiet -- --legacy -e '(dlog (file "test.csv") 1)'

# Test IR-only
echo '(dlog (file "test.csv") 1)' | cargo run --quiet -- --ir-only -e '(dlog (file "test.csv") 1)'
```

---

## Expected Output

### IR Path (Hybrid/IR-Only):
```
🎯 [EXEC PATH] IR executor (planner → NumericFunc::SHF_PTW_NLN_DLOG → exec)
  ↳ [PLANNER] Mapping 'dlog' → NumericFunc::SHF_PTW_NLN_DLOG @ src/planner.rs:123
  ↳ [IR EXEC] NumericFunc::SHF_PTW_NLN_DLOG @ src/exec.rs:157
```

### Legacy Path (Legacy mode):
```
🎯 [EXEC PATH] Legacy evaluator (builtin_dlog_cols)
  ↳ [BUILTIN] builtin_dlog_cols @ src/builtins.rs:3471
```

---

## Key Finding

**The token "dlog" is BOTH:**
1. Registered as builtin: `builtins.rs:74` → `builtin_dlog_cols`
2. In planner: `planner.rs:123` → `NumericFunc::SHF_PTW_NLN_DLOG`

**The builtin registration is IGNORED in HYBRID/IR-ONLY modes** because:
- The IR path (`try_ir_eval`) is tried FIRST
- It succeeds for Frame operations
- The builtin path is only used as fallback in HYBRID mode
- In LEGACY mode, builtin is always used (RT.eval checks `is_builtin` first)

---

## Call Stack Summary

### IR Path Full Stack:
```
blisp::main()
  → eval_code(rt, code, use_legacy=false, use_ir_only=false)
    → try_ir_eval(rt, expr)                      [main.rs:543]
      → normalize::normalize(expr, interner)      [main.rs:545]
      → planner::plan(&normalized, interner)      [main.rs:548]
        → planner.rs:123 "dlog" match arm
          → plan_unary(NumericFunc::SHF_PTW_NLN_DLOG, ...)
      → exec::execute(&plan, rt)                  [main.rs:551]
        → exec.rs:157 match NumericFunc::SHF_PTW_NLN_DLOG
          → dlog_column(col, 1)                   [exec.rs:1092]
```

### Legacy Path Full Stack:
```
blisp::main()
  → eval_code(rt, code, use_legacy=false, use_ir_only=false)
    → try_ir_eval(rt, expr.clone())              [main.rs:574]
      → planner::plan() fails (e.g., not a Frame)
    → rt.eval(&expr)                             [main.rs:587 - FALLBACK]
      → eval.rs:82 is_builtin("dlog") → true
      → eval.rs:90 call_builtin("dlog", args)
        → builtins.rs:74 lookup "dlog" → builtin_dlog_cols
          → builtin_dlog_cols(rt, args)          [builtins.rs:3471]
            → dlog_column(col, lag)              [exec.rs:1092 - SAME HELPER!]
```

---

## Critical Insight

**Both paths use the SAME helper function:**
```rust
// src/exec.rs:1092
fn dlog_column(col: &Column, _lag: usize) -> Column { ... }
```

So the algorithm is identical! The difference is:
- **IR path**: Goes through planner/optimizer (can fuse operations)
- **Legacy path**: Direct function call (no optimization)

The IR path is faster because:
1. Schema validation at plan time (no runtime checks)
2. Fusion opportunities (e.g., fuse dlog + cs1)
3. Single-pass execution with known types

