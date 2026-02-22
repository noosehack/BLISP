#!/bin/bash
# Generate ES1I_locf_wzs_250_1.csv using BLISP
# Pipeline: ES1I.csv → locf → ft-zscore(250)

set -e  # Exit on error

INPUT_FILE="/home/ubuntu/ES1I.csv"
OUTPUT_FILE="/home/ubuntu/ES1I_locf_wzs_250_1.csv"

echo "=========================================="
echo "BLISP Pipeline: ES1I → locf → ft-zscore(250)"
echo "=========================================="
echo ""

# Check input exists
if [ ! -f "$INPUT_FILE" ]; then
    echo "ERROR: Input file not found: $INPUT_FILE"
    exit 1
fi

echo "Input:  $INPUT_FILE"
echo "Output: $OUTPUT_FILE"
echo ""

# Generate output
echo "Running BLISP pipeline..."
cd /home/ubuntu/blisp
cargo run --release -- -e \
  "(ft-zscore 250 (locf (read-csv \"$INPUT_FILE\")))" \
  2>/dev/null > "$OUTPUT_FILE"

# Verify output
if [ ! -s "$OUTPUT_FILE" ]; then
    echo "ERROR: Output file is empty or not created"
    exit 1
fi

echo "✅ Pipeline complete!"
echo ""
echo "Output statistics:"
wc -l "$OUTPUT_FILE"
ls -lh "$OUTPUT_FILE"
echo ""

echo "First 10 rows:"
head -10 "$OUTPUT_FILE"
echo ""

echo "Last 5 rows:"
tail -5 "$OUTPUT_FILE"
echo ""

echo "=========================================="
echo "File saved: $OUTPUT_FILE"
echo "=========================================="
