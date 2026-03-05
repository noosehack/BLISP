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

/// Rewrite events emitted during normalization (for --pipe inspector)
#[derive(Debug, Clone)]
pub enum RewriteEvent {
    /// `(-> x (f) (g))` expanded — form_count is number of pipeline steps (excluding -> and initial value)
    ThreadFirst { form_count: usize },
    /// Alias rewrite: `from` → `to` (exact symbols from canonical_name table)
    Alias { from: String, to: String },
    /// 2-param arg-order swap: `(op param data)` → `(op data param)`
    ArgSwap2 { op: String },
    /// 3-param arg-order swap: `(op w step data)` → `(op data w step)`
    ArgSwap3 { op: String },
}

/// Result of traced normalization
pub struct NormalizeTrace {
    pub expr: CanonExpr,
    pub post_thread: Expr,
    pub rewrites: Vec<RewriteEvent>,
}

/// Normalize an expression into canonical form
///
/// Two passes:
/// 1. Thread-first (`->`) expansion
/// 2. Canonicalization: alias rewrite + arg-order rewrite (prefix → data-first)
///
/// The result is idempotent: normalize(normalize(x)) == normalize(x)
pub fn normalize(expr: Expr, interner: &mut Interner) -> CanonExpr {
    let expanded = normalize_expr(expr, interner);
    let canonical = canonicalize_expr_impl(expanded, interner, None);
    CanonExpr(canonical)
}

/// Normalize with full tracing for --pipe inspector
pub fn normalize_traced(expr: Expr, interner: &mut Interner) -> NormalizeTrace {
    let mut rewrites = Vec::new();
    let expanded = normalize_expr_traced(expr, interner, &mut rewrites);
    let post_thread = expanded.clone();
    let canonical = canonicalize_expr_impl(expanded, interner, Some(&mut rewrites));
    NormalizeTrace {
        expr: CanonExpr(canonical),
        post_thread,
        rewrites,
    }
}

