#!/bin/bash
# GOLDEN_2: Irregular Timestamps + High NA Density
# Purpose: Stress OBS semantics with calendar gaps and clustered NAs
# Pipeline: dlog → rolling-mean(5) → rolling-std(10)

set -e

BLISP="./target/release/blisp"
COMPAT="--load stdlib/compat_clispi.cl"

echo "Building BLISP..."
cargo build --locked --release --bin blisp > /dev/null 2>&1

echo "Running GOLDEN_2 pipeline..."
# Note: locf not fully integrated yet, starting directly with dlog
# Uses planner operations: dlog, rolling-mean, rolling-std (see OPERATION_NAMING_MAP.md)
# Argument order: (rolling-mean window data), (rolling-std window data)
$BLISP $COMPAT -e \
  '(save "GOLDEN_2_OUTPUT.csv"
     (rolling-std 10
       (rolling-mean 5
         (dlog (file "GOLDEN_2_DATA.csv")))))' \
  > /dev/null 2>&1

echo "Pipeline complete: GOLDEN_2_OUTPUT.csv"
echo ""
echo "Validation:"
LINES=$(wc -l < GOLDEN_2_OUTPUT.csv)
echo "  Output rows: $LINES (expected: 501 including header)"

# Quick stats
python3 << 'EOF'
import csv
with open('GOLDEN_2_OUTPUT.csv') as f:
    reader = csv.reader(f, delimiter=';')
    header = next(reader)
    values = []
    na_count = 0
    for row in reader:
        if len(row) >= 2 and row[1]:
            try:
                values.append(float(row[1]))
            except:
                na_count += 1
        else:
            na_count += 1

    print(f"  Valid values: {len(values)}")
    print(f"  NA values: {na_count}")
    if values:
        print(f"  Value range: [{min(values):.6f}, {max(values):.6f}]")
EOF

echo ""
echo "✅ GOLDEN_2 generated successfully"
