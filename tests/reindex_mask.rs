//! Phase G: reindex_by Mask Propagation Tests
//!
//! Verifies that masks are correctly reindexed when frame indices change.
//! Ensures the semantic hole in mask propagation is closed.

use bitvec::prelude::*;
use blisp::frame::{Frame, IndexColumn, Tags};
use blisp::mask::{ActiveMask, MaskExpr};
use std::sync::Arc;

/// Helper: Create test frame
fn make_frame(dates: Vec<i32>, values: Vec<f64>, colname: &str) -> Frame {
    let index = IndexColumn::Date(Arc::new(dates));
    let tags = Tags::new("DATE".to_string(), index, vec![colname.to_string()]);
    let col = Arc::new(blawktrust::Column::new_f64(values));
    Frame::new(tags, vec![col])
}

/// Helper: Add named mask to frame
fn add_mask(frame: Frame, mask_name: &str, mask_bits: BitVec) -> Frame {
    let nrows = frame.nrows();

    let mut new_masks = frame.tags.masks.clone();
    new_masks
        .insert(mask_name.to_string(), Arc::new(mask_bits), nrows)
        .expect("Failed to insert mask");

    let new_tags = Tags {
        index_name: frame.tags.index_name.clone(),
        index: Arc::clone(&frame.tags.index),
        colnames: Arc::clone(&frame.tags.colnames),
        masks: new_masks,
        active_mask: frame.tags.active_mask.clone(),
    };

    Frame::with_tags(
        Arc::new(new_tags),
        frame
            .cols
            .iter()
            .map(|cd| {
                let blisp::frame::ColData::Mat(col) = cd;
                Arc::clone(col)
            })
            .collect(),
    )
}

/// Helper: Activate mask expression
fn activate_mask(frame: Frame, expr: MaskExpr) -> Frame {
    let nrows = frame.nrows();
    let compiled = blisp::mask::compile_mask_expr(&expr, &frame.tags.masks, nrows)
        .expect("Failed to compile mask expression");

    let new_active_mask = ActiveMask::from_bitvec(compiled, Some(expr));

    let new_tags = Tags {
        index_name: frame.tags.index_name.clone(),
        index: Arc::clone(&frame.tags.index),
        colnames: Arc::clone(&frame.tags.colnames),
        masks: frame.tags.masks.clone(),
        active_mask: new_active_mask,
    };

    Frame::with_tags(
        Arc::new(new_tags),
        frame
            .cols
            .iter()
            .map(|cd| {
                let blisp::frame::ColData::Mat(col) = cd;
                Arc::clone(col)
            })
            .collect(),
    )
}

// ==================== G.1: Preserves Named Masks on Overlap ====================

#[test]
fn reindex_preserves_named_masks_on_overlap() {
    // Source: dates [10, 11, 12, 13, 14] with mask on [11, 13]
    // Target: dates [11, 12, 13, 15] (overlap on 11,12,13; new row 15)

    let source_dates = vec![10, 11, 12, 13, 14];
    let source_values = vec![100.0, 101.0, 102.0, 103.0, 104.0];

    let source_frame = make_frame(source_dates, source_values, "price");

    // Add "custom" mask: positions 1 and 3 (dates 11 and 13)
    let mask_bits = bitvec![0, 1, 0, 1, 0]; // 5 rows
    let source_frame = add_mask(source_frame, "custom", mask_bits);

    // Reindex onto target
    let target_dates = vec![11, 12, 13, 15];
    let target_index = Arc::new(IndexColumn::Date(Arc::new(target_dates)));

    let result = blisp::frame::reindex_by(&source_frame, target_index);

    // Verify mask was reindexed
    assert!(
        result.tags.masks.contains("custom"),
        "Reindexed frame should have 'custom' mask"
    );

    let reindexed_mask = result
        .tags
        .masks
        .get("custom")
        .expect("custom mask should exist");

    // Expected reindexed mask on target [11, 12, 13, 15]:
    // date 11 → was masked in source → true
    // date 12 → was unmasked in source → false
    // date 13 → was masked in source → true
    // date 15 → new row (not in source) → false (default)

    assert_eq!(reindexed_mask.len(), 4, "Reindexed mask should have 4 rows");
    assert!(
        reindexed_mask[0],
        "Date 11 should be masked (was in source)"
    );
    assert!(
        !reindexed_mask[1],
        "Date 12 should be unmasked (was in source)"
    );
    assert!(
        reindexed_mask[2],
        "Date 13 should be masked (was in source)"
    );
    assert!(
        !reindexed_mask[3],
        "Date 15 should be unmasked (new row, default false)"
    );
}

