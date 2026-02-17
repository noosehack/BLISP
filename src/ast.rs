//! Abstract Syntax Tree types for blisp

use std::collections::HashMap;

/// Symbol ID (interned string)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub usize);

/// AST expression
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Sym(SymbolId),
    List(Vec<Expr>),
    Quote(Box<Expr>),
}

/// String interner for symbols
pub struct Interner {
    map: HashMap<String, SymbolId>,
    names: Vec<String>,
}

impl Interner {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            names: Vec::new(),
        }
    }

    /// Intern a string and return its ID
    pub fn intern(&mut self, name: &str) -> SymbolId {
        if let Some(&id) = self.map.get(name) {
            return id;
        }
        let id = SymbolId(self.names.len());
        self.names.push(name.to_string());
        self.map.insert(name.to_string(), id);
        id
    }

    /// Resolve a symbol ID to its string
    pub fn resolve(&self, id: SymbolId) -> &str {
        &self.names[id.0]
    }

    /// Check if a string is interned
    pub fn get(&self, name: &str) -> Option<SymbolId> {
        self.map.get(name).copied()
    }
}

impl Default for Interner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interner() {
        let mut interner = Interner::new();

        let id1 = interner.intern("foo");
        let id2 = interner.intern("bar");
        let id3 = interner.intern("foo"); // Should reuse

        assert_eq!(id1, id3);
        assert_ne!(id1, id2);
        assert_eq!(interner.resolve(id1), "foo");
        assert_eq!(interner.resolve(id2), "bar");
    }

    #[test]
    fn test_expr_types() {
        let nil = Expr::Nil;
        let int = Expr::Int(42);
        let list = Expr::List(vec![Expr::Int(1), Expr::Int(2)]);

        assert_eq!(nil, Expr::Nil);
        assert_eq!(int, Expr::Int(42));
        assert_eq!(list.clone(), list);
    }
}
