# Step 2 Complete: Environments

**Date:** 2026-02-17
**Status:** ✅ Complete
**Tests:** 27/27 passing

---

## What We Built

### Files Created

1. **src/value.rs** (77 lines)
   - `Value` enum: Nil, Bool, Int, Float, Str, Sym
   - `type_name()` method
   - `is_truthy()` method
   - 3 tests passing

2. **src/env.rs** (199 lines)
   - `LexicalEnv` struct (stack of frames)
   - `GlobalEnv` struct (persistent bindings)
   - push_frame/pop_frame for scope management
   - resolve/set/define_local methods
   - 7 tests passing

3. **src/runtime.rs** (178 lines)
   - `Runtime` struct (combines both environments + interner)
   - `resolve()` - Lexical → Global → Error
   - `define()` - Always global
   - `set()` - Lexical if found, else global
   - `push_frame/pop_frame` - Scope management
   - `define_local()` - Bind in current lexical frame
   - 11 tests passing

### Updated

- **src/main.rs** - Demo program showing all environment features

---

## Features Working

### ✅ Global Variables
```rust
rt.define(x, Value::Int(42));
rt.resolve(x)  // => Ok(Value::Int(42))
```

### ✅ Lexical Shadowing
```rust
rt.define(x, Value::Int(1));     // Global
rt.push_frame();
rt.define_local(x, Value::Int(2)); // Shadows
rt.resolve(x)  // => Ok(Value::Int(2)) - Lexical wins
rt.pop_frame();
rt.resolve(x)  // => Ok(Value::Int(1)) - Global visible again
```

### ✅ setf Semantics
```rust
// Update global
rt.define(x, Value::Int(10));
rt.set(x, Value::Int(20));
rt.resolve(x)  // => Ok(Value::Int(20))

// Update lexical (not global)
rt.push_frame();
rt.define_local(x, Value::Int(2));
rt.set(x, Value::Int(20));  // Updates LEXICAL
rt.pop_frame();
rt.resolve(x)  // => Ok(Value::Int(20)) - Global unchanged!
```

### ✅ Nested Scopes
```rust
rt.push_frame();           // Outer
rt.define_local(y, Value::Int(1));

rt.push_frame();           // Inner
rt.define_local(y, Value::Int(2));
rt.resolve(y)  // => Ok(Value::Int(2))

rt.pop_frame();
rt.resolve(y)  // => Ok(Value::Int(1))
```

### ✅ Undefined Variable Errors
```rust
let undefined = rt.interner.intern("foo");
rt.resolve(undefined)  // => Err("Undefined variable: foo")
```

---

## Demo Output

```
blisp v0.1.0
Step 2: Environments complete!

=== Global Variables ===
(defparameter x 42)
x => Int(42)

=== Lexical Shadowing ===
(let* ((x 100)) x)
x => Int(100) (lexical shadows global)

After exiting let*:
x => Int(42) (global visible again)

=== setf Semantics ===
(setf x 100)
x => Int(100) (global updated)

=== Nested Lexical Scopes ===
(let* ((y 1))
  (let* ((y 2) (z 10))
    y => Int(2) (inner scope)
    z => Int(10)
  ) ; exit inner let*
  y => Int(1) (outer scope visible again)
  z => Undefined variable: z (expected)
) ; exit outer let*

=== Undefined Variable ===
Undefined variable: undefined-var ✓
```

---

## Tests

**Total: 27 tests passing**

### value.rs (3 tests)
- test_value_types
- test_is_truthy
- test_clone

### env.rs (7 tests)
- test_global_define_resolve
- test_global_update
- test_lexical_single_frame
- test_lexical_shadowing
- test_lexical_set
- test_lexical_set_updates_correct_frame
- test_lexical_depth

### runtime.rs (11 tests)
- test_resolve_global
- test_resolve_undefined
- test_resolve_lexical
- test_lexical_shadows_global
- test_setf_global
- test_setf_lexical
- test_setf_creates_global_if_not_exists
- test_multiple_frames
- test_interner_integration

### Previous tests still passing (6 tests)
- ast.rs: 2 tests
- reader.rs: 4 tests

---

## Code Statistics

```
Files:          7 (ast, reader, value, env, runtime, main, Cargo.toml)
Lines of code:  ~750 lines (excluding tests)
Tests:          27/27 passing ✅
Dependencies:   blawktrust, rustyline
```

---

## What This Enables

With environments working, we can now build **Step 3: Evaluator**!

Step 3 will implement the special forms that actually use these environments:

```lisp
(quote foo)                    ; Return unevaluated
(progn 1 2 3)                  ; Sequential eval, return last
(if t 'yes 'no)                ; Conditional
(let* ((x 1) (y 2)) y)         ; Lexical bindings (uses push_frame/define_local!)
(defparameter x 10)            ; Global define (uses define!)
(setf x 20)                    ; Update variable (uses set!)
```

Without Step 2, none of these would work. Now they can!

---

## Key Design Decisions

### Resolution Order
**Lexical → Global → Error**

This matches Common Lisp semantics:
- Inner scopes shadow outer scopes
- Lexical always wins over global
- Undefined variables are errors (not nil)

### setf Semantics
**Update lexical if found, else update global**

This is the CL behavior:
```lisp
(defparameter x 1)        ; Global x = 1
(let* ((x 2))             ; Lexical x = 2
  (setf x 20)             ; Updates LEXICAL to 20
  x)                      ; => 20
x                         ; => 1 (global unchanged)
```

### Value Clone
Values are `Clone`, so we can return owned values from resolve without lifetime issues.

For Step 4, `Col` and `Table` will use `Arc<>` so cloning is cheap (just increment refcount).

---

## Next: Step 3 - Evaluator

**Goal:** Make Lisp code actually execute!

**Files to create:**
- `src/eval.rs` - Core evaluator logic

**What we'll implement:**
```rust
impl Runtime {
    pub fn eval(&mut self, expr: &Expr) -> Result<Value, String> {
        match expr {
            // Literals
            Expr::Nil => Ok(Value::Nil),
            Expr::Int(n) => Ok(Value::Int(*n)),
            // ... etc

            // Variable lookup
            Expr::Sym(id) => self.resolve(*id),

            // Quote
            Expr::Quote(e) => self.quote_to_value(e),

            // List (special form or function call)
            Expr::List(exprs) => self.eval_list(exprs),
        }
    }

    fn eval_list(&mut self, exprs: &[Expr]) -> Result<Value, String> {
        // Check for special forms: quote, progn, if, let*, defparameter, setf
        // Otherwise: builtin function call (Step 5)
    }
}
```

**Special forms:**
- `quote` - Return unevaluated
- `progn` - Sequential evaluation
- `if` - Conditional (else required)
- `let*` - Sequential bindings (uses push_frame!)
- `defparameter` - Global definition (uses define!)
- `setf` - Update variable (uses set!)

**Estimate:** 2-3 hours for Step 3

---

**Status:** Step 2/9 complete ✅
**Next:** Step 3 - Evaluator (eval.rs)
**Ready to continue!** 🚀