// ==================== G.2: Defaults False for New Rows ====================

#[test]
fn reindex_defaults_false_for_new_rows() {
    // Source: dates [10, 11, 12] with all masked
    // Target: dates [11, 12, 13, 14, 15] (overlap on 11,12; new rows 13,14,15)

    let source_dates = vec![10, 11, 12];
    let source_values = vec![100.0, 101.0, 102.0];

    let source_frame = make_frame(source_dates, source_values, "price");

    // Mask all rows
    let mask_bits = bitvec![1, 1, 1]; // All masked
    let source_frame = add_mask(source_frame, "all_masked", mask_bits);

    // Reindex onto target with new rows
    let target_dates = vec![11, 12, 13, 14, 15];
    let target_index = Arc::new(IndexColumn::Date(Arc::new(target_dates)));

    let result = blisp::frame::reindex_by(&source_frame, target_index);

    let reindexed_mask = result
        .tags
        .masks
        .get("all_masked")
        .expect("mask should exist");

    // Expected:
    // date 11 → was masked → true
    // date 12 → was masked → true
    // date 13, 14, 15 → new rows → false (default)

    assert_eq!(reindexed_mask.len(), 5);
    assert!(reindexed_mask[0], "Date 11 should be masked");
    assert!(reindexed_mask[1], "Date 12 should be masked");
    assert!(
        !reindexed_mask[2],
        "Date 13 should be unmasked (new row, default)"
    );
    assert!(
        !reindexed_mask[3],
        "Date 14 should be unmasked (new row, default)"
    );
    assert!(
        !reindexed_mask[4],
        "Date 15 should be unmasked (new row, default)"
    );

    // Count: 2 masked (overlap), 3 unmasked (new)
    assert_eq!(reindexed_mask.count_ones(), 2);
    assert_eq!(reindexed_mask.count_zeros(), 3);
}

// ==================== G.3: Preserves Active Mask via Expr ====================

#[test]
fn reindex_preserves_active_mask_via_expr() {
    // Source: dates [10, 11, 12, 13, 14]
    // Named mask "odds" on [11, 13] (positions 1, 3)
    // Active mask: (not odds)
    // Target: dates [11, 12, 13, 15]

    let source_dates = vec![10, 11, 12, 13, 14];
    let source_values = vec![100.0, 101.0, 102.0, 103.0, 104.0];

    let source_frame = make_frame(source_dates, source_values, "price");

    // Add "odds" mask
    let odds_bits = bitvec![0, 1, 0, 1, 0]; // dates 11, 13
    let source_frame = add_mask(source_frame, "odds", odds_bits);

    // Activate (not odds)
    let expr = MaskExpr::Not(Box::new(MaskExpr::Name("odds".to_string())));
    let source_frame = activate_mask(source_frame, expr.clone());

    // Verify source active mask
    // (not odds) on [10, 11, 12, 13, 14] = [1, 0, 1, 0, 1]
    assert!(source_frame.tags.active_mask.is_masked(0)); // 10: not odd
    assert!(!source_frame.tags.active_mask.is_masked(1)); // 11: odd
    assert!(source_frame.tags.active_mask.is_masked(2)); // 12: not odd

    // Reindex onto target
    let target_dates = vec![11, 12, 13, 15];
    let target_index = Arc::new(IndexColumn::Date(Arc::new(target_dates)));

    let result = blisp::frame::reindex_by(&source_frame, target_index);

    // Verify expr was preserved
    assert!(
        result.tags.active_mask.expr.is_some(),
        "Active mask expr should be preserved"
    );

    // Verify recompiled active mask on target
    // "odds" reindexed on [11, 12, 13, 15] = [1, 0, 1, 0] (dates 11,13 are odd; 15 is new→false)
    // (not odds) = [0, 1, 0, 1]

    assert!(
        !result.tags.active_mask.is_masked(0),
        "Date 11 is odd → not masked"
    );
    assert!(
        result.tags.active_mask.is_masked(1),
        "Date 12 is not odd → masked"
    );
    assert!(
        !result.tags.active_mask.is_masked(2),
        "Date 13 is odd → not masked"
    );
    assert!(
        result.tags.active_mask.is_masked(3),
        "Date 15 is new (default false) → (not false) = true"
    );
}

