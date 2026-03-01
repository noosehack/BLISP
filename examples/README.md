# BLISP Examples

This directory contains example BLISP scripts demonstrating various features.

## Quickstart Examples

Located in `quickstart/`, these are simple 10-row demonstrations:

### hello.blisp
Basic arithmetic operations:
```bash
blisp run examples/quickstart/hello.blisp
```

### load_csv.blisp
Load and display CSV data:
```bash
blisp run examples/quickstart/load_csv.blisp
```

### rolling_window.blisp
Demonstrates rolling window operations:
```bash
blisp run examples/quickstart/rolling_window.blisp
```

## Golden Test Examples

### gld_num_mini.blisp
100-row subset of the GLD_NUM benchmark (work in progress).

### gld_num_full.blisp
Full 6826-row GLD_NUM golden test (requires external data files).

## Running Examples

```bash
# Basic run
blisp run examples/quickstart/hello.blisp

# With explicit 'run' subcommand
blisp run examples/quickstart/load_csv.blisp

# Backward compatible (no subcommand)
blisp examples/quickstart/hello.blisp
```

## Verifying Outputs

Generate output and verify against expected:
```bash
blisp run examples/quickstart/load_csv.blisp > output.csv
blisp verify output.csv expected/quickstart_load_csv.csv --tol 1e-6
```
