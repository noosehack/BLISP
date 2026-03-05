//! Tripwire tests for fixture file integrity.
//!
//! Ensures all CSV fixtures in tests/fixtures/ use the project-standard format:
//! - Semicolon delimiter (;)
//! - At least one header column
//! - At least one data row
//! - No comma-only delimited files

use std::fs;
use std::path::Path;

fn fixture_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Every .csv file in tests/fixtures/ must use semicolon delimiter.
#[test]
fn tripwire_all_fixtures_use_semicolon_delimiter() {
    let dir = fixture_dir();
    assert!(dir.exists(), "tests/fixtures/ directory must exist");

    let mut checked = 0;
    for entry in fs::read_dir(&dir).expect("cannot read fixtures dir") {
        let entry = entry.expect("cannot read dir entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("csv") {
            continue;
        }

        let content = fs::read_to_string(&path).expect("cannot read fixture");
        let header = content.lines().next().expect("fixture is empty");

        // Header must contain at least one semicolon
        assert!(
            header.contains(';'),
            "Fixture {} header has no semicolons: {:?}\n\
             All fixtures must use ';' as delimiter (project standard).",
            path.display(),
            header
        );

        // Header must not be comma-only (no semicolons but has commas)
        if header.contains(',') {
            assert!(
                header.contains(';'),
                "Fixture {} uses comma delimiter instead of semicolon: {:?}",
                path.display(),
                header
            );
        }

        checked += 1;
    }

    assert!(
        checked >= 1,
        "No CSV fixtures found in tests/fixtures/. Add at least one."
    );
}

/// Every fixture must have at least one data row that parses.
#[test]
fn tripwire_all_fixtures_have_data_rows() {
    let dir = fixture_dir();

    for entry in fs::read_dir(&dir).expect("cannot read fixtures dir") {
        let entry = entry.expect("cannot read dir entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("csv") {
            continue;
        }

        let content = fs::read_to_string(&path).expect("cannot read fixture");
        let line_count = content.lines().count();

        // Must have header + at least 1 data row
        assert!(
            line_count >= 2,
            "Fixture {} has only {} line(s). Need header + at least 1 data row.",
            path.display(),
            line_count
        );
    }
}

/// Every fixture header must have at least one column name.
#[test]
fn tripwire_all_fixtures_have_columns() {
    let dir = fixture_dir();

    for entry in fs::read_dir(&dir).expect("cannot read fixtures dir") {
        let entry = entry.expect("cannot read dir entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("csv") {
            continue;
        }

        let content = fs::read_to_string(&path).expect("cannot read fixture");
        let header = content.lines().next().expect("fixture is empty");
        let col_count = header.split(';').count();

        assert!(
            col_count >= 2,
            "Fixture {} has only {} column(s). Need index + at least 1 data column.",
            path.display(),
            col_count
        );
    }
}

/// Fixture filenames must be lowercase with underscores only.
#[test]
fn tripwire_fixture_naming_convention() {
    let dir = fixture_dir();

    for entry in fs::read_dir(&dir).expect("cannot read fixtures dir") {
        let entry = entry.expect("cannot read dir entry");
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".csv") {
            continue;
        }

        let stem = name.trim_end_matches(".csv");
        assert!(
            stem.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "Fixture filename '{}' must be lowercase + underscores only.",
            name
        );
    }
}