/// Internal recursive normalization (traced variant — emits RewriteEvent::ThreadFirst)
fn normalize_expr_traced(
    expr: Expr,
    interner: &mut Interner,
    events: &mut Vec<RewriteEvent>,
) -> Expr {
    match expr {
        Expr::List(elements) => {
            if elements.is_empty() {
                return Expr::List(elements);
            }
            if let Expr::Sym(sym) = &elements[0] {
                let name = interner.resolve(*sym);
                if name == "->" {
                    // Record the -> expansion: form_count = pipeline steps (excluding -> and initial value)
                    let form_count = if elements.len() >= 2 {
                        elements.len() - 2
                    } else {
                        0
                    };
                    events.push(RewriteEvent::ThreadFirst { form_count });
                    return normalize_thread_first(&elements[1..], interner);
                }
            }
            let normalized = elements
                .into_iter()
                .map(|e| normalize_expr_traced(e, interner, events))
                .collect();
            Expr::List(normalized)
        }
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Str(_) | Expr::Sym(_) | Expr::Nil => {
            expr
        }
        Expr::Quote(inner) => {
            Expr::Quote(Box::new(normalize_expr_traced(*inner, interner, events)))
        }
        Expr::QuasiQuote(inner) => {
            Expr::QuasiQuote(Box::new(normalize_expr_traced(*inner, interner, events)))
        }
        Expr::Unquote(inner) => {
            Expr::Unquote(Box::new(normalize_expr_traced(*inner, interner, events)))
        }
        Expr::UnquoteSplicing(inner) => {
            Expr::UnquoteSplicing(Box::new(normalize_expr_traced(*inner, interner, events)))
        }
    }
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

// ─── Canonicalization ────────────────────────────────────────────────────────
//
// Phase 2: After `->` expansion, rewrite aliases and fix arg order.
//
// 1. Alias rewrite: legacy spellings → canonical names
//    e.g. (dlog-cols x) → (dlog x), (cs1-col x) → (cs1 x)
//
// 2. Arg-order rewrite: prefix form → data-first (ambiguity-safe)
//    e.g. (rolling-mean 250 x) → (rolling-mean x 250)
//    Only rewrites when roles are unambiguous (exactly one Int vs non-Int).

/// Alias table: legacy spelling → canonical name (single source of truth)
///
/// `canonical_name()` is a lookup into this table.
/// `dic --matrix` reads this table directly for the NORMALIZE→ column.
pub const NORMALIZE_ALIASES: &[(&str, &str)] = &[
    ("dlog-cols", "dlog"),
    ("dlog-col", "dlog"),
    // Canonical renames: old hyphenated → new underscore names
    ("rolling-mean", "rol_avg"),
    ("rolling-std", "rol_std"),
    ("rolling-zscore", "rol_zsc"),
    ("wzs", "rol_zsc"),
    ("cs1", "run_sum"),
    ("ur", "rsk_adj"),
    // -cols aliases for renamed ops
    ("cs1-cols", "run_sum"),
    ("cs1-col", "run_sum"),
    ("ur-cols", "rsk_adj"),
    ("ur-col", "rsk_adj"),
    ("rolling-mean-cols", "rol_avg"),
    ("rolling-std-cols", "rol_std"),
    ("rolling-zscore-cols", "rol_zsc"),
    ("rol_avg_cols", "rol_avg"),
    ("rol_std_cols", "rol_std"),
    ("rol_zsc_cols", "rol_zsc"),
    ("run_sum_cols", "run_sum"),
    ("rsk_adj_cols", "rsk_adj"),
    // Other aliases
    ("shift-cols", "shift"),
    ("shift-col", "shift"),
    (">-cols", ">"),
    (">-col", ">"),
    ("locf-cols", "locf"),
    ("diff-cols", "diff"),
    ("diff-col", "diff"),
    ("ecs1-cols", "ecs1"),
    ("ecs1-col", "ecs1"),
    ("x-", "xminus"),
    ("w5", "wkd"),
    ("let*", "let"),
    // Word-form aliases for arithmetic operators
    ("add", "+"),
    ("sub", "-"),
    ("mul", "*"),
    ("div", "/"),
    // Word-form aliases for comparison operators
    ("eq", "=="),
    ("neq", "!="),
    ("gt", ">"),
    ("gte", ">="),
    ("lt", "<"),
    ("lte", "<="),
    // Math aliases
    ("ln", "log"),
];

/// Lookup legacy spelling → canonical name
fn canonical_name(name: &str) -> Option<&'static str> {
    NORMALIZE_ALIASES
        .iter()
        .find(|(from, _)| *from == name)
        .map(|(_, to)| *to)
}

/// 2-param ops that take (op param data) in prefix form.
/// Canonical (data-first) form: (op data param)
const PARAM_OPS_2: &[&str] = &[
    "rol_avg",
    "rol_std",
    "rolling-mean-min2",
    "rolling-std-min2",
    "rol_zsc",
    "shift",
    "diff",
    "keep",
    "lag-obs",
    "shift-obs",
    "ft-mean",
    "ft-std",
    "ft-zscore",
];

/// 3-param ops that take (op w step data) in prefix form.
/// Canonical (data-first) form: (op data w step)
const PARAM_OPS_3: &[&str] = &["rol_zsc", "rsk_adj"];

/// Returns true if the expression is a literal integer
fn is_int(e: &Expr) -> bool {
    matches!(e, Expr::Int(_))
}

