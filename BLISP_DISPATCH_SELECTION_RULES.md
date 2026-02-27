# BLISP Dispatch Selection Rules - Precise Specification

**Date:** 2026-02-27
**Repository:** /home/ubuntu/blisp
**Branch:** reconstruct/tableview-only

This document precisely specifies when BLISP routes to the IR planner vs. the legacy evaluator, which path wins when both are available, and how to resolve dual-routing conflicts.

---

## The Dispatch Decision Point

### Location: `main.rs:556-609` - Function `eval_code()`

```rust
fn eval_code(rt: &mut Runtime, code: &str, use_legacy: bool, use_ir_only: bool)
    -> Result<value::Value, String>
```

This is the **single decision point** for all expression evaluation in BLISP.

---

## Three Operating Modes

### Mode Selection (main.rs:39-48)

```rust
// Parse command-line flags and environment variables
let use_ir_only = env::var("BLISP_IR_ONLY").is_ok() ||
                  args.contains(&"--ir-only".to_string());
let use_legacy = env::var("BLISP_LEGACY").is_ok() ||
                 args.contains(&"--legacy".to_string());

if use_ir_only {
    eprintln!("🚧 Running in IR-ONLY mode (Frame operations only, experimental)");
} else if use_legacy {
    eprintln!("⚠️  Running in LEGACY mode (old AST evaluator only)");
} else {
    eprintln!("✅ Running in HYBRID mode (IR for Frame ops, legacy fallback)");
}
```

**Priority:** `use_legacy` > `use_ir_only` > HYBRID (default)

### Mode 1: LEGACY-ONLY (Forced)

**Trigger:** `BLISP_LEGACY=1` or `--legacy` flag

**Location:** main.rs:565-567
```rust
if use_legacy {
    // Legacy-only mode: use old AST evaluator
    result = rt.eval(&expr)?;
}
```

**Behavior:**
- **ALL expressions** go to `rt.eval()` (eval.rs:11)
- IR planner is **never called**
- Builtins checked via `is_builtin()` (eval.rs:82)
- No schema validation, no fusion, no optimization

**Use case:** Debugging, comparing output, testing legacy path

---

### Mode 2: IR-ONLY (Experimental)

**Trigger:** `BLISP_IR_ONLY=1` or `--ir-only` flag

**Location:** main.rs:568-570
```rust
else if use_ir_only {
    // IR-only mode: force IR path (experimental, Frame ops only)
    result = try_ir_eval(rt, expr)?;
}
```

**Behavior:**
- **ALL expressions** go to `try_ir_eval()` (main.rs:543)
- Legacy evaluator is **never called**
- Fails on non-Frame operations (defparameter, if, let*, print, etc.)
- Schema validation enforced at plan time

**Use case:** Testing IR path, ensuring Frame-only pipelines, benchmarking

---

### Mode 3: HYBRID (Default)

**Trigger:** No flags (default behavior)

**Location:** main.rs:572-594
```rust
else {
    // 🎯 HYBRID mode (DEFAULT):
    // Try IR first for Frame operations, fall back to legacy for general Lisp
    match try_ir_eval(rt, expr.clone()) {
        Ok(val) => {
            // ✅ IR succeeded (Frame pipeline)
            result = val;
        }
        Err(e) if e.contains("Cannot plan") ||
                  e.contains("not supported") ||
                  e.contains("Unknown function") => {
            // IR can't handle this expression → fallback to legacy
            // This is NORMAL for general Lisp (defparameter, if, let*, etc.)
            result = rt.eval(&expr)?;
        }
        Err(e) => {
            // IR failed with real error → propagate
            return Err(e);
        }
    }
}
```

**Behavior:**
1. **ALWAYS try IR first** via `try_ir_eval(rt, expr.clone())` (main.rs:574)
2. **If IR succeeds:** Use IR result, done (main.rs:575-582)
3. **If IR fails with "Cannot plan" / "not supported" / "Unknown function":**
   - Fall back to legacy via `rt.eval(&expr)` (main.rs:587)
4. **If IR fails with other error:** Propagate error, no fallback (main.rs:589-592)

**Use case:** Normal operation - fast IR for Frame ops, legacy for general Lisp

---

## Route 1: IR Planner Path

### Entry: `main.rs:543` - Function `try_ir_eval()`

