#!/bin/bash
# GLD_NUM Speed Benchmark
# Compares BLISP vs CLISPI execution time

echo "=========================================="
echo "GLD_NUM Speed Benchmark"
echo "=========================================="
echo ""

# Benchmark BLISP
echo -n "BLISP:   "
START_BLISP=$(date +%s.%N)
./GLD_NUM_BLISP.sh
END_BLISP=$(date +%s.%N)
TIME_BLISP=$(echo "$END_BLISP - $START_BLISP" | bc -l)

# Benchmark CLISPI
echo -n "CLISPI:  "
START_CLISPI=$(date +%s.%N)
./GLD_NUM_CLISPI.sh
END_CLISPI=$(date +%s.%N)
TIME_CLISPI=$(echo "$END_CLISPI - $START_CLISPI" | bc -l)

echo ""
echo "=========================================="
echo "Results:"
echo "=========================================="
printf "BLISP:   %.3f seconds\n" $TIME_BLISP
printf "CLISPI:  %.3f seconds\n" $TIME_CLISPI
echo ""

# Calculate speedup
python3 << EOF
blisp = float("$TIME_BLISP")
clispi = float("$TIME_CLISPI")

if blisp < clispi:
    speedup = clispi / blisp
    print(f"✅ BLISP is {speedup:.2f}x FASTER than CLISPI")
elif clispi < blisp:
    speedup = blisp / clispi
    print(f"⚠️  CLISPI is {speedup:.2f}x FASTER than BLISP")
else:
    print("⚡ Same speed")
EOF

echo ""
echo "=========================================="
echo "Accuracy Check:"
echo "=========================================="

BLISP_VAL=$(tail -1 GLD_NUM_BLISP.csv | cut -d';' -f2)
CLISPI_VAL=$(tail -1 GLD_NUM_CLISPI.csv | cut -d';' -f2)

python3 << EOF
blisp = float("$BLISP_VAL")
clispi = float("$CLISPI_VAL")
diff = abs(blisp - clispi)
pct = (diff / clispi) * 100

print(f"BLISP:   {blisp}")
print(f"CLISPI:  {clispi}")
print(f"Diff:    {diff:.10f} ({pct:.6f}%)")
print("")

if pct < 0.01:
    print("✅ EXCELLENT accuracy (< 0.01%)")
elif pct < 0.1:
    print("✅ GOOD accuracy (< 0.1%)")
else:
    print("⚠️  WARNING: Difference > 0.1%")
EOF