/// Canonicalize a single expression (post-threading), with optional event capture
fn canonicalize_expr_impl(
    expr: Expr,
    interner: &mut Interner,
    mut events: Option<&mut Vec<RewriteEvent>>,
) -> Expr {
    match expr {
        Expr::List(elements) => {
            if elements.is_empty() {
                return Expr::List(elements);
            }

            // First, recursively canonicalize all sub-expressions
            let mut elements: Vec<Expr> = elements
                .into_iter()
                .map(|e| canonicalize_expr_impl(e, interner, events.as_deref_mut()))
                .collect();

            // Rewrite alias in function position
            if let Expr::Sym(sym) = &elements[0] {
                let name = interner.resolve(*sym);
                if let Some(canonical) = canonical_name(name) {
                    if let Some(ref mut ev) = events {
                        ev.push(RewriteEvent::Alias {
                            from: name.to_string(),
                            to: canonical.to_string(),
                        });
                    }
                    let new_sym = interner.intern(canonical);
                    elements[0] = Expr::Sym(new_sym);
                }
            }

            // Arg-order rewrite (prefix → data-first), ambiguity-safe
            if let Expr::Sym(sym) = &elements[0] {
                let name = interner.resolve(*sym).to_string();

                // 2-param ops: (op a b) where a=Int, b=non-Int → (op b a)
                if elements.len() == 3 && PARAM_OPS_2.contains(&name.as_str()) {
                    if is_int(&elements[1]) && !is_int(&elements[2]) {
                        if let Some(ref mut ev) = events {
                            ev.push(RewriteEvent::ArgSwap2 { op: name.clone() });
                        }
                        elements.swap(1, 2);
                    }
                }

                // 3-param ops: (op a b c) where a=Int, b=Int, c=non-Int → (op c a b)
                if elements.len() == 4 && PARAM_OPS_3.contains(&name.as_str()) {
                    if is_int(&elements[1]) && is_int(&elements[2]) && !is_int(&elements[3]) {
                        if let Some(ref mut ev) = events {
                            ev.push(RewriteEvent::ArgSwap3 { op: name.clone() });
                        }
                        let data = elements.remove(3);
                        elements.insert(1, data);
                    }
                }
            }

            Expr::List(elements)
        }
        // Atoms pass through
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Str(_) | Expr::Sym(_) | Expr::Nil => {
            expr
        }
        // Quote forms
        Expr::Quote(inner) => {
            Expr::Quote(Box::new(canonicalize_expr_impl(*inner, interner, events)))
        }
        Expr::QuasiQuote(inner) => {
            Expr::QuasiQuote(Box::new(canonicalize_expr_impl(*inner, interner, events)))
        }
        Expr::Unquote(inner) => {
            Expr::Unquote(Box::new(canonicalize_expr_impl(*inner, interner, events)))
        }
        Expr::UnquoteSplicing(inner) => {
            Expr::UnquoteSplicing(Box::new(canonicalize_expr_impl(*inner, interner, events)))
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

    fn int(n: i64) -> Expr {
        Expr::Int(n)
    }

    // ─── Canonicalization tests ─────────────────────────────────────────

    #[test]
    fn test_alias_rewrite() {
        let mut interner = Interner::new();

        // (dlog-cols x) => (dlog x)
        let expr = Expr::List(vec![sym(&mut interner, "dlog-cols"), num(1.0)]);
        let result = normalize(expr, &mut interner);
        let expected = Expr::List(vec![sym(&mut interner, "dlog"), num(1.0)]);
        assert_eq!(result.inner(), &expected);
    }

    #[test]
    fn test_alias_cs1() {
        let mut interner = Interner::new();

        // (cs1-col x) => (run_sum x) via cs1-col → run_sum alias
        let expr = Expr::List(vec![sym(&mut interner, "cs1-col"), num(1.0)]);
        let result = normalize(expr, &mut interner);
        let expected = Expr::List(vec![sym(&mut interner, "run_sum"), num(1.0)]);
        assert_eq!(result.inner(), &expected);
    }

    #[test]
    fn test_arg_reorder_2param_prefix() {
        let mut interner = Interner::new();

        // Prefix: (rolling-mean 250 x) → alias to rol_avg → reorder → (rol_avg x 250)
        let expr = Expr::List(vec![
            sym(&mut interner, "rolling-mean"),
            int(250),
            sym(&mut interner, "x"),
        ]);
        let result = normalize(expr, &mut interner);
        let expected = Expr::List(vec![
            sym(&mut interner, "rol_avg"),
            sym(&mut interner, "x"),
            int(250),
        ]);
        assert_eq!(result.inner(), &expected);
    }

    #[test]
    fn test_arg_reorder_2param_already_data_first() {
        let mut interner = Interner::new();

        // Already data-first: (rolling-mean x 250) → alias to (rol_avg x 250)
        let expr = Expr::List(vec![
            sym(&mut interner, "rolling-mean"),
            sym(&mut interner, "x"),
            int(250),
        ]);
        let result = normalize(expr, &mut interner);
        let expected = Expr::List(vec![
            sym(&mut interner, "rol_avg"),
            sym(&mut interner, "x"),
            int(250),
        ]);
        assert_eq!(result.inner(), &expected);
    }

    #[test]
    fn test_arg_reorder_2param_ambiguous() {
        let mut interner = Interner::new();

        // Ambiguous: (shift 2 3) — both are Int, don't rewrite
        let expr = Expr::List(vec![sym(&mut interner, "shift"), int(2), int(3)]);
        let result = normalize(expr, &mut interner);
        let expected = Expr::List(vec![sym(&mut interner, "shift"), int(2), int(3)]);
        assert_eq!(result.inner(), &expected);
    }

    #[test]
    fn test_arg_reorder_3param_prefix() {
        let mut interner = Interner::new();

        // Prefix: (ur 250 5 x) → alias to rsk_adj → reorder → (rsk_adj x 250 5)
        let expr = Expr::List(vec![
            sym(&mut interner, "ur"),
            int(250),
            int(5),
            sym(&mut interner, "x"),
        ]);
        let result = normalize(expr, &mut interner);
        let expected = Expr::List(vec![
            sym(&mut interner, "rsk_adj"),
            sym(&mut interner, "x"),
            int(250),
            int(5),
        ]);
        assert_eq!(result.inner(), &expected);
    }

    #[test]
    fn test_arg_reorder_3param_already_data_first() {
        let mut interner = Interner::new();

        // Already data-first: (ur x 250 5) → alias to (rsk_adj x 250 5)
        let expr = Expr::List(vec![
            sym(&mut interner, "ur"),
            sym(&mut interner, "x"),
            int(250),
            int(5),
        ]);
        let result = normalize(expr, &mut interner);
        let expected = Expr::List(vec![
            sym(&mut interner, "rsk_adj"),
            sym(&mut interner, "x"),
            int(250),
            int(5),
        ]);
        assert_eq!(result.inner(), &expected);
    }

    #[test]
    fn test_thread_then_canonicalize() {
        let mut interner = Interner::new();

        // Full pipeline: (-> x (rolling-mean 250)) → alias + thread → (rol_avg x 250)
        let expr = Expr::List(vec![
            sym(&mut interner, "->"),
            sym(&mut interner, "x"),
            Expr::List(vec![sym(&mut interner, "rolling-mean"), int(250)]),
        ]);
        let result = normalize(expr, &mut interner);
        let expected = Expr::List(vec![
            sym(&mut interner, "rol_avg"),
            sym(&mut interner, "x"),
            int(250),
        ]);
        assert_eq!(result.inner(), &expected);
    }

    #[test]
    fn test_alias_plus_reorder() {
        let mut interner = Interner::new();

        // (shift-cols 2 x) → alias to (shift 2 x) → reorder to (shift x 2)
        let expr = Expr::List(vec![
            sym(&mut interner, "shift-cols"),
            int(2),
            sym(&mut interner, "x"),
        ]);
        let result = normalize(expr, &mut interner);
        let expected = Expr::List(vec![
            sym(&mut interner, "shift"),
            sym(&mut interner, "x"),
            int(2),
        ]);
        assert_eq!(result.inner(), &expected);
    }

    #[test]
    fn test_canonicalize_idempotent() {
        let mut interner = Interner::new();

        // Prefix: (rolling-mean 250 x) → (rol_avg x 250)
        // Second normalize should be identity
        let expr = Expr::List(vec![
            sym(&mut interner, "rolling-mean"),
            int(250),
            sym(&mut interner, "x"),
        ]);
        let once = normalize(expr, &mut interner);
        let twice = normalize(once.inner().clone(), &mut interner);
        assert_eq!(once, twice, "Canonicalization must be idempotent");
    }

    #[test]
    fn test_nested_canonicalize() {
        let mut interner = Interner::new();

        // Nested: (f (dlog-cols x)) → (f (dlog x))
        let expr = Expr::List(vec![
            sym(&mut interner, "f"),
            Expr::List(vec![sym(&mut interner, "dlog-cols"), num(1.0)]),
        ]);
        let result = normalize(expr, &mut interner);
        let expected = Expr::List(vec![
            sym(&mut interner, "f"),
            Expr::List(vec![sym(&mut interner, "dlog"), num(1.0)]),
        ]);
        assert_eq!(result.inner(), &expected);
    }
}
