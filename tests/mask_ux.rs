//! Phase H: Mask UX Builtins Tests
//!
//! Verifies user-facing mask utilities for debugging and composition.

use bitvec::prelude::*;
use blisp::builtins::register_builtins;
use blisp::frame::{Frame, IndexColumn, Tags};
use blisp::mask::{ActiveMask, MaskExpr, MaskSet};
use blisp::runtime::Runtime;
use blisp::value::Value;
use std::sync::Arc;

/// Helper: Create test frame with weekend mask
fn make_frame_with_weekend_mask() -> (Frame, Runtime) {
    let dates: Vec<i32> = (0..14).collect(); // 2 weeks
    let values: Vec<f64> = (0..14).map(|i| 100.0 + i as f64).collect();

    let index = IndexColumn::Date(Arc::new(dates));
    let tags = Tags::new("DATE".to_string(), index, vec!["price".to_string()]);
    let col = Arc::new(blawktrust::Column::new_f64(values));
    let frame = Frame::new(tags, vec![col]);

    // Add weekend mask
    let weekend_bitvec: BitVec = (0..14)
        .map(|date| {
            let day_of_week = (4 + date) % 7;
            day_of_week == 0 || day_of_week == 6
        })
        .collect();

    let mut new_masks = frame.tags.masks.clone();
    new_masks
        .insert("weekend".to_string(), Arc::new(weekend_bitvec), 14)
        .unwrap();

    let new_tags = Tags {
        index_name: frame.tags.index_name.clone(),
        index: Arc::clone(&frame.tags.index),
        colnames: Arc::clone(&frame.tags.colnames),
        masks: new_masks,
        active_mask: frame.tags.active_mask.clone(),
    };

    let frame = Frame::with_tags(
        Arc::new(new_tags),
        frame
            .cols
            .iter()
            .filter_map(|cd| {
                if let blisp::frame::ColData::Mat(col) = cd {
                    Some(Arc::clone(col))
                } else {
                    None
                }
            })
            .collect(),
    );

    let mut rt = Runtime::new();
    register_builtins(&mut rt);

    (frame, rt)
}

// ==================== H.1: mask-list ====================

#[test]
fn test_mask_list_contains_weekend() {
    let (frame, mut rt) = make_frame_with_weekend_mask();

    // Call (mask-list frame)
    let frame_val = Value::Frame(Arc::new(frame));
    let result = blisp::builtins::builtin_mask_list(&mut rt, &[frame_val])
        .expect("mask-list should succeed");

    // Should return a list of mask info
    match result {
        Value::List(masks) => {
            assert_eq!(masks.len(), 1, "Should have 1 mask (weekend)");

            // First mask should be weekend
            match &masks[0] {
                Value::List(info) => {
                    assert_eq!(info.len(), 4, "Mask info should have 4 elements");

                    // Check name
                    match &info[0] {
                        Value::Str(name) => assert_eq!(&**name, "weekend"),
                        _ => panic!("Expected string name"),
                    }

                    // Check masked_count (4 weekend days in 14 days)
                    match &info[1] {
                        Value::Int(count) => assert_eq!(*count, 4),
                        _ => panic!("Expected int masked_count"),
                    }

                    // Check total_count
                    match &info[2] {
                        Value::Int(total) => assert_eq!(*total, 14),
                        _ => panic!("Expected int total_count"),
                    }

                    // Check pct_masked (4/14 ≈ 28.57%)
                    match &info[3] {
                        Value::Float(pct) => {
                            assert!(
                                (*pct - 28.571).abs() < 0.01,
                                "Expected ~28.57%, got {}",
                                pct
                            );
                        }
                        _ => panic!("Expected float pct_masked"),
                    }
                }
                _ => panic!("Expected list for mask info"),
            }
        }
        _ => panic!("Expected list from mask-list"),
    }
}

// ==================== H.2: mask-stats ====================

#[test]
fn test_mask_stats_counts_match_known_calendar() {
    let (frame, mut rt) = make_frame_with_weekend_mask();

    // Test stats for 'weekend expression
    let expr_val = Value::Sym(rt.interner.intern("weekend"));
    let frame_val = Value::Frame(Arc::new(frame));

    let result = blisp::builtins::builtin_mask_stats(&mut rt, &[frame_val, expr_val])
        .expect("mask-stats should succeed");

    match result {
        Value::List(stats) => {
            assert_eq!(stats.len(), 3, "Stats should have 3 elements");

            // masked_count = 4
            match &stats[0] {
                Value::Int(count) => assert_eq!(*count, 4, "4 weekend days"),
                _ => panic!("Expected int masked_count"),
            }

            // unmasked_count = 10
            match &stats[1] {
                Value::Int(count) => assert_eq!(*count, 10, "10 weekdays"),
                _ => panic!("Expected int unmasked_count"),
            }

            // pct_masked ≈ 28.57%
            match &stats[2] {
                Value::Float(pct) => {
                    assert!((*pct - 28.571).abs() < 0.01);
                }
                _ => panic!("Expected float pct_masked"),
            }
        }
        _ => panic!("Expected list from mask-stats"),
    }
}

// ==================== H.3: mask-define ====================

