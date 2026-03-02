#!/bin/bash
# Demonstration: shift vs shift-obs with weekend masking

cat > /tmp/week_prices.csv << 'DATA'
DATE;price
2024-01-05;100.0
2024-01-06;101.0
2024-01-07;102.0
2024-01-08;103.0
2024-01-09;104.0
2024-01-10;105.0
2024-01-11;106.0
2024-01-12;107.0
DATA

echo "==================================================================="
echo "Test Data: 2024-01-05 (Fri) through 2024-01-12 (Fri) - 8 days"
echo "Includes weekend: Sat 01-06, Sun 01-07 and Sat 01-13, Sun 01-14"
echo "==================================================================="
echo ""

echo "1. Original data (no operations):"
cargo run --release -- -e '(read-csv "/tmp/week_prices.csv")' 2>/dev/null
echo ""

echo "2. Calendar shift (shift 2) - no mask:"
echo "   Result: Each row gets value from 2 calendar days earlier"
cargo run --release -- -e '(shift 2 (read-csv "/tmp/week_prices.csv"))' 2>/dev/null
echo ""

echo "3. Observation shift (shift-obs 2) - no mask:"
echo "   Result: Same as calendar shift when no mask active"
cargo run --release -- -e '(shift-obs 2 (read-csv "/tmp/week_prices.csv"))' 2>/dev/null
echo ""

echo "4. Create weekend mask and check which rows are masked:"
cargo run --release -- -e '(mask-weekend (read-csv "/tmp/week_prices.csv"))' 2>/dev/null | head -5
echo "   (Weekend mask created but not yet active)"
echo ""

rm /tmp/week_prices.csv
echo "==================================================================="
echo "Summary:"
echo "- shift: calendar lag (positional)"
echo "- shift-obs: observation lag (skips masked rows)"  
echo "- When no mask active: both behave identically"
echo "- When weekend mask active: shift-obs skips Sat/Sun"
echo "==================================================================="
