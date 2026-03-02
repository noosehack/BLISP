#!/bin/bash
# GLD_NUM Golden Test - BLISP Implementation
# SACRED SCRIPT - DO NOT MODIFY WITHOUT APPROVAL
# Purpose: Test ACCURACY and SPEED of BLISP implementation
# Output: GLD_NUM_BLISP.csv (format: TIMESTAMP;GLD_NUM with sep=";")

set -e

BLISP="./target/release/blisp"
COMPAT="--load stdlib/compat_clispi.cl"

# Run the GLD_NUM pipeline (silent execution)
cgrep ../RAW_FUT_PRC.csv BZ1 TP1 | \
  $BLISP $COMPAT -e \
  '(save "GLD_NUM_BLISP_temp.csv"
     (let* ((s (-> (stdin) (w5) (dlog) (x- 1) (cs1) (wzs 25 1) (> -1) (shift 2))))
       (-> (file "../GC1C.csv") (mapr s) (dlog) (ur 250 5) (* s) (cs1))))' \
  > /dev/null 2>&1

# Replace first line with proper header (TIMESTAMP;GLD_NUM)
sed -i '1s/.*/TIMESTAMP;GLD_NUM/' GLD_NUM_BLISP_temp.csv
mv GLD_NUM_BLISP_temp.csv GLD_NUM_BLISP.csv
