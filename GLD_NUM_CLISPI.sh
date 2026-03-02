#!/bin/bash
# GLD_NUM Golden Test - CLISPI Implementation
# SACRED SCRIPT - DO NOT MODIFY WITHOUT APPROVAL
# Purpose: Test ACCURACY and SPEED of CLISPI implementation
# Output: GLD_NUM_CLISPI.csv (format: TIMESTAMP;GLD_NUM with sep=";")

set -e

CLISPI="../clispi_dev/clispi_dev"
STDLIB="--load ../stdlib/finance_short.cl"

# Run the GLD_NUM pipeline (silent execution, clispi prints to stdout)
cgrep ../RAW_FUT_PRC.csv BZ1 TP1 | \
  $CLISPI $STDLIB -e \
  '(let* ((s (-> (stdin) (w5) (dlog) (x- 1) (cs1) (wzs 25 1) (> -1) (shift 2))))
     (-> (file "../GC1C.csv") (mapr s) (dlog) (ur 250 5) (* s) (cs1)))' \
  2>&1 | sed 1d | sed '1iTIMESTAMP;GLD_NUM' > GLD_NUM_CLISPI.csv
