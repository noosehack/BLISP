//! Integration test for blawktrust API surface
//!
//! This test exists to catch breaking changes in blawktrust's public API
//! that BLISP depends on. If blawktrust removes or changes types/functions
//! that BLISP uses, this test will fail at compile time or runtime.
//!
//! **Critical**: This test prevented a production incident on 2026-02-28
//! where blawktrust removed Column::Date and Column::Timestamp, breaking
//! BLISP with 48 compile errors. This test would have caught it in CI.

use blawktrust::{Column, NULL_DATE, NULL_TIMESTAMP, NULL_TS};

/// Test that all required Column types exist and can be constructed
#[test]
fn test_column_types_exist() {
    // F64 column (always required)
    let f64_col = Column::new_f64(vec![1.0, 2.0, 3.0]);
    assert_eq!(f64_col.len(), 3);

    // Date column (required for CSV I/O)
    let date_col = Column::new_date(vec![18628, 18629, NULL_DATE]);
    assert_eq!(date_col.len(), 3);

    // Timestamp column (required for CSV I/O)
    let ts_col = Column::new_timestamp(vec![0, 1_000_000_000, NULL_TIMESTAMP]);
    assert_eq!(ts_col.len(), 3);

    // Ts column (may be deprecated but must exist during migration)
    let ts_legacy = Column::new_ts(vec![100, NULL_TS, 300]);
    assert_eq!(ts_legacy.len(), 3);
}

/// Test that NULL sentinels are exported and have correct values
#[test]
fn test_null_sentinels_exist() {
    // These must be public exports
    let _date_null: i32 = NULL_DATE;
    let _timestamp_null: i64 = NULL_TIMESTAMP;
    let _ts_null: i64 = NULL_TS;

    // Verify they are actually null sentinels (extreme values)
    assert_eq!(NULL_DATE, i32::MIN);
    assert_eq!(NULL_TIMESTAMP, i64::MIN);
    assert_eq!(NULL_TS, i64::MIN);
}

/// Test that Column variants can be pattern matched
#[test]
fn test_column_pattern_matching() {
    let f64_col = Column::new_f64(vec![1.0, 2.0]);
    let date_col = Column::new_date(vec![18628, 18629]);
    let ts_col = Column::new_timestamp(vec![0, 1_000_000_000]);
    let ts_legacy = Column::new_ts(vec![100, 200]);

    // Pattern matching is critical for BLISP's CSV I/O
    match &f64_col {
        Column::F64(data) => assert_eq!(data.len(), 2),
        _ => panic!("F64 column pattern match failed"),
    }

    match &date_col {
        Column::Date(data) => assert_eq!(data.len(), 2),
        _ => panic!("Date column pattern match failed"),
    }

    match &ts_col {
        Column::Timestamp(data) => assert_eq!(data.len(), 2),
        _ => panic!("Timestamp column pattern match failed"),
    }

    match &ts_legacy {
        Column::Ts(data) => assert_eq!(data.len(), 2),
        _ => panic!("Ts column pattern match failed"),
    }
}

/// Test TableView and orientation system
#[test]
fn test_tableview_and_orientations() {
    use blawktrust::{Table, TableView, ORI_H, ORI_Z, ORI_R};

    // Create a simple table
    let table = Table::new(
        vec!["a".to_string(), "b".to_string()],
        vec![
            Column::new_f64(vec![1.0, 3.0]),
            Column::new_f64(vec![2.0, 4.0]),
        ],
    );

    // Test that TableView can be created with different orientations
    let view_h = TableView::with_ori(table.clone(), ORI_H);
    let view_z = TableView::with_ori(table.clone(), ORI_Z);
    let view_r = TableView::with_ori(table.clone(), ORI_R);

    // Verify orientations are set correctly
    assert_eq!(view_h.ori, ORI_H);
    assert_eq!(view_z.ori, ORI_Z);
    assert_eq!(view_r.ori, ORI_R);

    // Verify table shape methods exist
    let (nr, nc) = view_h.logical_shape();
    assert_eq!(nr, 2); // 2 rows
    assert_eq!(nc, 2); // 2 columns
}

