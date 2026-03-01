#!/bin/bash
# Verify GLD_NUM output matches CLISPI reference
# Run this after any changes to ensure correctness

set -e

echo "=== GLD_NUM Verification Script ==="
echo ""

# Run BLISP golden test
echo "Running GLD_NUM_BLISP.sh..."
./GLD_NUM_BLISP.sh

# Check row count
BLISP_ROWS=$(wc -l < GLD_NUM_BLISP.csv)
CLISPI_ROWS=$(wc -l < GLD_NUM_CLISPI.csv)

echo "Row counts:"
echo "  CLISPI: $CLISPI_ROWS"
echo "  BLISP:  $BLISP_ROWS"

if [ "$BLISP_ROWS" != "$CLISPI_ROWS" ]; then
    echo "❌ FAIL: Row count mismatch"
    exit 1
fi
echo "✅ Row counts match"
echo ""

# Numerical comparison
echo "Running numerical comparison..."
python3 << 'PYTHON'
import csv
import sys

with open('GLD_NUM_CLISPI.csv', 'r') as f:
    clispi = list(csv.reader(f, delimiter=';'))

with open('GLD_NUM_BLISP.csv', 'r') as f:
    blisp = list(csv.reader(f, delimiter=';'))

clispi_data = clispi[1:]
blisp_data = blisp[1:]

max_diff = 0.0
max_diff_row = 0
tolerance = 1e-6
failures = []

for i, (c_row, b_row) in enumerate(zip(clispi_data, blisp_data)):
    if c_row[0] != b_row[0]:
        print(f"❌ FAIL: Timestamp mismatch at row {i}")
        sys.exit(1)
    
    c_val = float(c_row[1])
    b_val = float(b_row[1])
    diff = abs(c_val - b_val)
    
    if diff > max_diff:
        max_diff = diff
        max_diff_row = i
    
    if diff > tolerance:
        failures.append((i, c_row[0], c_val, b_val, diff))

print(f"Rows compared: {len(clispi_data)}")
print(f"Maximum difference: {max_diff:.2e} (at row {max_diff_row})")
print(f"Tolerance: {tolerance:.2e}")
print()

if failures:
    print(f"❌ FAIL: {len(failures)} rows exceed tolerance")
    for i, date, c_val, b_val, diff in failures[:5]:
        print(f"  Row {i} ({date}): diff={diff:.2e}")
    sys.exit(1)
else:
    print(f"✅ All values match within tolerance")
PYTHON

echo ""
echo "=== Verification Summary ==="
echo "✅ GLD_NUM output matches CLISPI reference"
echo "✅ Ready for production"