```rust
fn try_ir_eval(rt: &mut Runtime, expr: ast::Expr) -> Result<value::Value, String> {
    // Step 1: Normalize (macro expansion, desugaring)
    let normalized = normalize::normalize(expr, &mut rt.interner);  // main.rs:545

    // Step 2: Plan (AST → IR with schema validation)
    let plan = planner::plan(&normalized, &rt.interner)?;  // main.rs:548

    // Step 3: Execute (run optimized IR executor)
    let result = exec::execute(&plan, rt)?;  // main.rs:551

    Ok(result)
}
```

### When IR Routes (Succeeds):

**Condition:** Expression is recognized by planner.rs match arms

**Recognition:** planner.rs:86 `match func_name`

**Recognized tokens (36 total):**
```
file, stdin, dlog, dlog-ofs, ret, log, exp, sqrt, abs, inv, locf, wkd, cs1,
shift, lag-obs, shift-obs, keep, rolling-mean, rolling-mean-min2, ft-mean,
rolling-std, rolling-std-min2, ft-std, rolling-zscore, wzs, ur,
+, -, *, /, >,
mapr, asofr, xminus, mask-weekend, with-mask, let
```

**Success criteria:**
- All nested subexpressions are also recognized
- Schema validation passes (if applicable)
- No runtime type errors

### When IR Fails (Triggers Fallback):

**Location:** planner.rs:640, 647

**Error messages that trigger legacy fallback (main.rs:584):**
1. `"Unknown function: <token>"` (planner.rs:640)
   - Token not in planner.rs match arms
2. `"Cannot plan expression: <expr>"` (planner.rs:647)
   - Expression type not supported (e.g., non-list forms)
3. Any error containing `"not supported"`

**Example:**
```lisp
(dlog (w5 20 PRC))
;; IR tries to plan "w5" → NOT FOUND in planner.rs
;; Returns: Err("Unknown function: w5")
;; main.rs:584 catches this → falls back to rt.eval()
```

---

## Route 2: Legacy Evaluator Path

### Entry: `eval.rs:11` - Function `Runtime::eval()`

```rust
pub fn eval(&mut self, expr: &Expr) -> Result<Value, String>
```

### When Legacy Routes:

**Trigger 1:** LEGACY-ONLY mode (forced)

**Trigger 2:** HYBRID mode + IR fallback (main.rs:587)
- IR returned "Cannot plan" / "not supported" / "Unknown function"

**Trigger 3:** Special forms (never go to IR)
```rust
// eval.rs:66-79
match head_name {
    "quote" | "progn" | "if" | "let*" | "defparameter" | "setf" |
    "define" | "lambda" | "defmacro" | "->" => {
        // Handle special form directly
    }
    _ => {
        // Check builtins
    }
}
```

**Trigger 4:** Macro calls (never go to IR)
```rust
// eval.rs:55-59
if !is_special_form && self.lookup_macro(*head_sym).is_some() {
    // Macroexpand and then evaluate the result
    let expanded = self.macroexpand_1(&Expr::List(exprs.to_vec()))?;
    return self.eval(&expanded);
}
```

### Builtin Check: `eval.rs:82`

```rust
// Check if it's a builtin function
if self.is_builtin(*head_sym) {
    // Evaluate arguments
    let mut arg_vals = Vec::new();
    for arg in &exprs[1..] {
        arg_vals.push(self.eval(arg)?);
    }

    // Call builtin
    return self.call_builtin(*head_sym, &arg_vals);  // eval.rs:90
}
```

**Recognized builtins (71 total):**
All tokens registered via `rt.register_builtin()` in builtins.rs:119-222

---

## If Both Are Available: Which Wins?

### Rule: **IR ALWAYS WINS in HYBRID mode**

**Location:** main.rs:574-587

**Precedence:**
1. **IR tries first** (main.rs:574)
2. **IR succeeds** → Use IR result, done (main.rs:575-582)
3. **IR fails with "Unknown function"** → Fallback to legacy (main.rs:584-587)
4. **Legacy evaluates** → Use legacy result

### Example: `wkd` (has both IR and builtin)

```lisp
(wkd data)
```

**Execution flow in HYBRID mode:**
1. main.rs:574 calls `try_ir_eval()`
2. planner.rs:132 matches `"wkd"` → Success
3. IR creates `NumericFunc::MSK_WKE` node
4. exec.rs:134 calls `wkd_mask_weekends()`
5. **Result:** IR path used, builtin never called

