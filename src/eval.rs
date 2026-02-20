//! Evaluator: execute Lisp expressions
//!
//! Implements the core eval() function and all special forms.

use crate::ast::{Expr, SymbolId};
use crate::runtime::Runtime;
use crate::value::Value;

impl Runtime {
    /// Evaluate an expression
    pub fn eval(&mut self, expr: &Expr) -> Result<Value, String> {
        match expr {
            // Literals evaluate to themselves
            Expr::Nil => Ok(Value::Nil),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Int(n) => Ok(Value::Int(*n)),
            Expr::Float(f) => Ok(Value::Float(*f)),
            Expr::Str(s) => Ok(Value::Str(s.clone().into())),

            // Symbols resolve to their values
            Expr::Sym(id) => self.resolve(*id),

            // Quote returns the inner expression unevaluated
            Expr::Quote(e) => self.expr_to_value(e),

            // Quasiquote (Phase 3)
            Expr::QuasiQuote(inner) => self.eval_quasiquote(inner, 1),
            Expr::Unquote(_) => {
                Err("unquote outside quasiquote".to_string())
            }
            Expr::UnquoteSplicing(_) => {
                Err("unquote-splicing outside quasiquote".to_string())
            }

            // Lists are either special forms or function calls
            Expr::List(exprs) => self.eval_list(exprs),
        }
    }

    /// Evaluate a list (special form or function call)
    fn eval_list(&mut self, exprs: &[Expr]) -> Result<Value, String> {
        if exprs.is_empty() {
            return Ok(Value::Nil);
        }

        // Macroexpand first (if it's a macro call)
        if let Expr::Sym(head_sym) = &exprs[0] {
            let head_name = self.interner.resolve(*head_sym);

            // Don't macroexpand special forms
            let is_special_form = matches!(head_name,
                "quote" | "progn" | "if" | "let*" | "defparameter" | "setf" | "define" | "lambda" | "defmacro" | "->"
            );

            if !is_special_form && self.lookup_macro(*head_sym).is_some() {
                // Macroexpand and then evaluate the result
                let expanded = self.macroexpand_1(&Expr::List(exprs.to_vec()))?;
                return self.eval(&expanded);
            }
        }

        // Evaluate head to see if it's a lambda (or check if it's a special form first)
        let head_val = if let Expr::Sym(head_sym) = &exprs[0] {
            let head_name = self.interner.resolve(*head_sym);

            // Check for special forms first
            match head_name {
                "quote" => return self.eval_quote(&exprs[1..]),
                "progn" => return self.eval_progn(&exprs[1..]),
                "if" => return self.eval_if(&exprs[1..]),
                "let*" => return self.eval_let_star(&exprs[1..]),
                "defparameter" => return self.eval_defparameter(&exprs[1..]),
                "setf" => return self.eval_setf(&exprs[1..]),
                "define" => return self.eval_define(&exprs[1..]),
                "lambda" => return self.eval_lambda(&exprs[1..]),
                "defmacro" => return self.eval_defmacro(&exprs[1..]),
                "->" => return self.eval_thread(&exprs[1..]),
                _ => {}
            }

            // Check if it's a builtin function
            if self.is_builtin(*head_sym) {
                // Evaluate arguments
                let mut arg_vals = Vec::new();
                for arg in &exprs[1..] {
                    arg_vals.push(self.eval(arg)?);
                }

                // Call builtin
                return self.call_builtin(*head_sym, &arg_vals);
            }

            // Try to resolve as variable (might be lambda)
            self.resolve(*head_sym)?
        } else {
            // Head is not a symbol, evaluate it (for lambda expressions)
            self.eval(&exprs[0])?
        };

        // Check if head_val is a lambda
        if let Value::Lambda { params, body, env } = head_val {
            return self.apply_lambda(&params, &body, &env, &exprs[1..]);
        }

        // Not a callable
        Err(format!("Value is not callable: {}", head_val.type_name()))
    }

    /// (quote expr) - Return expression unevaluated
    fn eval_quote(&mut self, args: &[Expr]) -> Result<Value, String> {
        if args.len() != 1 {
            return Err(format!("quote expects 1 argument, got {}", args.len()));
        }
        self.expr_to_value(&args[0])
    }

