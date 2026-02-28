//! Lexical and global environments for variable binding
#![allow(clippy::doc_lazy_continuation)]

use crate::ast::SymbolId;
use crate::value::Value;
use std::collections::HashMap;

/// Lexical environment (stack of scopes)
///
/// Used for let* bindings, function arguments (later), etc.
/// Inner scopes shadow outer scopes.
pub struct LexicalEnv {
    frames: Vec<HashMap<SymbolId, Value>>,
}

impl LexicalEnv {
    pub fn new() -> Self {
        Self { frames: Vec::new() }
    }

    /// Push a new lexical frame (enter new scope)
    pub fn push_frame(&mut self) {
        self.frames.push(HashMap::new());
    }

    /// Pop the innermost lexical frame (exit scope)
    pub fn pop_frame(&mut self) {
        self.frames.pop();
    }

    /// Resolve a symbol in lexical scope (search inner to outer)
    pub fn resolve(&self, sym: SymbolId) -> Option<&Value> {
        for frame in self.frames.iter().rev() {
            if let Some(val) = frame.get(&sym) {
                return Some(val);
            }
        }
        None
    }

    /// Update a symbol in lexical scope if it exists
    /// Returns true if found and updated
    pub fn set(&mut self, sym: SymbolId, val: Value) -> bool {
        for frame in self.frames.iter_mut().rev() {
            if frame.contains_key(&sym) {
                frame.insert(sym, val);
                return true;
            }
        }
        false
    }

    /// Define a symbol in the current (innermost) lexical frame
    /// Used by let* to add bindings
    pub fn define_local(&mut self, sym: SymbolId, val: Value) {
        if let Some(frame) = self.frames.last_mut() {
            frame.insert(sym, val);
        }
    }

    /// Get the current depth (number of frames)
    #[allow(dead_code)]
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Create a snapshot of the current lexical environment (for closures)
    pub fn snapshot(&self) -> crate::value::LexicalSnapshot {
        self.frames.clone()
    }

    /// Restore a captured environment and push a new frame for parameters
    pub fn restore_and_push(&mut self, snapshot: &crate::value::LexicalSnapshot) {
        self.frames = snapshot.clone();
        self.push_frame();
    }
}

impl Default for LexicalEnv {
    fn default() -> Self {
        Self::new()
    }
}

/// Global environment (persistent bindings)
///
/// Used for defparameter, top-level definitions, etc.
pub struct GlobalEnv {
    bindings: HashMap<SymbolId, Value>,
}

impl GlobalEnv {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Resolve a symbol in global scope
    pub fn resolve(&self, sym: SymbolId) -> Option<&Value> {
        self.bindings.get(&sym)
    }

    /// Define or update a global variable
    pub fn define(&mut self, sym: SymbolId, val: Value) {
        self.bindings.insert(sym, val);
    }

    /// Get the number of global bindings
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Check if empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

impl Default for GlobalEnv {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_define_resolve() {
        let mut global = GlobalEnv::new();
        let sym = SymbolId(0);

        global.define(sym, Value::Int(42));
        assert_eq!(global.resolve(sym), Some(&Value::Int(42)));
    }

    #[test]
    fn test_global_update() {
        let mut global = GlobalEnv::new();
        let sym = SymbolId(0);

        global.define(sym, Value::Int(10));
        global.define(sym, Value::Int(20)); // Update
        assert_eq!(global.resolve(sym), Some(&Value::Int(20)));
    }

    #[test]
    fn test_lexical_single_frame() {
        let mut lexical = LexicalEnv::new();
        let sym = SymbolId(0);

        lexical.push_frame();
        lexical.define_local(sym, Value::Int(100));
        assert_eq!(lexical.resolve(sym), Some(&Value::Int(100)));

        lexical.pop_frame();
        assert_eq!(lexical.resolve(sym), None); // Gone after pop
    }

    #[test]
    fn test_lexical_shadowing() {
        let mut lexical = LexicalEnv::new();
        let sym = SymbolId(0);

        // Outer frame
        lexical.push_frame();
        lexical.define_local(sym, Value::Int(1));

        // Inner frame shadows
        lexical.push_frame();
        lexical.define_local(sym, Value::Int(2));

        assert_eq!(lexical.resolve(sym), Some(&Value::Int(2))); // Inner wins

        // Pop inner
        lexical.pop_frame();
        assert_eq!(lexical.resolve(sym), Some(&Value::Int(1))); // Outer visible again
    }

    #[test]
    fn test_lexical_set() {
        let mut lexical = LexicalEnv::new();
        let sym = SymbolId(0);

        lexical.push_frame();
        lexical.define_local(sym, Value::Int(10));

        // Update existing binding
        assert!(lexical.set(sym, Value::Int(20)));
        assert_eq!(lexical.resolve(sym), Some(&Value::Int(20)));

        // Try to update non-existent
        let other_sym = SymbolId(1);
        assert!(!lexical.set(other_sym, Value::Int(30)));
    }

    #[test]
    fn test_lexical_set_updates_correct_frame() {
        let mut lexical = LexicalEnv::new();
        let x = SymbolId(0);
        let y = SymbolId(1);

        // Outer frame: x=1
        lexical.push_frame();
        lexical.define_local(x, Value::Int(1));

        // Inner frame: x=2, y=10
        lexical.push_frame();
        lexical.define_local(x, Value::Int(2));
        lexical.define_local(y, Value::Int(10));

        // Update x in inner frame
        lexical.set(x, Value::Int(20));
        assert_eq!(lexical.resolve(x), Some(&Value::Int(20)));

        // Pop inner, check outer x unchanged
        lexical.pop_frame();
        assert_eq!(lexical.resolve(x), Some(&Value::Int(1))); // Still 1!
    }

    #[test]
    fn test_lexical_depth() {
        let mut lexical = LexicalEnv::new();
        assert_eq!(lexical.depth(), 0);

        lexical.push_frame();
        assert_eq!(lexical.depth(), 1);

        lexical.push_frame();
        assert_eq!(lexical.depth(), 2);

        lexical.pop_frame();
        assert_eq!(lexical.depth(), 1);
    }
}