**Builtin is shadowed** - `builtin_wkd` (builtins.rs:963) is never reached in HYBRID mode for Frame inputs.

### Shadowing Direction

```
┌─────────────────────────────────────┐
│  HYBRID Mode Precedence             │
├─────────────────────────────────────┤
│  1. IR Planner (planner.rs)         │  ← Wins if recognized
│     ↓ (on "Unknown function")       │
│  2. Legacy Builtin (builtins.rs)    │  ← Fallback
└─────────────────────────────────────┘
```

**Key insight:** IR planner **shadows** builtin registrations in HYBRID mode.

---

## Dual-Routing Tokens (Both Paths Available)

### Complete List (9 tokens):

| Token | IR Mapping | Builtin Registration | Status |
|-------|-----------|----------------------|--------|
| `+` | planner.rs:520 → BinaryFunc::ADD | builtins.rs:121 builtin_add | ✅ IR wins |
| `-` | planner.rs:521 → BinaryFunc::SUB | builtins.rs:122 builtin_sub | ✅ IR wins |
| `*` | planner.rs:522 → BinaryFunc::MUL | builtins.rs:123 builtin_mul | ✅ IR wins |
| `/` | planner.rs:523 → BinaryFunc::DIV | builtins.rs:124 builtin_div | ✅ IR wins |
| `>` | planner.rs:524 → BinaryFunc::GTR | builtins.rs:125 builtin_gt | ✅ IR wins |
| `mapr` | planner.rs:527 → JoinOp::ALIGN | builtins.rs:196 builtin_mapr | ✅ IR wins |
| `stdin` | planner.rs:108 → Source::Stdin | builtins.rs:148 builtin_stdin | ✅ IR wins |
| `wkd` | planner.rs:132 → NumericFunc::MSK_WKE | builtins.rs:186 builtin_wkd | ✅ IR wins |
| `xminus` | planner.rs:531 → SchemaOp::SHF_PTW_LIN_SPR | builtins.rs:188 builtin_xminus | ✅ IR wins |

**Note:** `file` is NOT dual-routing (builtin exists but for compatibility only)

### Why Dual Routing Exists

**Historical reason:** Gradual migration from legacy to IR

**Current state:** IR implementation is preferred for performance:
- Arithmetic ops: Vectorized operations in IR
- `mapr`: Schema-validated join in IR
- `wkd`: Shape-preserving mask in IR
- `xminus`: Schema-rebuilding operation in IR

**Legacy builtins remain for:**
1. Backward compatibility
2. Fallback when IR fails
3. Non-Frame inputs (scalars)

---

## Single Source of Truth Policy

### Proposal: Eliminate Dual Routing

**Goal:** Each operation should have ONE canonical implementation

**Strategy:** Choose based on operation characteristics

### Policy 1: IR-First (Recommended)

**Operations that should be IR-ONLY:**
```
+, -, *, /, >          (Frame operations - vectorized)
mapr, asofr            (Join operations - schema-validated)
wkd                    (Mask operations - shape-preserving)
xminus                 (Schema operations - colname rebuilding)
stdin                  (Source operations - Frame construction)
```

**Rationale:**
- These operations benefit from IR optimization
- Schema validation catches errors at plan time
- Fusion opportunities for chained operations
- Performance gains: 6-102x faster for rolling ops

**Implementation:**
1. **Remove builtin registrations** for IR-first operations
2. **Update error messages** to guide users:
   ```
   "Undefined variable: +"
   → "Arithmetic requires Frame input (use IR path)"
   ```
3. **Keep legacy path** for special cases (if needed):
   ```lisp
   ;; Force legacy for scalars
   BLISP_LEGACY=1 blisp -e '(+ 1 2)'
   ```

### Policy 2: Builtin-Only (For Side Effects)

**Operations that should be BUILTIN-ONLY:**
```
print, save, file-head     (I/O side effects)
type-of, len               (Introspection)
defparameter, setf         (Special forms)
```

**Rationale:**
- Side-effect operations don't fit IR's pure functional model
- Introspection needs runtime type information
- Special forms are evaluated, not planned

**Implementation:**
- **Never add to IR** for these operations
- Keep in legacy evaluator only

