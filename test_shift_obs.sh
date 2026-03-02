#!/bin/bash
# Test shift-obs (mask-aware shift) vs shift (calendar shift)

# Create test data: Mon-Fri week with simple values
cat > /tmp/test_shift_obs.csv << 'DATA'
DATE;price
2024-01-01;100
2024-01-02;101
2024-01-03;102
2024-01-04;103
2024-01-05;104
2024-01-06;105
2024-01-07;106
2024-01-08;107
2024-01-09;108
2024-01-10;109
DATA

echo "=== Test Data (10 days including 2 weekends) ==="
cat /tmp/test_shift_obs.csv
echo ""

echo "=== Test 1: Calendar shift (no mask) - shift 2 ==="
echo "(shift 2 (read-csv \"/tmp/test_shift_obs.csv\"))" | cargo run --release --quiet 2>/dev/null
echo ""

echo "=== Test 2: Calendar shift (shift 2) with weekend mask active ==="
echo "(-> (read-csv \"/tmp/test_shift_obs.csv\") (mask-weekend) (with-mask \"weekend\") (shift 2))" | cargo run --release --quiet 2>/dev/null
echo ""

echo "=== Test 3: Observation shift (shift-obs 2) with weekend mask active ==="
echo "(-> (read-csv \"/tmp/test_shift_obs.csv\") (mask-weekend) (with-mask \"weekend\") (shift-obs 2))" | cargo run --release --quiet 2>/dev/null
echo ""

echo "=== Test 4: Comparison - Tuesday after weekend ==="
echo "Calendar shift: Tuesday lands on Sunday (masked) → NA"
echo "Observation shift: Tuesday lands on Friday (2 business days back) → preserves value"
echo ""

rm /tmp/test_shift_obs.csv