    /// (progn expr1 expr2 ...) - Evaluate sequentially, return last
    fn eval_progn(&mut self, forms: &[Expr]) -> Result<Value, String> {
        if forms.is_empty() {
            return Ok(Value::Nil);
        }

        let mut result = Value::Nil;
        for form in forms {
            result = self.eval(form)?;
        }
        Ok(result)
    }

    /// (-> x (f a) (g b) ...) - Threading macro
    /// Threads x through a series of function calls
    /// (-> x (f a)) => (f x a)
    /// (-> x (f a) (g b)) => (g (f x a) b)
    fn eval_thread(&mut self, args: &[Expr]) -> Result<Value, String> {
        if args.is_empty() {
            return Err("-> expects at least 1 argument".to_string());
        }

        // Evaluate the initial value
        let mut result = self.eval(&args[0])?;

        // Use a temporary variable to hold the threaded value
        // This avoids needing to convert complex Values back to Exprs
        let temp_sym = self.interner.intern("__thread_tmp__");

        // Thread through each form
        for form in &args[1..] {
            // Bind the current result to the temp variable
            self.global.define(temp_sym, result.clone());

            result = match form {
                Expr::List(exprs) if !exprs.is_empty() => {
                    // (f a b) with result becomes (f __thread_tmp__ a b)
                    let mut new_exprs = vec![exprs[0].clone()];
                    new_exprs.push(Expr::Sym(temp_sym));
                    new_exprs.extend_from_slice(&exprs[1..]);
                    self.eval(&Expr::List(new_exprs))?
                }
                Expr::Sym(s) => {
                    // Just a symbol f becomes (f __thread_tmp__)
                    self.eval(&Expr::List(vec![Expr::Sym(*s), Expr::Sym(temp_sym)]))?
                }
                _ => {
                    return Err(format!("-> expects list or symbol forms, got {:?}", form));
                }
            };
        }

        Ok(result)
    }

    /// (if cond then else) - Conditional evaluation (else is required)
    fn eval_if(&mut self, args: &[Expr]) -> Result<Value, String> {
        if args.len() != 3 {
            return Err(format!("if expects 3 arguments (cond then else), got {}", args.len()));
        }

        let cond = self.eval(&args[0])?;

        if cond.is_truthy() {
            self.eval(&args[1])
        } else {
            self.eval(&args[2])
        }
    }

    /// (let* ((var1 val1) (var2 val2) ...) body...)
    /// Sequential bindings: each binding can use previous ones
    fn eval_let_star(&mut self, args: &[Expr]) -> Result<Value, String> {
        if args.is_empty() {
            return Err("let* expects bindings and body".to_string());
        }

        // First arg must be bindings list
        let Expr::List(bindings) = &args[0] else {
            return Err("let* bindings must be a list".to_string());
        };

        // Push new lexical frame
        self.push_frame();

        // Process bindings sequentially
        for binding in bindings {
            let Expr::List(pair) = binding else {
                self.pop_frame();
                return Err("let* binding must be a list (var val)".to_string());
            };

            if pair.len() != 2 {
                self.pop_frame();
                return Err("let* binding must have exactly 2 elements (var val)".to_string());
            }

            let Expr::Sym(var_sym) = pair[0] else {
                self.pop_frame();
                return Err("let* binding variable must be a symbol".to_string());
            };

            // Evaluate value (can reference previous bindings!)
            let val = self.eval(&pair[1])?;

            // Bind in current frame
            self.define_local(var_sym, val);
        }

        // Evaluate body forms
        let result = self.eval_progn(&args[1..])?;

        // Pop frame
        self.pop_frame();

        Ok(result)
    }

    /// (defparameter var val) - Define global variable
    fn eval_defparameter(&mut self, args: &[Expr]) -> Result<Value, String> {
        if args.len() != 2 {
            return Err(format!("defparameter expects 2 arguments (var val), got {}", args.len()));
        }

        let Expr::Sym(var_sym) = args[0] else {
            return Err("defparameter variable must be a symbol".to_string());
        };

        let val = self.eval(&args[1])?;
        self.define(var_sym, val.clone());
        Ok(val)
    }

    /// (setf var val) - Update variable (lexical or global)
    fn eval_setf(&mut self, args: &[Expr]) -> Result<Value, String> {
        if args.len() != 2 {
            return Err(format!("setf expects 2 arguments (var val), got {}", args.len()));
        }

        let Expr::Sym(var_sym) = args[0] else {
            return Err("setf variable must be a symbol".to_string());
        };

        let val = self.eval(&args[1])?;
        self.set(var_sym, val.clone())?;
        Ok(val)
    }