### Policy 3: Graduated Migration (Transition Period)

**For operations in active migration:**

**Phase 1: Soft deprecation** (current state)
- Keep both IR and builtin
- IR wins in HYBRID mode
- Builtin acts as fallback

**Phase 2: Hard deprecation**
- Remove builtin registration
- Add warning on first use:
  ```
  Warning: 'wkd' builtin is deprecated, use IR path
  ```

**Phase 3: Complete removal**
- Remove builtin implementation
- Only IR path remains

---

## Minimal Breakage Migration Plan

### Step 1: Audit Current Usage

**Check which operations are actually used via builtin path:**
```bash
# Add instrumentation to builtins.rs
fn call_builtin(&mut self, sym: SymbolId, args: &[Value]) -> Result<Value, String> {
    eprintln!("BUILTIN_CALL: {}", self.interner.resolve(sym));
    // ... existing code
}
```

### Step 2: Categorize Operations

| Category | Operations | Action |
|----------|-----------|--------|
| **High-value IR** | `+`, `-`, `*`, `/`, `>`, `mapr`, `wkd`, `xminus` | Remove builtin (Phase 3) |
| **Low-impact dual** | `stdin` | Keep dual (no strong reason to remove) |
| **Legacy-essential** | `print`, `save`, `file-head`, `type-of`, `len` | Keep builtin-only |
| **Migration target** | `dlog`, `shift`, `locf`, `cs1`, `ur` | Already IR-only ✅ |

### Step 3: Remove High-Value Builtins

**Operations to remove from builtins.rs:**
```rust
// REMOVE these registrations:
rt.register_builtin("+", builtin_add);        // Line 121
rt.register_builtin("-", builtin_sub);        // Line 122
rt.register_builtin("*", builtin_mul);        // Line 123
rt.register_builtin("/", builtin_div);        // Line 124
rt.register_builtin(">", builtin_gt);         // Line 125
rt.register_builtin("mapr", builtin_mapr);    // Line 196
rt.register_builtin("wkd", builtin_wkd);      // Line 186
rt.register_builtin("xminus", builtin_xminus); // Line 188
```

**Impact:** Users must use Frame inputs for these operations
- ✅ `(+ frame1 frame2)` - Works (IR path)
- ❌ `(+ 1 2)` - Fails "Undefined variable: +"
- 🔧 Workaround: Use `BLISP_LEGACY=1` for scalar arithmetic

### Step 4: Add IR Alias for Legacy Names

**Problem:** Users still use `w5` instead of `wkd`

**Solution:** Add alias in planner.rs:
```rust
// planner.rs:132-133 (add after "wkd")
"wkd" => plan_unary(NumericFunc::MSK_WKE, &elements[1..], plan, ctx, interner),
"w5" => plan_unary(NumericFunc::MSK_WKE, &elements[1..], plan, ctx, interner),  // Alias for wkd
```

**Impact:** Zero breakage - `w5` now works in IR

### Step 5: Deprecation Warnings

**Add warnings for legacy names:**
```rust
// planner.rs:132-135
"wkd" => plan_unary(NumericFunc::MSK_WKE, &elements[1..], plan, ctx, interner),
"w5" => {
    eprintln!("Warning: 'w5' is deprecated, use 'wkd' instead");
    plan_unary(NumericFunc::MSK_WKE, &elements[1..], plan, ctx, interner)
},
```

---

## Testing the Dispatch Rules

### Test 1: IR Wins (Dual Routing)

```bash
# Test that IR path is taken for 'wkd' (not builtin)
cat > test_ir_wins.lisp << 'EOF'
(defparameter data (file "test.csv"))
(wkd data)
EOF

# Add debug output to exec.rs:134
# Expected: "EXEC: wkd_mask_weekends() called"
# Not: "BUILTIN: wkd called"
```

### Test 2: Legacy Fallback (Unknown Function)

```bash
# Test that legacy fallback works for 'w5' (IR doesn't know it)
echo '(w5 (file "test.csv"))' | blisp
# Expected: Error "Unknown function: w5"
# Then fallback to legacy builtin_wkd
```

### Test 3: IR-Only Failure

```bash
# Test that IR-only operations fail with scalar input
echo '(dlog 42)' | blisp
# Expected IR: Err("Cannot plan: 42 is not a Frame")
# Then legacy: Err("Undefined variable: dlog")
# Final: Error "Undefined variable: dlog"
```

