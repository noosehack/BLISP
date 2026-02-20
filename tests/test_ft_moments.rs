//! Tests for Ft-measurable rolling moments

use blawktrust::builtins::{rolling_moments_past_only_f64, MomentsMask};

#[test]
fn test_past_only_window_off_by_one() {
    // Critical test: verify window is [i-window, i-1], not [i-window+1, i]
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let window = 3;

    let mask = MomentsMask::from_names(&["mean"]);
    let output = rolling_moments_past_only_f64(&data, window, None, mask, None);

    let means = output.mean.unwrap();

    // Position 0, 1, 2: window too small
    assert!(means[0].is_nan());
    assert!(means[1].is_nan());
    assert!(means[2].is_nan());

    // Position 3: window [0,2] = [1.0, 2.0, 3.0], mean = 2.0
    assert!(
        (means[3] - 2.0).abs() < 1e-10,
        "Position 3: expected 2.0, got {}",
        means[3]
    );

    // Position 4: window [1,3] = [2.0, 3.0, 4.0], mean = 3.0
    assert!(
        (means[4] - 3.0).abs() < 1e-10,
        "Position 4: expected 3.0, got {}",
        means[4]
    );

    // Position 5: window [2,4] = [3.0, 4.0, 5.0], mean = 4.0
    assert!(
        (means[5] - 4.0).abs() < 1e-10,
        "Position 5: expected 4.0, got {}",
        means[5]
    );
}

#[test]
fn test_na_handling_in_past_window() {
    let data = vec![1.0, f64::NAN, 3.0, 4.0, f64::NAN, 6.0];
    let window = 3;

    let mask = MomentsMask::from_names(&["mean", "count"]);
    let output = rolling_moments_past_only_f64(&data, window, Some(2), mask, None);

    let means = output.mean.unwrap();
    let counts = output.count.unwrap();

    // Position 3: window [0,2] = [1.0, NaN, 3.0], count=2, mean=2.0
    assert!((counts[3] - 2.0).abs() < 1e-10);
    assert!((means[3] - 2.0).abs() < 1e-10);

    // Position 4: window [1,3] = [NaN, 3.0, 4.0], count=2, mean=3.5
    assert!((counts[4] - 2.0).abs() < 1e-10);
    assert!((means[4] - 3.5).abs() < 1e-10);

    // Position 5: window [2,4] = [3.0, 4.0, NaN], count=2, mean=3.5
    assert!((counts[5] - 2.0).abs() < 1e-10);
    assert!((means[5] - 3.5).abs() < 1e-10);
}

#[test]
fn test_std_sample_variance_ddof1() {
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let window = 3;

    let mask = MomentsMask::from_names(&["mean", "std"]);
    let output = rolling_moments_past_only_f64(&data, window, None, mask, None);

    let stds = output.std.unwrap();

    // Position 3: window [1,2,3], sample std = 1.0
    assert!(
        (stds[3] - 1.0).abs() < 1e-10,
        "Position 3: expected std=1.0, got {}",
        stds[3]
    );

    // Position 4: window [2,3,4], sample std = 1.0
    assert!(
        (stds[4] - 1.0).abs() < 1e-10,
        "Position 4: expected std=1.0, got {}",
        stds[4]
    );
}

#[test]
fn test_skew_right_skewed_data() {
    // Right-skewed: most values low, one high outlier
    let data = vec![0.0, 1.0, 1.0, 1.0, 10.0, 1.0, 1.0];
    let window = 4;

    let mask = MomentsMask::from_names(&["skew"]);
    let output = rolling_moments_past_only_f64(&data, window, None, mask, None);

    let skews = output.skew.unwrap();

    // Position 4: window [0,1,2,3] = [0,1,1,1] is left-skewed (negative)
    assert!(skews[4] < 0.0, "Expected negative skew, got {}", skews[4]);

    // Position 5: window [1,2,3,4] = [1,1,1,10] is right-skewed (positive)
    assert!(skews[5] > 0.5, "Expected positive skew, got {}", skews[5]);
}

#[test]
fn test_kurt_high_kurtosis() {
    // High kurtosis: outliers present
    let data = vec![0.0, 1.0, 1.0, 1.0, 10.0, 1.0, 1.0];
    let window = 5;

    let mask = MomentsMask::from_names(&["kurt"]);
    let output = rolling_moments_past_only_f64(&data, window, None, mask, None);

    let kurts = output.kurt.unwrap();

    // Position 5: window [0,1,2,3,4] = [0,1,1,1,10] has high kurtosis
    assert!(
        kurts[5] > 0.0,
        "Expected positive excess kurtosis, got {}",
        kurts[5]
    );
}

