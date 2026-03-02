/// BLADE Phase 3: Macro Normalization
///
/// Purpose: Transform surface syntax into canonical forms for IR compilation
///
/// Key properties:
/// 1. Idempotent: normalize(normalize(expr)) == normalize(expr)
/// 2. Meaning-preserving: eval(expr) == eval(normalize(expr))
/// 3. Syntax-only: No data inspection, pure AST rewriting
///
/// Primary transformation: Thread-first macro expansion
///   (-> a (f x) (g y z))  =>  (g (f a x) y z)
///
/// This enables pipeline optimization without changing semantics.
use crate::ast::{Expr, Interner};

/// Canonical expression after normalization
///
/// Normalized forms are "evaluation-ready" - all macros expanded,
/// all threading resolved. The IR compiler expects this form.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonExpr(pub Expr);

impl CanonExpr {
    /// Get the underlying expression
    pub fn inner(&self) -> &Expr {
        &self.0
    }

    /// Consume and return the underlying expression
    pub fn into_inner(self) -> Expr {
        self.0
    }
}

/// Normalize an expression into canonical form
///
/// This is the entry point for macro expansion. The result is guaranteed
/// to be idempotent: normalize(normalize(x)) == normalize(x)
pub fn normalize(expr: Expr, interner: &mut Interner) -> CanonExpr {
    CanonExpr(normalize_expr(expr, interner))
}

/// Internal recursive normalization
fn normalize_expr(expr: Expr, interner: &mut Interner) -> Expr {
    match expr {
        Expr::List(elements) => {
            if elements.is_empty() {
                return Expr::List(elements);
            }

            // Check if this is a thread-first macro: (-> ...)
            if let Expr::Sym(sym) = &elements[0] {
                let name = interner.resolve(*sym);
                if name == "->" {
                    return normalize_thread_first(&elements[1..], interner);
                }
            }

            // Not a macro - recursively normalize all elements
            let normalized = elements
                .into_iter()
                .map(|e| normalize_expr(e, interner))
                .collect();
            Expr::List(normalized)
        }
        // Atoms are already canonical
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Str(_) | Expr::Sym(_) | Expr::Nil => {
            expr
        }
        // Quote forms - normalize the inner expression
        Expr::Quote(inner) => Expr::Quote(Box::new(normalize_expr(*inner, interner))),
        Expr::QuasiQuote(inner) => Expr::QuasiQuote(Box::new(normalize_expr(*inner, interner))),
        Expr::Unquote(inner) => Expr::Unquote(Box::new(normalize_expr(*inner, interner))),
        Expr::UnquoteSplicing(inner) => {
            Expr::UnquoteSplicing(Box::new(normalize_expr(*inner, interner)))
        }
    }
}

/// Normalize thread-first macro: (-> a (f x) (g y))
///
/// Semantics:
///   (-> a)           => a
///   (-> a (f x))     => (f a x)
///   (-> a (f x) (g y)) => (g (f a x) y)
///
/// The initial value is threaded as the FIRST argument to each function.
fn normalize_thread_first(args: &[Expr], interner: &mut Interner) -> Expr {
    if args.is_empty() {
        // (-> ) with no args - malformed, but return nil gracefully
        return Expr::Nil;
    }

    if args.len() == 1 {
        // (-> a) => a
        return normalize_expr(args[0].clone(), interner);
    }

    // Start with the initial value
    let mut current = normalize_expr(args[0].clone(), interner);

    // Thread through each form
    for form in &args[1..] {
        current = thread_into_form(current, form.clone(), interner);
    }

    current
}