/// Test that orientation operations exist (sum is critical)
#[test]
fn test_orientation_operations() {
    use blawktrust::{Table, TableView, ORI_H, ORI_Z, ORI_R};
    use blawktrust::builtins::ori_ops::{sum, dlog};

    let table = Table::new(
        vec!["a".to_string(), "b".to_string()],
        vec![
            Column::new_f64(vec![1.0, 3.0]),
            Column::new_f64(vec![2.0, 4.0]),
        ],
    );

    // Test H orientation (column sums)
    let view_h = TableView::with_ori(table.clone(), ORI_H);
    let result_h = sum(&view_h);
    match result_h {
        Column::F64(data) => {
            assert_eq!(data.len(), 2); // Should have 2 sums (one per column)
            assert_eq!(data[0], 4.0);  // 1 + 3
            assert_eq!(data[1], 6.0);  // 2 + 4
        }
        _ => panic!("Expected F64 column from sum"),
    }

    // Test Z orientation (row sums)
    let view_z = TableView::with_ori(table.clone(), ORI_Z);
    let result_z = sum(&view_z);
    match result_z {
        Column::F64(data) => {
            assert_eq!(data.len(), 2); // Should have 2 sums (one per row)
            assert_eq!(data[0], 3.0);  // 1 + 2
            assert_eq!(data[1], 7.0);  // 3 + 4
        }
        _ => panic!("Expected F64 column from sum"),
    }

    // Test R orientation (scalar sum)
    let view_r = TableView::with_ori(table.clone(), ORI_R);
    let result_r = sum(&view_r);
    match result_r {
        Column::F64(data) => {
            assert_eq!(data.len(), 1);   // Should be single value
            assert_eq!(data[0], 10.0);   // 1 + 2 + 3 + 4
        }
        _ => panic!("Expected F64 column from sum"),
    }

    // Test dlog exists and works
    let view_h = TableView::with_ori(table, ORI_H);
    let _dlog_result = dlog(&view_h);
    // Just verify it doesn't panic
}

/// Test that Table has Clone (required for TableView construction)
#[test]
fn test_table_clone() {
    use blawktrust::Table;

    let table = Table::new(
        vec!["a".to_string()],
        vec![Column::new_f64(vec![1.0, 2.0])],
    );

    // Must be able to clone
    let table2 = table.clone();
    assert_eq!(table.row_count(), table2.row_count());
    assert_eq!(table.col_count(), table2.col_count());
}

/// Test CSV I/O path that uses Date/Timestamp columns
#[test]
fn test_csv_io_column_usage() {
    // This simulates what BLISP's io.rs does when parsing CSV files
    // If Date/Timestamp types are removed, this will fail to compile

    let date_col = Column::new_date(vec![18628, NULL_DATE, 18630]);
    let ts_col = Column::new_timestamp(vec![0, NULL_TIMESTAMP, 1_000_000_000]);
    let f64_col = Column::new_f64(vec![100.0, f64::NAN, 102.0]);

    // Simulate the pattern matching that io.rs does
    let columns = vec![date_col, ts_col, f64_col];

    for col in &columns {
        match col {
            Column::Date(data) => {
                for &val in data {
                    if val == NULL_DATE {
                        // Would write "NA" in CSV
                    } else {
                        // Would format date
                    }
                }
            }
            Column::Timestamp(data) => {
                for &val in data {
                    if val == NULL_TIMESTAMP {
                        // Would write "NA" in CSV
                    } else {
                        // Would format timestamp
                    }
                }
            }
            Column::F64(data) => {
                for &val in data {
                    if val.is_nan() {
                        // Would write "NA" in CSV
                    } else {
                        // Would format number
                    }
                }
            }
            Column::Ts(data) => {
                for &val in data {
                    if val == NULL_TS {
                        // Would write "NA" in CSV
                    } else {
                        // Would format timestamp
                    }
                }
            }
        }
    }
}

/// Test that has_nulls() method exists on Column
#[test]
fn test_column_has_nulls() {
    let f64_clean = Column::new_f64(vec![1.0, 2.0, 3.0]);
    let f64_nulls = Column::new_f64(vec![1.0, f64::NAN, 3.0]);

    assert!(!f64_clean.has_nulls());
    assert!(f64_nulls.has_nulls());

    let date_clean = Column::new_date(vec![18628, 18629]);
    let date_nulls = Column::new_date(vec![18628, NULL_DATE]);

    assert!(!date_clean.has_nulls());
    assert!(date_nulls.has_nulls());

    let ts_clean = Column::new_timestamp(vec![0, 1_000_000_000]);
    let ts_nulls = Column::new_timestamp(vec![0, NULL_TIMESTAMP]);

    assert!(!ts_clean.has_nulls());
    assert!(ts_nulls.has_nulls());
}
