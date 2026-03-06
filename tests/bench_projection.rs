// Quick projection benchmark - run with:
// cargo test --release bench_projection -- --nocapture --ignored
use blisp::ast::Interner;
use blisp::io_fast;
use std::time::Instant;

fn bench_load(label: &str, filename: &str, cols: Option<&[String]>, runs: usize) {
    let mut times = Vec::with_capacity(runs);
    for _ in 0..runs {
        let mut interner = Interner::new();
        let t = Instant::now();
        if let Some(c) = cols {
            let _ = io_fast::load_csv_fast_cols(filename, c, &mut interner);
        } else {
            let _ = io_fast::load_csv_fast(filename, &mut interner);
        }
        times.push(t.elapsed());
    }
    let avg_ms = times.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / runs as f64;
    let min_ms = times
        .iter()
        .map(|d| d.as_secs_f64() * 1000.0)
        .fold(f64::MAX, f64::min);
    println!(
        "{:30} avg={:.1}ms  min={:.1}ms  ({}x)",
        label, avg_ms, min_ms, runs
    );
}

#[test]
#[ignore]
fn bench_projection_at_csv() {
    let file = "/home/ubuntu/At.csv";
    let runs = 5;

    // Full read (all 880 numeric cols)
    bench_load("all cols (880)", file, None, runs);

    // ~50% (440 cols) - first 440
    let headers: Vec<String> = {
        let data = std::fs::read_to_string(file).unwrap();
        let first_line = data.lines().next().unwrap();
        first_line
            .split(';')
            .skip(1)
            .map(|s| s.trim().to_string())
            .collect()
    };

    let cols_440: Vec<String> = headers[..440].to_vec();
    bench_load("50% cols (440)", file, Some(&cols_440), runs);

    // ~10% (88 cols)
    let cols_88: Vec<String> = headers[..88].to_vec();
    bench_load("10% cols (88)", file, Some(&cols_88), runs);

    // 5 cols
    let cols_5: Vec<String> = headers[..5].to_vec();
    bench_load("5 cols", file, Some(&cols_5), runs);

    // 1 col
    let cols_1: Vec<String> = headers[..1].to_vec();
    bench_load("1 col", file, Some(&cols_1), runs);
}