/// Thread a value into a form as the first argument
///
/// Examples:
///   thread_into_form(x, (f y z)) => (f x y z)
///   thread_into_form(x, f)       => (f x)
fn thread_into_form(value: Expr, form: Expr, interner: &mut Interner) -> Expr {
    match form {
        Expr::List(elements) => {
            // (f y z) becomes (f x y z) where x is the threaded value
            if elements.is_empty() {
                // Edge case: threading into () - treat as identity
                return value;
            }

            // Recursively normalize the form first
            let normalized_elements: Vec<Expr> = elements
                .into_iter()
                .map(|e| normalize_expr(e, interner))
                .collect();

            // Insert the threaded value as the first argument (position 1)
            let mut result = vec![normalized_elements[0].clone()];
            result.push(value);
            result.extend_from_slice(&normalized_elements[1..]);

            Expr::List(result)
        }
        Expr::Sym(_) => {
            // Threading into a bare symbol: f becomes (f x)
            let normalized_form = normalize_expr(form, interner);
            Expr::List(vec![normalized_form, value])
        }
        _ => {
            // Threading into a non-callable (number, string) - malformed
            // Return the value unchanged (graceful degradation)
            value
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Interner;

    fn sym(interner: &mut Interner, s: &str) -> Expr {
        Expr::Sym(interner.intern(s))
    }

    fn num(n: f64) -> Expr {
        Expr::Float(n)
    }

    #[test]
    fn test_normalize_atoms() {
        let mut interner = Interner::new();

        // Atoms are already canonical
        let n = num(42.0);
        let result = normalize(n.clone(), &mut interner);
        assert_eq!(result.inner(), &n);

        let s = sym(&mut interner, "foo");
        let result = normalize(s.clone(), &mut interner);
        assert_eq!(result.inner(), &s);
    }

    #[test]
    fn test_normalize_simple_list() {
        let mut interner = Interner::new();

        // (f x y) => (f x y) - no macros
        let expr = Expr::List(vec![sym(&mut interner, "f"), num(1.0), num(2.0)]);
        let result = normalize(expr.clone(), &mut interner);
        assert_eq!(result.inner(), &expr);
    }

    #[test]
    fn test_thread_first_identity() {
        let mut interner = Interner::new();

        // (-> x) => x
        let expr = Expr::List(vec![sym(&mut interner, "->"), num(42.0)]);
        let result = normalize(expr, &mut interner);
        assert_eq!(result.inner(), &num(42.0));
    }

    #[test]
    fn test_thread_first_single() {
        let mut interner = Interner::new();

        // (-> x (f y)) => (f x y)
        let expr = Expr::List(vec![
            sym(&mut interner, "->"),
            num(1.0),
            Expr::List(vec![sym(&mut interner, "f"), num(2.0)]),
        ]);

        let expected = Expr::List(vec![sym(&mut interner, "f"), num(1.0), num(2.0)]);

        let result = normalize(expr, &mut interner);
        assert_eq!(result.inner(), &expected);
    }

    #[test]
    fn test_thread_first_chain() {
        let mut interner = Interner::new();

        // (-> x (f) (g y)) => (g (f x) y)
        let expr = Expr::List(vec![
            sym(&mut interner, "->"),
            num(1.0),
            Expr::List(vec![sym(&mut interner, "f")]),
            Expr::List(vec![sym(&mut interner, "g"), num(2.0)]),
        ]);

        let expected = Expr::List(vec![
            sym(&mut interner, "g"),
            Expr::List(vec![sym(&mut interner, "f"), num(1.0)]),
            num(2.0),
        ]);

        let result = normalize(expr, &mut interner);
        assert_eq!(result.inner(), &expected);
    }

    #[test]
    fn test_thread_first_bare_symbol() {
        let mut interner = Interner::new();

        // (-> x f) => (f x)
        let expr = Expr::List(vec![
            sym(&mut interner, "->"),
            num(42.0),
            sym(&mut interner, "f"),
        ]);

        let expected = Expr::List(vec![sym(&mut interner, "f"), num(42.0)]);

        let result = normalize(expr, &mut interner);
        assert_eq!(result.inner(), &expected);
    }

    #[test]
    fn test_idempotence() {
        let mut interner = Interner::new();

        // normalize(normalize(x)) == normalize(x)
        let expr = Expr::List(vec![
            sym(&mut interner, "->"),
            num(1.0),
            Expr::List(vec![sym(&mut interner, "f"), num(2.0)]),
            Expr::List(vec![sym(&mut interner, "g"), num(3.0)]),
        ]);

        let once = normalize(expr.clone(), &mut interner);
        let twice = normalize(once.inner().clone(), &mut interner);

        assert_eq!(once, twice, "Normalization must be idempotent");
    }

    #[test]
    fn test_nested_thread_first() {
        let mut interner = Interner::new();

        // (-> (-> x (f)) (g)) => (g (f x))
        let inner = Expr::List(vec![
            sym(&mut interner, "->"),
            num(1.0),
            Expr::List(vec![sym(&mut interner, "f")]),
        ]);

        let outer = Expr::List(vec![
            sym(&mut interner, "->"),
            inner,
            Expr::List(vec![sym(&mut interner, "g")]),
        ]);

        let expected = Expr::List(vec![
            sym(&mut interner, "g"),
            Expr::List(vec![sym(&mut interner, "f"), num(1.0)]),
        ]);

        let result = normalize(outer, &mut interner);
        assert_eq!(result.inner(), &expected);
    }

    #[test]
    fn test_recursive_normalization() {
        let mut interner = Interner::new();

        // (f (-> x (g))) => (f (g x))
        let expr = Expr::List(vec![
            sym(&mut interner, "f"),
            Expr::List(vec![
                sym(&mut interner, "->"),
                num(1.0),
                Expr::List(vec![sym(&mut interner, "g")]),
            ]),
        ]);

        let expected = Expr::List(vec![
            sym(&mut interner, "f"),
            Expr::List(vec![sym(&mut interner, "g"), num(1.0)]),
        ]);

        let result = normalize(expr, &mut interner);
        assert_eq!(result.inner(), &expected);
    }
}
