//! Mask system for row-level filtering (weekends, holidays, etc.)
//!
//! Design:
//! - Masks are orthogonal to NA: mask ⇒ "excluded", NA ⇒ "missing value"
//! - Named masks stored in MaskSet (weekend, holiday_US, etc.)
//! - Active mask computed from MaskExpr (boolean composition)
//! - Calendar index stays intact (masked rows remain, just excluded from compute)

use std::collections::BTreeMap;
use std::sync::Arc;
use bitvec::prelude::*;

/// Mask expression: boolean composition of named masks
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskExpr {
    Name(String),
    Not(Box<MaskExpr>),
    And(Vec<MaskExpr>),
    Or(Vec<MaskExpr>),
}

/// Named row masks (all must have same length as frame index)
#[derive(Debug, Clone)]
pub struct MaskSet {
    /// Named masks: "weekend" → BitVec, "holiday_US" → BitVec, etc.
    /// Invariant: all BitVecs have same length == index.len()
    pub row_masks: BTreeMap<String, Arc<BitVec>>,
}

impl MaskSet {
    /// Create empty mask set
    pub fn new() -> Self {
        Self {
            row_masks: BTreeMap::new(),
        }
    }

    /// Add or update a named mask
    /// Returns error if length doesn't match expected
    pub fn insert(&mut self, name: String, mask: Arc<BitVec>, expected_len: usize) -> Result<(), String> {
        if mask.len() != expected_len {
            return Err(format!(
                "Mask '{}' length {} doesn't match expected {}",
                name,
                mask.len(),
                expected_len
            ));
        }
        self.row_masks.insert(name, mask);
        Ok(())
    }

    /// Get a named mask
    pub fn get(&self, name: &str) -> Option<&Arc<BitVec>> {
        self.row_masks.get(name)
    }

    /// Check if mask exists
    pub fn contains(&self, name: &str) -> bool {
        self.row_masks.contains_key(name)
    }

    /// Merge another mask set (for binary ops)
    /// Returns error if same name has different bitsets
    pub fn merge(&mut self, other: &MaskSet) -> Result<(), String> {
        for (name, other_mask) in &other.row_masks {
            if let Some(existing_mask) = self.row_masks.get(name) {
                // Check if they're the same (pointer equality or bitwise)
                if !Arc::ptr_eq(existing_mask, other_mask) && **existing_mask != **other_mask {
                    return Err(format!(
                        "Mask '{}' collision: different bitsets with same name",
                        name
                    ));
                }
            } else {
                self.row_masks.insert(name.clone(), Arc::clone(other_mask));
            }
        }
        Ok(())
    }
}

impl Default for MaskSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Active mask: compiled BitVec + optional provenance expression
#[derive(Debug, Clone)]
pub struct ActiveMask {
    /// Provenance: how this mask was created (for debugging/replay)
    pub expr: Option<MaskExpr>,

    /// Compiled bitmask: true = masked (excluded), false = unmasked (included)
    /// Invariant: compiled.len() == index.len() always
    pub compiled: Arc<BitVec>,
}

impl ActiveMask {
    /// Create empty active mask (all unmasked)
    pub fn empty(nrows: usize) -> Self {
        Self {
            expr: None,
            compiled: Arc::new(bitvec![0; nrows]),
        }
    }

    /// Create from compiled bitvec
    pub fn from_bitvec(compiled: BitVec, expr: Option<MaskExpr>) -> Self {
        Self {
            expr,
            compiled: Arc::new(compiled),
        }
    }

    /// Check if row is masked
    #[inline]
    pub fn is_masked(&self, row: usize) -> bool {
        self.compiled[row]
    }

    /// Count masked rows
    pub fn count_masked(&self) -> usize {
        self.compiled.count_ones()
    }

    /// Count unmasked rows
    pub fn count_unmasked(&self) -> usize {
        self.compiled.count_zeros()
    }
}