    /// (define name expr) - Define global variable
    fn eval_define(&mut self, args: &[Expr]) -> Result<Value, String> {
        if args.len() != 2 {
            return Err(format!("define expects 2 arguments (name expr), got {}", args.len()));
        }

        let Expr::Sym(name_sym) = args[0] else {
            return Err("define: first argument must be a symbol".to_string());
        };

        let val = self.eval(&args[1])?;
        self.define(name_sym, val.clone());
        Ok(val)
    }

    /// (lambda (params...) body...) - Create closure
    fn eval_lambda(&mut self, args: &[Expr]) -> Result<Value, String> {
        if args.len() < 2 {
            return Err("lambda expects (params) body...".to_string());
        }

        // Parse parameter list
        let Expr::List(param_exprs) = &args[0] else {
            return Err("lambda: first argument must be parameter list".to_string());
        };

        let mut params = Vec::new();
        for param_expr in param_exprs {
            let Expr::Sym(sym) = param_expr else {
                return Err("lambda: parameters must be symbols".to_string());
            };
            params.push(*sym);
        }

        // Body is remaining forms
        let body = args[1..].to_vec();

        // Capture current lexical environment
        let env = self.lexical.snapshot();

        Ok(Value::Lambda { params, body, env })
    }

    /// Apply a lambda function
    fn apply_lambda(
        &mut self,
        params: &[SymbolId],
        body: &[Expr],
        captured_env: &crate::value::LexicalSnapshot,
        args: &[Expr],
    ) -> Result<Value, String> {
        // Evaluate arguments
        let mut arg_vals = Vec::new();
        for arg in args {
            arg_vals.push(self.eval(arg)?);
        }

        if arg_vals.len() != params.len() {
            return Err(format!(
                "lambda expects {} arguments, got {}",
                params.len(),
                arg_vals.len()
            ));
        }

        // Restore captured environment and push new frame
        self.lexical.restore_and_push(captured_env);

        // Bind parameters
        for (param, val) in params.iter().zip(arg_vals) {
            self.define_local(*param, val);
        }

        // Evaluate body (implicit progn)
        let result = self.eval_progn(body)?;

        // Pop frame
        self.pop_frame();

        Ok(result)
    }

    /// Evaluate quasiquote with proper nesting and splicing
    fn eval_quasiquote(&mut self, expr: &Expr, depth: usize) -> Result<Value, String> {
        match expr {
            Expr::Unquote(inner) => {
                if depth == 1 {
                    // Unquote at depth 1: evaluate
                    self.eval(inner)
                } else {
                    // Nested: decrement depth and recurse
                    let inner_val = self.eval_quasiquote(inner, depth - 1)?;
                    let unquote_sym = self.interner.intern("unquote");
                    Ok(Value::List(vec![Value::Sym(unquote_sym), inner_val]))
                }
            }

            Expr::UnquoteSplicing(inner) => {
                if depth == 1 {
                    // Cannot splice at top level
                    return Err("unquote-splicing not in list context".to_string());
                } else {
                    // Nested: decrement depth
                    let inner_val = self.eval_quasiquote(inner, depth - 1)?;
                    let splice_sym = self.interner.intern("unquote-splicing");
                    Ok(Value::List(vec![Value::Sym(splice_sym), inner_val]))
                }
            }

            Expr::QuasiQuote(inner) => {
                // Nested quasiquote: increment depth
                let inner_val = self.eval_quasiquote(inner, depth + 1)?;
                let qq_sym = self.interner.intern("quasiquote");
                Ok(Value::List(vec![Value::Sym(qq_sym), inner_val]))
            }

            Expr::List(exprs) => {
                // Process list elements, handling splicing
                let mut result = Vec::new();

                for e in exprs {
                    if let Expr::UnquoteSplicing(inner) = e {
                        if depth == 1 {
                            // Evaluate and splice
                            let val = self.eval(inner)?;
                            if let Value::List(items) = val {
                                result.extend(items);
                            } else {
                                return Err("unquote-splicing requires list".to_string());
                            }
                        } else {
                            // Nested: recurse with depth
                            let val = self.eval_quasiquote(e, depth)?;
                            result.push(val);
                        }
                    } else {
                        // Regular element: recurse
                        let val = self.eval_quasiquote(e, depth)?;
                        result.push(val);
                    }
                }

                Ok(Value::List(result))
            }

            // Atoms: return as-is (quoted)
            _ => self.expr_to_value(expr),
        }
    }

