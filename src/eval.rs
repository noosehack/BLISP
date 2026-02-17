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

            // Lists are either special forms or function calls
            Expr::List(exprs) => self.eval_list(exprs),
        }
    }

    /// Evaluate a list (special form or function call)
    fn eval_list(&mut self, exprs: &[Expr]) -> Result<Value, String> {
        if exprs.is_empty() {
            return Ok(Value::Nil);
        }

        // Head must be a symbol
        let Expr::Sym(head_sym) = &exprs[0] else {
            return Err("First element of list must be a symbol".to_string());
        };

        let head_name = self.interner.resolve(*head_sym);

        // Check for special forms first
        match head_name {
            "quote" => return self.eval_quote(&exprs[1..]),
            "progn" => return self.eval_progn(&exprs[1..]),
            "if" => return self.eval_if(&exprs[1..]),
            "let*" => return self.eval_let_star(&exprs[1..]),
            "defparameter" => return self.eval_defparameter(&exprs[1..]),
            "setf" => return self.eval_setf(&exprs[1..]),
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

        // Unknown function
        Err(format!("Unknown function or special form: {}", head_name))
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

    /// Convert an AST expression to a runtime Value (for quote)
    fn expr_to_value(&mut self, expr: &Expr) -> Result<Value, String> {
        match expr {
            Expr::Nil => Ok(Value::Nil),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Int(n) => Ok(Value::Int(*n)),
            Expr::Float(f) => Ok(Value::Float(*f)),
            Expr::Str(s) => Ok(Value::Str(s.clone().into())),
            Expr::Sym(id) => Ok(Value::Sym(*id)),
            Expr::Quote(inner) => self.expr_to_value(inner),
            Expr::List(_) => {
                // For now, quoted lists become nil
                // TODO: Step 4 - proper list support
                Ok(Value::Nil)
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
        assert!(result.unwrap_err().contains("Unknown function"));
    }
}