#[test]
fn test_min_periods() {
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let window = 4;
    let min_periods = 2;

    let mask = MomentsMask::from_names(&["mean"]);
    let output = rolling_moments_past_only_f64(&data, window, Some(min_periods), mask, None);

    let means = output.mean.unwrap();

    // Position 4: window [0,3] has 4 values >= min_periods, valid
    assert!(!means[4].is_nan());
}

#[test]
fn test_all_moments_together() {
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
    let window = 4;

    let mask = MomentsMask::all();
    let output = rolling_moments_past_only_f64(&data, window, None, mask, None);

    // Verify all outputs exist
    assert!(output.mean.is_some());
    assert!(output.std.is_some());
    assert!(output.skew.is_some());
    assert!(output.kurt.is_some());
    assert!(output.count.is_some());

    // Position 4: all moments should be valid
    let means = output.mean.as_ref().unwrap();
    let stds = output.std.as_ref().unwrap();
    let skews = output.skew.as_ref().unwrap();
    let kurts = output.kurt.as_ref().unwrap();
    let counts = output.count.as_ref().unwrap();

    assert!(!means[4].is_nan());
    assert!(!stds[4].is_nan());
    assert!(!skews[4].is_nan());
    assert!(!kurts[4].is_nan());
    assert!((counts[4] - 4.0).abs() < 1e-10);
}

#[test]
fn test_comparison_with_python_reference() {
    // Python reference implementation (computed offline):
    // data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    // window = 5
    // For position i, use data[i-5:i] (past only)
    //
    // Position 5: window [0:5] = [1,2,3,4,5]
    //   mean = 3.0, std = 1.5811 (ddof=1), skew = 0.0, kurt = -1.3

    let data: Vec<f64> = (1..=10).map(|x| x as f64).collect();
    let window = 5;

    let mask = MomentsMask::all();
    let output = rolling_moments_past_only_f64(&data, window, None, mask, None);

    let means = output.mean.unwrap();
    let stds = output.std.unwrap();
    let skews = output.skew.unwrap();
    let kurts = output.kurt.unwrap();

    // Position 5: window [0,4] = [1,2,3,4,5]
    assert!((means[5] - 3.0).abs() < 1e-6, "mean mismatch: {}", means[5]);
    assert!(
        (stds[5] - 1.5811388300841898).abs() < 1e-6,
        "std mismatch: {}",
        stds[5]
    );
    assert!((skews[5] - 0.0).abs() < 1e-6, "skew mismatch: {}", skews[5]);
    assert!(
        (kurts[5] - (-1.3)).abs() < 1e-6,
        "kurt mismatch: {}",
        kurts[5]
    );

    // Position 6: window [1,5] = [2,3,4,5,6]
    assert!((means[6] - 4.0).abs() < 1e-6);
    assert!((stds[6] - 1.5811388300841898).abs() < 1e-6);
    assert!((skews[6] - 0.0).abs() < 1e-6);
    assert!((kurts[6] - (-1.3)).abs() < 1e-6);
}

#[test]
fn test_tiny_variance_clamping() {
    // All values identical -> variance = 0
    let data = vec![5.0, 5.0, 5.0, 5.0, 5.0];
    let window = 3;

    let mask = MomentsMask::from_names(&["std", "skew", "kurt"]);
    let output = rolling_moments_past_only_f64(&data, window, None, mask, None);

    let stds = output.std.unwrap();
    let skews = output.skew.unwrap();
    let kurts = output.kurt.unwrap();

    // Position 3: std should be 0.0, skew/kurt should be NaN (zero variance)
    assert!((stds[3] - 0.0).abs() < 1e-10);
    assert!(skews[3].is_nan() || skews[3].abs() < 1e-6);
    assert!(kurts[3].is_nan() || kurts[3].abs() < 1e-6);
}

#[test]
fn test_insufficient_data_for_higher_moments() {
    let data = vec![1.0, 2.0, 3.0, 4.0];
    let window = 2; // Only 2 values in window

    let mask = MomentsMask::all();
    let output = rolling_moments_past_only_f64(&data, window, None, mask, None);

    let stds = output.std.unwrap();
    let skews = output.skew.unwrap();
    let kurts = output.kurt.unwrap();

    // Position 2: window [0,1] = [1,2], count=2
    // std is valid (n>=2), but skew needs n>=3, kurt needs n>=4
    assert!(!stds[2].is_nan(), "std should be valid with n=2");
    assert!(skews[2].is_nan(), "skew should be NaN with n=2");
    assert!(kurts[2].is_nan(), "kurt should be NaN with n=2");
}