    /// (defmacro name (params...) body...)
    fn eval_defmacro(&mut self, args: &[Expr]) -> Result<Value, String> {
        if args.len() < 3 {
            return Err("defmacro expects name (params) body...".to_string());
        }

        let Expr::Sym(name_sym) = args[0] else {
            return Err("defmacro: name must be a symbol".to_string());
        };

        // Parse params
        let Expr::List(param_exprs) = &args[1] else {
            return Err("defmacro: second argument must be parameter list".to_string());
        };

        let mut params = Vec::new();
        for param_expr in param_exprs {
            let Expr::Sym(sym) = param_expr else {
                return Err("defmacro: parameters must be symbols".to_string());
            };
            params.push(*sym);
        }

        // Body
        let body = args[2..].to_vec();

        // Capture environment
        let env = self.lexical.snapshot();

        let macro_val = Value::Macro { params, body, env };
        self.define_macro(name_sym, macro_val);

        Ok(Value::Sym(name_sym))
    }

    /// Expand macros recursively to fixed point
    pub fn macroexpand(&mut self, expr: &Expr) -> Result<Expr, String> {
        let mut current = expr.clone();
        let mut count = 0;
        const MAX_EXPANSIONS: usize = 1000;

        loop {
            let expanded = self.macroexpand_1(&current)?;
            if expanded == current {
                // Fixed point reached
                return Ok(current);
            }
            current = expanded;
            count += 1;
            if count > MAX_EXPANSIONS {
                return Err("macro expansion exceeded limit (possible infinite loop)".to_string());
            }
        }
    }

    /// Expand macros one level
    pub fn macroexpand_1(&mut self, expr: &Expr) -> Result<Expr, String> {
        // Only expand lists that start with a macro symbol
        let Expr::List(exprs) = expr else {
            return Ok(expr.clone());
        };

        if exprs.is_empty() {
            return Ok(expr.clone());
        }

        let Expr::Sym(head_sym) = exprs[0] else {
            return Ok(expr.clone());
        };

        // Check if head is a macro
        if let Some(macro_val) = self.lookup_macro(head_sym).cloned() {
            let Value::Macro { params, body, env } = macro_val else {
                return Err("internal error: macro value is not Macro variant".to_string());
            };

            // Apply macro to *unevaluated* arguments
            let args = &exprs[1..];

            // Convert args from Expr to Value (unevaluated)
            let mut arg_vals = Vec::new();
            for arg in args {
                arg_vals.push(self.expr_to_value(arg)?);
            }

            if arg_vals.len() != params.len() {
                return Err(format!(
                    "macro {} expects {} arguments, got {}",
                    self.interner.resolve(head_sym),
                    params.len(),
                    arg_vals.len()
                ));
            }

            // CRITICAL: Save caller's lexical environment before macro expansion
            // Macro expansion happens in the macro's captured env, but the expanded
            // code must be evaluated in the caller's env (to see let* bindings, etc.)
            let caller_env = self.lexical.snapshot();

            // Execute macro body with arguments bound in macro's environment
            self.lexical.restore_and_push(&env);

            for (param, val) in params.iter().zip(arg_vals) {
                self.define_local(*param, val);
            }

            // Evaluate macro body with error-safe env restoration
            let result_val = match self.eval_progn(&body) {
                Ok(v) => v,
                Err(e) => {
                    // Restore caller env even on macro body error
                    self.lexical.restore_and_push(&caller_env);
                    self.pop_frame();
                    return Err(e);
                }
            };

            // CRITICAL: Restore caller's environment before evaluating expansion
            // This ensures the expanded code sees the caller's bindings (e.g., let*)
            self.lexical.restore_and_push(&caller_env);
            self.pop_frame();  // Remove the empty frame added by restore_and_push

            // Convert result Value back to Expr
            let expanded_expr = self.value_to_expr(&result_val)?;

            return Ok(expanded_expr);
        }

        // Not a macro: return as-is (don't recurse into subforms for now)
        Ok(expr.clone())
    }

