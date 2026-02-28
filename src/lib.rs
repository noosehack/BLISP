//! BLISP Library
//!
//! Exposes modules for testing and embedding

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::single_match)]
#![allow(clippy::new_without_default)]
#![allow(clippy::collapsible_if)]
#![allow(irrefutable_let_patterns)]

pub mod ast;
pub mod builtins;
pub mod env;
pub mod eval;
pub mod exec;
pub mod frame;
pub mod io;
pub mod ir;
pub mod ir_fusion;
pub mod mask;
pub mod normalize;
pub mod planner;
pub mod reader;
pub mod runtime;
pub mod value;
