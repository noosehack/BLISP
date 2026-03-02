// PR2 Blocker: Test proving dlog_column semantic divergence
use blawktrust::Column;

#[test]
fn test_dlog_na_skipping_divergence() {
    // Data with NAs in middle (weekend mask scenario)
    let data = vec![100.0, f64::NAN, f64::NAN, 110.0, 120.0];
    let col = Column::F64(data.clone());
    
    // blawktrust version (position-based lag)
    let blawktrust_result = blawktrust::builtins::ops::dlog_column(&col, 1);
    
    // Expected behavior:
    // Position 0: NA (first element)
    // Position 1: ln(NA/100) = NA
    // Position 2: ln(NA/NA) = NA  
    // Position 3: ln(110/NA) = NA (previous is NA!)
    // Position 4: ln(120/110) = valid
    
    if let Column::F64(result) = blawktrust_result {
        assert!(result[0].is_nan(), "Position 0 should be NA");
        assert!(result[1].is_nan(), "Position 1 should be NA (prev is valid but curr is NA)");
        assert!(result[2].is_nan(), "Position 2 should be NA");
        assert!(result[3].is_nan(), "Position 3 should be NA (prev[2] is NA!)");
        assert!(!result[4].is_nan(), "Position 4 should be valid");
        
        // This proves blawktrust uses POSITIONAL lag (not observation-based)
        println!("blawktrust dlog_column: position-based lag (uses x[i-1] even if NA)");
    }
    
    // Local exec.rs version would give:
    // Position 0: NA (first valid, no previous)
    // Position 1: NA (NA input)
    // Position 2: NA (NA input)
    // Position 3: ln(110/100) = valid (skipped NAs to find last valid!)
    // Position 4: ln(120/110) = valid
    //
    // Positions 3 differs! This proves semantic divergence.
}
