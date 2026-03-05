//! Runtime: combines lexical and global environments with symbol interner

use crate::ast::{Interner, SymbolId};
use crate::builtins::BuiltinFn;
use crate::env::{GlobalEnv, LexicalEnv};
use crate::value::Value;
use std::collections::HashMap;

/// Runtime state for blisp evaluation
pub struct Runtime {
    pub lexical: LexicalEnv,
    pub global: GlobalEnv,
    pub interner: Interner,
    pub builtins: HashMap<SymbolId, BuiltinFn>,
    pub macros: HashMap<SymbolId, Value>,
    gensym_counter: usize,
}

impl Runtime {
    pub fn new() -> Self {
        let mut rt = Self {
            lexical: LexicalEnv::new(),
            global: GlobalEnv::new(),
            interner: Interner::new(),
            builtins: HashMap::new(),
            macros: HashMap::new(),
            gensym_counter: 0,
        };

        // Register all builtins
        crate::builtins::register_builtins(&mut rt);

        rt
    }

    /// Register a builtin function
    pub fn register_builtin(&mut self, name: &str, func: BuiltinFn) {
        let sym = self.interner.intern(name);
        self.builtins.insert(sym, func);
    }

    /// Check if a symbol is a builtin function
    pub fn is_builtin(&self, sym: SymbolId) -> bool {
        self.builtins.contains_key(&sym)
    }

    /// Return all registered builtin names (resolved from interner)
    pub fn builtin_names(&self) -> Vec<String> {
        self.builtins
            .keys()
            .map(|sym| self.interner.resolve(*sym).to_string())
            .collect()
    }

    /// Call a builtin function
    pub fn call_builtin(&mut self, sym: SymbolId, args: &[Value]) -> Result<Value, String> {
        if let Some(func) = self.builtins.get(&sym) {
            func(self, args)
        } else {
            let name = self.interner.resolve(sym);
            Err(format!("Unknown builtin: {}", name))
        }
    }

    /// Resolve a symbol to its value
    ///
    /// Resolution order:
    /// 1. Check lexical environment (inner to outer)
    /// 2. Check global environment
    /// 3. Error if not found
    pub fn resolve(&self, sym: SymbolId) -> Result<Value, String> {
        // Check lexical first
        if let Some(val) = self.lexical.resolve(sym) {
            return Ok(val.clone());
        }

        // Check global
        if let Some(val) = self.global.resolve(sym) {
            return Ok(val.clone());
        }

        // Not found
        let name = self.interner.resolve(sym);
        Err(format!("Undefined variable: {}", name))
    }

    /// Define a global variable (defparameter)
    pub fn define(&mut self, sym: SymbolId, val: Value) {
        self.global.define(sym, val);
    }

    /// Set a variable (setf semantics)
    ///
    /// If the variable exists in lexical scope, update it there.
    /// Otherwise, update (or create) in global scope.
    pub fn set(&mut self, sym: SymbolId, val: Value) -> Result<(), String> {
        // Try to update in lexical
        if self.lexical.set(sym, val.clone()) {
            return Ok(());
        }

        // Otherwise update global (creates if doesn't exist)
        self.global.define(sym, val);
        Ok(())
    }

    /// Push a new lexical frame (enter scope)
    pub fn push_frame(&mut self) {
        self.lexical.push_frame();
    }

    /// Pop the innermost lexical frame (exit scope)
    pub fn pop_frame(&mut self) {
        self.lexical.pop_frame();
    }

    /// Define a variable in the current lexical frame
    /// Used by let* to bind variables
    pub fn define_local(&mut self, sym: SymbolId, val: Value) {
        self.lexical.define_local(sym, val);
    }

    /// Define a macro
    pub fn define_macro(&mut self, sym: SymbolId, macro_val: Value) {
        self.macros.insert(sym, macro_val);
    }

    /// Lookup a macro
    pub fn lookup_macro(&self, sym: SymbolId) -> Option<&Value> {
        self.macros.get(&sym)
    }