    /// Convert Value back to Expr (inverse of expr_to_value) - public for builtins
    pub fn value_to_expr(&self, val: &Value) -> Result<Expr, String> {
        match val {
            Value::Nil => Ok(Expr::Nil),
            Value::Bool(b) => Ok(Expr::Bool(*b)),
            Value::Int(n) => Ok(Expr::Int(*n)),
            Value::Float(f) => Ok(Expr::Float(*f)),
            Value::Str(s) => Ok(Expr::Str(s.to_string())),
            Value::Sym(id) => Ok(Expr::Sym(*id)),
            Value::List(items) => {
                let mut exprs = Vec::new();
                for item in items {
                    exprs.push(self.value_to_expr(item)?);
                }
                Ok(Expr::List(exprs))
            }
            _ => Err(format!("cannot convert {} to expression", val.type_name())),
        }
    }

    /// Convert an AST expression to a runtime Value (for quote) - public for builtins
    pub fn expr_to_value(&mut self, expr: &Expr) -> Result<Value, String> {
        match expr {
            Expr::Nil => Ok(Value::Nil),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Int(n) => Ok(Value::Int(*n)),
            Expr::Float(f) => Ok(Value::Float(*f)),
            Expr::Str(s) => Ok(Value::Str(s.clone().into())),
            Expr::Sym(id) => Ok(Value::Sym(*id)),
            Expr::Quote(inner) => {
                // Nested quote: return (quote <inner>)
                let quote_sym = self.interner.intern("quote");
                let inner_val = self.expr_to_value(inner)?;
                Ok(Value::List(vec![Value::Sym(quote_sym), inner_val]))
            }
            Expr::QuasiQuote(inner) => {
                // Convert quasiquote to data structure
                let qq_sym = self.interner.intern("quasiquote");
                let inner_val = self.expr_to_value(inner)?;
                Ok(Value::List(vec![Value::Sym(qq_sym), inner_val]))
            }
            Expr::Unquote(inner) => {
                // Convert unquote to data structure
                let unq_sym = self.interner.intern("unquote");
                let inner_val = self.expr_to_value(inner)?;
                Ok(Value::List(vec![Value::Sym(unq_sym), inner_val]))
            }
            Expr::UnquoteSplicing(inner) => {
                // Convert unquote-splicing to data structure
                let splice_sym = self.interner.intern("unquote-splicing");
                let inner_val = self.expr_to_value(inner)?;
                Ok(Value::List(vec![Value::Sym(splice_sym), inner_val]))
            }
            Expr::List(exprs) => {
                // Convert each element
                let mut vals = Vec::new();
                for e in exprs {
                    vals.push(self.expr_to_value(e)?);
                }
                Ok(Value::List(vals))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::Reader;

    fn eval_str(rt: &mut Runtime, input: &str) -> Result<Value, String> {
        let mut reader = Reader::new(input).map_err(|e| format!("Parse error: {}", e))?;
        let expr = reader.read(&mut rt.interner).map_err(|e| format!("Read error: {}", e))?;
        rt.eval(&expr)
    }

    #[test]
    fn test_eval_literals() {
        let mut rt = Runtime::new();

        assert_eq!(eval_str(&mut rt, "42").unwrap(), Value::Int(42));
        assert_eq!(eval_str(&mut rt, "3.14").unwrap(), Value::Float(3.14));
        assert_eq!(eval_str(&mut rt, "t").unwrap(), Value::Bool(true));
        assert_eq!(eval_str(&mut rt, "nil").unwrap(), Value::Nil);
    }

    #[test]
    fn test_eval_quote() {
        let mut rt = Runtime::new();

        // Quote symbol
        let result = eval_str(&mut rt, "'foo").unwrap();
        assert!(matches!(result, Value::Sym(_)));

        // Quote number
        assert_eq!(eval_str(&mut rt, "'42").unwrap(), Value::Int(42));
    }

    #[test]
    fn test_eval_progn() {
        let mut rt = Runtime::new();

        // progn returns last value
        assert_eq!(eval_str(&mut rt, "(progn 1 2 3)").unwrap(), Value::Int(3));

        // Empty progn
        assert_eq!(eval_str(&mut rt, "(progn)").unwrap(), Value::Nil);

        // Side effects happen
        eval_str(&mut rt, "(progn (defparameter x 10) (defparameter y 20))").unwrap();
        assert_eq!(eval_str(&mut rt, "x").unwrap(), Value::Int(10));
        assert_eq!(eval_str(&mut rt, "y").unwrap(), Value::Int(20));
    }

    #[test]
    fn test_eval_if() {
        let mut rt = Runtime::new();

        // True branch
        assert_eq!(eval_str(&mut rt, "(if t 'yes 'no)").unwrap(), Value::Sym(rt.interner.intern("yes")));

        // False branch
        assert_eq!(eval_str(&mut rt, "(if nil 'yes 'no)").unwrap(), Value::Sym(rt.interner.intern("no")));

        // 0 is truthy in Lisp
        assert_eq!(eval_str(&mut rt, "(if 0 'yes 'no)").unwrap(), Value::Sym(rt.interner.intern("yes")));
    }

    #[test]
    fn test_eval_defparameter() {
        let mut rt = Runtime::new();

        // Define variable
        eval_str(&mut rt, "(defparameter x 42)").unwrap();
        assert_eq!(eval_str(&mut rt, "x").unwrap(), Value::Int(42));

        // Redefine
        eval_str(&mut rt, "(defparameter x 100)").unwrap();
        assert_eq!(eval_str(&mut rt, "x").unwrap(), Value::Int(100));
    }

    #[test]
    fn test_eval_setf() {
        let mut rt = Runtime::new();

        // setf on global
        eval_str(&mut rt, "(defparameter x 10)").unwrap();
        eval_str(&mut rt, "(setf x 20)").unwrap();
        assert_eq!(eval_str(&mut rt, "x").unwrap(), Value::Int(20));

        // setf creates if doesn't exist
        eval_str(&mut rt, "(setf y 30)").unwrap();
        assert_eq!(eval_str(&mut rt, "y").unwrap(), Value::Int(30));
    }

    #[test]
    fn test_eval_let_star() {
        let mut rt = Runtime::new();

        // Simple let*
        let result = eval_str(&mut rt, "(let* ((x 10)) x)").unwrap();
        assert_eq!(result, Value::Int(10));

        // Sequential bindings
        let result = eval_str(&mut rt, "(let* ((x 1) (y (progn x))) y)").unwrap();
        // Note: Can't add yet, so we just return x
        assert_eq!(result, Value::Int(1));

        // let* doesn't leak
        assert!(eval_str(&mut rt, "x").is_err());
    }

    #[test]
    fn test_eval_let_star_shadowing() {
        let mut rt = Runtime::new();

        eval_str(&mut rt, "(defparameter x 1)").unwrap();

        // let* shadows global
        let result = eval_str(&mut rt, "(let* ((x 2)) x)").unwrap();
        assert_eq!(result, Value::Int(2));

        // Global unchanged
        assert_eq!(eval_str(&mut rt, "x").unwrap(), Value::Int(1));
    }

    #[test]
    fn test_eval_let_star_sequential() {
        let mut rt = Runtime::new();

        // Second binding can use first
        let result = eval_str(&mut rt, "(let* ((x 10) (y x)) y)").unwrap();
        assert_eq!(result, Value::Int(10));
    }

    #[test]
    fn test_eval_nested_let_star() {
        let mut rt = Runtime::new();

        let result = eval_str(&mut rt,
            "(let* ((x 1)) (let* ((x 2)) x))"
        ).unwrap();
        assert_eq!(result, Value::Int(2));
    }

    #[test]
    fn test_eval_complex_expression() {
        let mut rt = Runtime::new();

        let program = r#"
            (progn
                (defparameter x 10)
                (let* ((x 20))
                    (setf x 30)
                    x))
        "#;

        let result = eval_str(&mut rt, program).unwrap();
        assert_eq!(result, Value::Int(30));

        // Global unchanged
        assert_eq!(eval_str(&mut rt, "x").unwrap(), Value::Int(10));
    }

    #[test]
    fn test_eval_if_nested() {
        let mut rt = Runtime::new();

        let result = eval_str(&mut rt,
            "(if t (if nil 'inner-yes 'inner-no) 'outer-no)"
        ).unwrap();

        assert_eq!(result, Value::Sym(rt.interner.intern("inner-no")));
    }

    #[test]
    fn test_undefined_variable_error() {
        let mut rt = Runtime::new();

        let result = eval_str(&mut rt, "undefined-var");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Undefined variable"));
    }

    #[test]
    fn test_unknown_function_error() {
        let mut rt = Runtime::new();

        let result = eval_str(&mut rt, "(unknown-fn 1 2 3)");
        assert!(result.is_err());
        let err = result.unwrap_err();
        // After variable lookup support, unknown functions show as undefined variables
        assert!(err.contains("Undefined variable") || err.contains("Unknown function"));
    }
}