/// Compile mask expression to BitVec
pub fn compile_mask_expr(
    expr: &MaskExpr,
    masks: &MaskSet,
    nrows: usize,
) -> Result<BitVec, String> {
    match expr {
        MaskExpr::Name(name) => {
            masks
                .get(name)
                .map(|m| (**m).clone())
                .ok_or_else(|| format!("Mask '{}' not found", name))
        }

        MaskExpr::Not(inner) => {
            let mut result = compile_mask_expr(inner, masks, nrows)?;
            result = !result;  // Bitwise NOT
            Ok(result)
        }

        MaskExpr::And(exprs) => {
            if exprs.is_empty() {
                return Ok(bitvec![0; nrows]);  // Empty AND = all unmasked
            }

            let mut result = compile_mask_expr(&exprs[0], masks, nrows)?;
            for expr in &exprs[1..] {
                let next = compile_mask_expr(expr, masks, nrows)?;
                result &= next;  // Bitwise AND
            }
            Ok(result)
        }

        MaskExpr::Or(exprs) => {
            if exprs.is_empty() {
                return Ok(bitvec![0; nrows]);  // Empty OR = all unmasked
            }

            let mut result = compile_mask_expr(&exprs[0], masks, nrows)?;
            for expr in &exprs[1..] {
                let next = compile_mask_expr(expr, masks, nrows)?;
                result |= next;  // Bitwise OR
            }
            Ok(result)
        }
    }
}

/// Helper: OR two active masks (for binary ops)
pub fn or_active_masks(a: &ActiveMask, b: &ActiveMask) -> ActiveMask {
    let mut result = a.compiled.as_ref().to_bitvec();
    result |= b.compiled.as_ref();

    // Combine expressions if both present
    let expr = match (&a.expr, &b.expr) {
        (Some(ae), Some(be)) => Some(MaskExpr::Or(vec![ae.clone(), be.clone()])),
        (Some(e), None) | (None, Some(e)) => Some(e.clone()),
        (None, None) => None,
    };

    ActiveMask::from_bitvec(result, expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_active_mask() {
        let mask = ActiveMask::empty(10);
        assert_eq!(mask.compiled.len(), 10);
        assert_eq!(mask.count_masked(), 0);
        assert_eq!(mask.count_unmasked(), 10);
    }

    #[test]
    fn test_mask_set_insert() {
        let mut masks = MaskSet::new();
        let weekend = Arc::new(bitvec![0, 0, 0, 0, 0, 1, 1]);  // Sat, Sun masked
        masks.insert("weekend".to_string(), weekend, 7).unwrap();
        assert!(masks.contains("weekend"));
    }

    #[test]
    fn test_compile_mask_name() {
        let mut masks = MaskSet::new();
        let weekend = Arc::new(bitvec![0, 0, 0, 0, 0, 1, 1]);
        masks.insert("weekend".to_string(), weekend, 7).unwrap();

        let expr = MaskExpr::Name("weekend".to_string());
        let compiled = compile_mask_expr(&expr, &masks, 7).unwrap();
        assert_eq!(compiled, bitvec![0, 0, 0, 0, 0, 1, 1]);
    }

    #[test]
    fn test_compile_mask_not() {
        let mut masks = MaskSet::new();
        let weekend = Arc::new(bitvec![0, 0, 0, 0, 0, 1, 1]);
        masks.insert("weekend".to_string(), weekend, 7).unwrap();

        let expr = MaskExpr::Not(Box::new(MaskExpr::Name("weekend".to_string())));
        let compiled = compile_mask_expr(&expr, &masks, 7).unwrap();
        assert_eq!(compiled, bitvec![1, 1, 1, 1, 1, 0, 0]);  // Inverted
    }

    #[test]
    fn test_compile_mask_or() {
        let mut masks = MaskSet::new();
        let weekend = Arc::new(bitvec![0, 0, 0, 0, 0, 1, 1]);
        let holiday = Arc::new(bitvec![0, 1, 0, 0, 0, 0, 0]);
        masks.insert("weekend".to_string(), weekend, 7).unwrap();
        masks.insert("holiday".to_string(), holiday, 7).unwrap();

        let expr = MaskExpr::Or(vec![
            MaskExpr::Name("weekend".to_string()),
            MaskExpr::Name("holiday".to_string()),
        ]);
        let compiled = compile_mask_expr(&expr, &masks, 7).unwrap();
        assert_eq!(compiled, bitvec![0, 1, 0, 0, 0, 1, 1]);  // weekend OR holiday
    }
}