#[test]
fn test_mask_define_or_equals_weekend() {
    let (frame, mut rt) = make_frame_with_weekend_mask();

    // Define (or weekend weekend) as "same_as_weekend"
    let frame_val = Value::Frame(Arc::new(frame));
    let name_val = Value::Str("same_as_weekend".into());

    // Build (or weekend weekend) expression
    let weekend_sym = Value::Sym(rt.interner.intern("weekend"));
    let or_expr = Value::List(vec![
        Value::Sym(rt.interner.intern("or")),
        weekend_sym.clone(),
        weekend_sym,
    ]);

    let result_frame =
        blisp::builtins::builtin_mask_define(&mut rt, &[frame_val, name_val, or_expr])
            .expect("mask-define should succeed");

    // Verify new mask was added
    match result_frame {
        Value::Frame(frame_arc) => {
            assert!(frame_arc.tags.masks.contains("same_as_weekend"));
            assert!(frame_arc.tags.masks.contains("weekend"));

            // Both masks should have same bits
            let weekend_mask = frame_arc.tags.masks.get("weekend").unwrap();
            let same_mask = frame_arc.tags.masks.get("same_as_weekend").unwrap();

            assert_eq!(weekend_mask.count_ones(), same_mask.count_ones());
            assert_eq!(
                **weekend_mask, **same_mask,
                "OR of same mask should equal original"
            );
        }
        _ => panic!("Expected frame from mask-define"),
    }
}

#[test]
fn test_mask_define_collision_errors_deterministically() {
    let (frame, mut rt) = make_frame_with_weekend_mask();

    // First, define a new mask "collision_test"
    let frame_val = Value::Frame(Arc::new(frame.clone()));
    let name_val = Value::Str("collision_test".into());
    let expr1 = Value::Sym(rt.interner.intern("weekend"));

    let result1 = blisp::builtins::builtin_mask_define(
        &mut rt,
        &[frame_val.clone(), name_val.clone(), expr1],
    )
    .expect("First define should succeed");

    // Try to redefine with same expression → should succeed (same bits)
    let expr2 = Value::Sym(rt.interner.intern("weekend"));
    let frame1 = match result1 {
        Value::Frame(f) => f,
        _ => panic!("Expected frame"),
    };

    let frame1_val = Value::Frame(frame1.clone());
    let result2 =
        blisp::builtins::builtin_mask_define(&mut rt, &[frame1_val, name_val.clone(), expr2]);
    assert!(result2.is_ok(), "Redefining with same bits should succeed");

    // Try to redefine with DIFFERENT expression → should error
    // Use (not weekend) which has different bits
    let not_expr = Value::List(vec![
        Value::Sym(rt.interner.intern("not")),
        Value::Sym(rt.interner.intern("weekend")),
    ]);

    let frame1_val2 = Value::Frame(frame1);
    let result3 = blisp::builtins::builtin_mask_define(&mut rt, &[frame1_val2, name_val, not_expr]);

    assert!(
        result3.is_err(),
        "Redefining with different bits should error"
    );

    let err_msg = result3.unwrap_err();
    assert!(
        err_msg.contains("collision") || err_msg.contains("different"),
        "Error should mention collision: {}",
        err_msg
    );
}

// ==================== H.4: mask-off ====================

#[test]
fn test_mask_off_clears_active_mask() {
    let (frame, mut rt) = make_frame_with_weekend_mask();

    // Activate weekend mask
    let expr = MaskExpr::Name("weekend".to_string());
    let compiled = blisp::mask::compile_mask_expr(&expr, &frame.tags.masks, 14).unwrap();
    let active = ActiveMask::from_bitvec(compiled, Some(expr));

    let new_tags = Tags {
        index_name: frame.tags.index_name.clone(),
        index: Arc::clone(&frame.tags.index),
        colnames: Arc::clone(&frame.tags.colnames),
        masks: frame.tags.masks.clone(),
        active_mask: active,
    };

    let frame_with_active = Frame::with_tags(
        Arc::new(new_tags),
        frame
            .cols
            .iter()
            .filter_map(|cd| {
                if let blisp::frame::ColData::Mat(col) = cd {
                    Some(Arc::clone(col))
                } else {
                    None
                }
            })
            .collect(),
    );

    // Verify active mask has 4 masked rows
    assert_eq!(frame_with_active.tags.active_mask.count_masked(), 4);

    // Call mask-off
    let frame_val = Value::Frame(Arc::new(frame_with_active));
    let result =
        blisp::builtins::builtin_mask_off(&mut rt, &[frame_val]).expect("mask-off should succeed");

    // Verify active mask is now empty
    match result {
        Value::Frame(frame_arc) => {
            assert_eq!(
                frame_arc.tags.active_mask.count_masked(),
                0,
                "Active mask should be cleared"
            );
            assert_eq!(
                frame_arc.tags.active_mask.count_unmasked(),
                14,
                "All rows should be unmasked"
            );

            // Named masks should still exist
            assert!(
                frame_arc.tags.masks.contains("weekend"),
                "Named masks should be preserved"
            );
        }
        _ => panic!("Expected frame from mask-off"),
    }
}
