//! Orientation tripwire tests
//!
//! These tests enforce that orientation changes (o 'H vs o 'Z) produce
//! materially different results for aggregations. They exist to prevent
//! regressions where layout/axis become disconnected.
//!
//! Critical: If these tests fail, the orientation system is broken and
//! users cannot rely on (o 'Z table) to change aggregation direction.

use blawktrust::builtins::ori_ops::sum;
use blawktrust::{Column, Table, TableView, ORI_H, ORI_Z};

#[test]
fn tripwire_orientation_z_affects_sum() {
    // Build a tiny 3×2 table where row-wise vs col-wise aggregation differ
    // Data: A=[1,3,5], B=[2,4,6]
    //
    // Expected for H (column-major, aggregate down columns):
    //   sum → [9, 12] (sum of A, sum of B)
    //
    // Expected for Z (row-major, aggregate across rows):
    //   sum → [3, 7, 11] (sum of each row)

    let table = Table::new(
        vec!["A".to_string(), "B".to_string()],
        vec![
            Column::new_f64(vec![1.0, 3.0, 5.0]),
            Column::new_f64(vec![2.0, 4.0, 6.0]),
        ],
    );

    // Test 1: H orientation (column-major) - should aggregate down columns
    let view_h = TableView::with_ori(table.clone(), ORI_H);
    let result_h = sum(&view_h);

    let data_h = match result_h {
        Column::F64(data) => data,
        _ => panic!("Expected F64 column from sum"),
    };

    assert_eq!(
        data_h.len(),
        2,
        "H orientation should produce 2 column sums"
    );
    assert!(
        (data_h[0] - 9.0).abs() < 1e-9,
        "H orientation: sum of A should be 9.0, got {}",
        data_h[0]
    );
    assert!(
        (data_h[1] - 12.0).abs() < 1e-9,
        "H orientation: sum of B should be 12.0, got {}",
        data_h[1]
    );

    // Test 2: Z orientation (row-major) - should aggregate across rows
    let view_z = TableView::with_ori(table, ORI_Z);
    let result_z = sum(&view_z);

    let data_z = match result_z {
        Column::F64(data) => data,
        _ => panic!("Expected F64 column from sum"),
    };

    assert_eq!(data_z.len(), 3, "Z orientation should produce 3 row sums");
    assert!(
        (data_z[0] - 3.0).abs() < 1e-9,
        "Z orientation: sum of row 0 should be 3.0, got {}",
        data_z[0]
    );
    assert!(
        (data_z[1] - 7.0).abs() < 1e-9,
        "Z orientation: sum of row 1 should be 7.0, got {}",
        data_z[1]
    );
    assert!(
        (data_z[2] - 11.0).abs() < 1e-9,
        "Z orientation: sum of row 2 should be 11.0, got {}",
        data_z[2]
    );

    // Critical assertion: results must be different shapes
    assert_ne!(
        data_h.len(),
        data_z.len(),
        "TRIPWIRE FAILED: H and Z orientations produce identical result shapes. \
         This indicates the orientation system is broken."
    );
}

#[test]
fn tripwire_orientation_z_vs_h_sum() {
    // Similar test for sum aggregation
    // Data: A=[10,20,30], B=[5,15,25]
    //
    // H (column-wise): sum → [60, 45]
    // Z (row-wise): sum → [15, 35, 55]

    let table = Table::new(
        vec!["A".to_string(), "B".to_string()],
        vec![
            Column::new_f64(vec![10.0, 20.0, 30.0]),
            Column::new_f64(vec![5.0, 15.0, 25.0]),
        ],
    );

    // H orientation - column-wise sum
    let view_h = TableView::with_ori(table.clone(), ORI_H);
    let result_h = sum(&view_h);

    let data_h = match result_h {
        Column::F64(data) => data,
        _ => panic!("Expected F64 column from sum"),
    };

    assert_eq!(data_h.len(), 2, "H: should have 2 column sums");
    assert!((data_h[0] - 60.0).abs() < 1e-9, "H: sum(A) = 60");
    assert!((data_h[1] - 45.0).abs() < 1e-9, "H: sum(B) = 45");

    // Z orientation - row-wise sum
    let view_z = TableView::with_ori(table, ORI_Z);
    let result_z = sum(&view_z);

    let data_z = match result_z {
        Column::F64(data) => data,
        _ => panic!("Expected F64 column from sum"),
    };

    assert_eq!(data_z.len(), 3, "Z: should have 3 row sums");
    assert!((data_z[0] - 15.0).abs() < 1e-9, "Z: sum(row 0) = 15");
    assert!((data_z[1] - 35.0).abs() < 1e-9, "Z: sum(row 1) = 35");
    assert!((data_z[2] - 55.0).abs() < 1e-9, "Z: sum(row 2) = 55");

    // Critical: different shapes
    assert_ne!(
        data_h.len(),
        data_z.len(),
        "TRIPWIRE: H and Z must produce different result shapes"
    );
}

#[test]
fn tripwire_all_rowwise_vs_colwise_orientations() {
    // Verify all row-major orientations (Z, S, _Z, _S) behave consistently
    // All should produce 3 row-wise aggregates, not 2 column-wise

    use blawktrust::{ORI_N, ORI_S, ORI__H, ORI__N, ORI__S, ORI__Z};

    let table = Table::new(
        vec!["A".to_string(), "B".to_string()],
        vec![
            Column::new_f64(vec![1.0, 2.0, 3.0]),
            Column::new_f64(vec![4.0, 5.0, 6.0]),
        ],
    );

    // Test all row-major orientations
    let rowwise_oris = [("Z", ORI_Z), ("S", ORI_S), ("_Z", ORI__Z), ("_S", ORI__S)];

    for (name, ori) in rowwise_oris {
        let oriented = TableView::with_ori(table.clone(), ori);
        let result = sum(&oriented);

        let data = match result {
            Column::F64(data) => data,
            _ => panic!("Expected F64 column from sum"),
        };

        assert_eq!(
            data.len(),
            3,
            "Orientation {} should produce 3 row sums, got {}",
            name,
            data.len()
        );
    }

    // Test all column-major orientations produce 2 column sums
    let colwise_oris = [("H", ORI_H), ("N", ORI_N), ("_H", ORI__H), ("_N", ORI__N)];

    for (name, ori) in colwise_oris {
        let oriented = TableView::with_ori(table.clone(), ori);
        let result = sum(&oriented);

        let data = match result {
            Column::F64(data) => data,
            _ => panic!("Expected F64 column from sum"),
        };

        assert_eq!(
            data.len(),
            2,
            "Orientation {} should produce 2 column sums, got {}",
            name,
            data.len()
        );
    }
}