// ==================== Regression: Active Mask Without Expr ====================

#[test]
fn reindex_active_mask_without_expr_uses_bitvec() {
    // If active_mask has no expr, reindex the compiled bitvec directly

    let source_dates = vec![10, 11, 12];
    let source_values = vec![100.0, 101.0, 102.0];

    let source_frame = make_frame(source_dates, source_values, "price");

    // Create active mask without expr (manual bitvec)
    let manual_mask = bitvec![0, 1, 0]; // Middle row masked
    let active = ActiveMask::from_bitvec(manual_mask, None); // No expr

    let new_tags = Tags {
        index_name: source_frame.tags.index_name.clone(),
        index: Arc::clone(&source_frame.tags.index),
        colnames: Arc::clone(&source_frame.tags.colnames),
        masks: source_frame.tags.masks.clone(),
        active_mask: active,
    };

    let source_frame = Frame::with_tags(
        Arc::new(new_tags),
        source_frame
            .cols
            .iter()
            .map(|cd| {
                let blisp::frame::ColData::Mat(col) = cd;
                Arc::clone(col)
            })
            .collect(),
    );

    // Reindex onto [11, 12, 13]
    let target_dates = vec![11, 12, 13];
    let target_index = Arc::new(IndexColumn::Date(Arc::new(target_dates)));

    let result = blisp::frame::reindex_by(&source_frame, target_index);

    // Verify: expr is None (no recompilation)
    assert!(
        result.tags.active_mask.expr.is_none(),
        "Should have no expr"
    );

    // Verify reindexed bitvec
    // Original: [10→0, 11→1, 12→0]
    // Target:   [11→1, 12→0, 13→0(new)]
    assert!(
        result.tags.active_mask.is_masked(0),
        "Date 11 should be masked"
    );
    assert!(
        !result.tags.active_mask.is_masked(1),
        "Date 12 should be unmasked"
    );
    assert!(
        !result.tags.active_mask.is_masked(2),
        "Date 13 should be unmasked (new)"
    );
}

// ==================== Edge Case: Empty Overlap ====================

#[test]
fn reindex_empty_overlap_all_new_rows() {
    // Source and target have no overlapping dates
    // All target rows should default to false (unmasked)

    let source_dates = vec![10, 11, 12];
    let source_values = vec![100.0, 101.0, 102.0];

    let source_frame = make_frame(source_dates, source_values, "price");

    // Mask all source rows
    let mask_bits = bitvec![1, 1, 1];
    let source_frame = add_mask(source_frame, "all_masked", mask_bits);

    // Reindex onto completely different dates
    let target_dates = vec![20, 21, 22];
    let target_index = Arc::new(IndexColumn::Date(Arc::new(target_dates)));

    let result = blisp::frame::reindex_by(&source_frame, target_index);

    let reindexed_mask = result
        .tags
        .masks
        .get("all_masked")
        .expect("mask should exist");

    // All target rows are new → all should be false
    assert_eq!(reindexed_mask.len(), 3);
    assert_eq!(
        reindexed_mask.count_ones(),
        0,
        "No overlapping rows, all should be unmasked"
    );
    assert_eq!(reindexed_mask.count_zeros(), 3);
}