### Test 4: Mode Override

```bash
# Test LEGACY mode bypasses IR
BLISP_LEGACY=1 blisp -e '(+ 1 2)'
# Expected: builtin_add called (not IR)

# Test IR-ONLY mode rejects special forms
BLISP_IR_ONLY=1 blisp -e '(defparameter x 10)'
# Expected: Err("Cannot plan: defparameter")
```

---

## Summary of Dispatch Rules

### The Three Commandments

1. **IR Always Tries First** (in HYBRID mode)
   - Location: main.rs:574
   - Precondition: `!use_legacy && !use_ir_only`

2. **IR Shadows Builtins** (for recognized tokens)
   - Planner.rs match wins → builtin never called
   - Builtins only reached on "Unknown function" fallback

3. **Legacy is Universal Fallback** (for unknown tokens)
   - Catches: special forms, macros, IR-unknown builtins
   - Location: main.rs:587

### Decision Matrix

| Expression Type | LEGACY Mode | IR-ONLY Mode | HYBRID Mode |
|----------------|-------------|--------------|-------------|
| Special form (`if`, `let*`) | Legacy | ❌ Error | Legacy |
| Macro call | Legacy | ❌ Error | Legacy |
| IR-recognized token | Legacy | IR | **IR** ← Wins |
| IR-unknown, builtin-known | Legacy | ❌ Error | Legacy (fallback) |
| Unknown token | ❌ Error | ❌ Error | ❌ Error |

### Precedence Hierarchy

```
┌─────────────────────────────────────────────┐
│ 1. Mode flags (LEGACY / IR-ONLY)           │  Highest priority
├─────────────────────────────────────────────┤
│ 2. Special forms (if, let*, defparameter)   │
├─────────────────────────────────────────────┤
│ 3. Macro calls (defmacro)                   │
├─────────────────────────────────────────────┤
│ 4. IR planner (if recognized)               │  ← Shadows builtins
├─────────────────────────────────────────────┤
│ 5. Legacy builtins (fallback)               │
├─────────────────────────────────────────────┤
│ 6. Variable lookup (final fallback)         │
└─────────────────────────────────────────────┘
```

---

## Recommendations

### For BLISP Developers

1. **Remove dual-routing for arithmetic:**
   - Delete builtin registrations for `+`, `-`, `*`, `/`, `>`
   - Force users to use Frame operations (better performance)
   - Keep `BLISP_LEGACY=1` as escape hatch for scalars

2. **Add IR aliases for legacy names:**
   - Map `w5` → `wkd` in planner.rs
   - Emit deprecation warnings
   - Gradual migration with zero breakage

3. **Document IR-only operations clearly:**
   - Update error messages to guide users
   - "dlog requires Frame input" instead of "Undefined variable: dlog"

4. **Keep minimal dual-routing:**
   - Only for operations where legacy provides value:
     - `stdin` (both paths reasonable)
     - `file` (legacy for compatibility)

### For BLISP Users

1. **Prefer canonical IR names:**
   - Use `wkd` not `w5`
   - Use `dlog` not `dlog-col`

2. **Use Frame operations for performance:**
   - IR arithmetic: 6-102x faster than legacy
   - Schema validation catches errors early

3. **Understand mode selection:**
   - Default HYBRID: best of both worlds
   - LEGACY: debugging, compatibility
   - IR-ONLY: performance testing, Frame-only pipelines

---

## Code References

All line numbers verified against:
- `/home/ubuntu/blisp/src/main.rs` (dispatch decision)
- `/home/ubuntu/blisp/src/eval.rs` (legacy evaluator)
- `/home/ubuntu/blisp/src/planner.rs` (IR planner)
- `/home/ubuntu/blisp/src/builtins.rs` (builtin registrations)

**Key Functions:**
- `main.rs:556` - `eval_code()` (dispatch decision point)
- `main.rs:543` - `try_ir_eval()` (IR entry)
- `eval.rs:11` - `Runtime::eval()` (legacy entry)
- `eval.rs:82` - `is_builtin()` (builtin check)
- `planner.rs:86` - match func_name (IR recognition)
- `planner.rs:640` - "Unknown function" error (triggers fallback)

---

**End of Document**