    /// Generate a unique symbol (for macro hygiene)
    pub fn gensym(&mut self, prefix: Option<&str>) -> SymbolId {
        let prefix = prefix.unwrap_or("G");
        self.gensym_counter += 1;
        let name = format!("{}#{}", prefix, self.gensym_counter);
        self.interner.intern(&name)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_global() {
        let mut rt = Runtime::new();
        let x = rt.interner.intern("x");

        rt.define(x, Value::Int(42));
        assert_eq!(rt.resolve(x).unwrap(), Value::Int(42));
    }

    #[test]
    fn test_resolve_undefined() {
        let mut rt = Runtime::new();
        let x = rt.interner.intern("undefined-var"); // Interned but not defined

        assert!(rt.resolve(x).is_err());
    }

    #[test]
    fn test_resolve_lexical() {
        let mut rt = Runtime::new();
        let x = rt.interner.intern("x");

        rt.push_frame();
        rt.define_local(x, Value::Int(100));

        assert_eq!(rt.resolve(x).unwrap(), Value::Int(100));
    }

    #[test]
    fn test_lexical_shadows_global() {
        let mut rt = Runtime::new();
        let x = rt.interner.intern("x");

        // Define global
        rt.define(x, Value::Int(1));

        // Define lexical (shadows)
        rt.push_frame();
        rt.define_local(x, Value::Int(2));

        // Lexical wins
        assert_eq!(rt.resolve(x).unwrap(), Value::Int(2));

        // Pop frame
        rt.pop_frame();

        // Global visible again
        assert_eq!(rt.resolve(x).unwrap(), Value::Int(1));
    }

    #[test]
    fn test_setf_global() {
        let mut rt = Runtime::new();
        let x = rt.interner.intern("x");

        rt.define(x, Value::Int(10));
        rt.set(x, Value::Int(20)).unwrap();

        assert_eq!(rt.resolve(x).unwrap(), Value::Int(20));
    }

    #[test]
    fn test_setf_lexical() {
        let mut rt = Runtime::new();
        let x = rt.interner.intern("x");

        // Global x
        rt.define(x, Value::Int(1));

        // Lexical x
        rt.push_frame();
        rt.define_local(x, Value::Int(2));

        // setf updates lexical
        rt.set(x, Value::Int(20)).unwrap();
        assert_eq!(rt.resolve(x).unwrap(), Value::Int(20));

        // Pop frame, global unchanged
        rt.pop_frame();
        assert_eq!(rt.resolve(x).unwrap(), Value::Int(1)); // Still 1!
    }

    #[test]
    fn test_setf_creates_global_if_not_exists() {
        let mut rt = Runtime::new();
        let x = rt.interner.intern("x");

        // setf on undefined variable creates global
        rt.set(x, Value::Int(42)).unwrap();
        assert_eq!(rt.resolve(x).unwrap(), Value::Int(42));
    }

    #[test]
    fn test_multiple_frames() {
        let mut rt = Runtime::new();
        let x = rt.interner.intern("x");
        let y = rt.interner.intern("y");

        // Frame 1: x=1
        rt.push_frame();
        rt.define_local(x, Value::Int(1));

        // Frame 2: x=2, y=10
        rt.push_frame();
        rt.define_local(x, Value::Int(2));
        rt.define_local(y, Value::Int(10));

        assert_eq!(rt.resolve(x).unwrap(), Value::Int(2));
        assert_eq!(rt.resolve(y).unwrap(), Value::Int(10));

        // Pop frame 2
        rt.pop_frame();
        assert_eq!(rt.resolve(x).unwrap(), Value::Int(1));
        assert!(rt.resolve(y).is_err()); // y no longer visible

        // Pop frame 1
        rt.pop_frame();
        assert!(rt.resolve(x).is_err()); // x no longer visible
    }

    #[test]
    fn test_interner_integration() {
        let mut rt = Runtime::new();

        let foo = rt.interner.intern("foo");
        let bar = rt.interner.intern("bar");
        let foo2 = rt.interner.intern("foo"); // Should reuse

        assert_eq!(foo, foo2);
        assert_ne!(foo, bar);

        rt.define(foo, Value::Int(42));
        assert_eq!(rt.resolve(foo).unwrap(), Value::Int(42));
        assert!(rt.resolve(bar).is_err());
    }
}
