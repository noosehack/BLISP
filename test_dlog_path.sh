#!/bin/bash
# Test which execution path (dlog ...) takes

cd /home/ubuntu/blisp

echo "════════════════════════════════════════════════════════════"
echo "Testing (dlog ...) execution paths in BLISP"
echo "════════════════════════════════════════════════════════════"
echo ""

# Create test data
cat > /tmp/test_dlog.csv <<EOF
DATE,price
2020-01-01,100.0
2020-01-02,102.0
2020-01-03,101.5
EOF

echo "Test data created: /tmp/test_dlog.csv"
echo ""

# Test 1: Hybrid mode (default) - should use IR path
echo "━━━ Test 1: HYBRID mode (default) ━━━"
echo "Command: blisp -e '(dlog (file \"/tmp/test_dlog.csv\") 1)'"
echo ""
cargo run --quiet 2>&1 -- -e '(print (dlog (file "/tmp/test_dlog.csv") 1))' | head -20
echo ""

# Test 2: Legacy mode - should use builtin path
echo "━━━ Test 2: LEGACY mode (--legacy) ━━━"
echo "Command: blisp --legacy -e '(dlog (file \"/tmp/test_dlog.csv\") 1)'"
echo ""
cargo run --quiet 2>&1 -- --legacy -e '(print (dlog (file "/tmp/test_dlog.csv") 1))' | head -20
echo ""

# Test 3: IR-only mode - should use IR path
echo "━━━ Test 3: IR-ONLY mode (--ir-only) ━━━"
echo "Command: blisp --ir-only -e '(dlog (file \"/tmp/test_dlog.csv\") 1)'"
echo ""
cargo run --quiet 2>&1 -- --ir-only -e '(print (dlog (file "/tmp/test_dlog.csv") 1))' | head -20
echo ""

echo "════════════════════════════════════════════════════════════"
echo "Analysis:"
echo "════════════════════════════════════════════════════════════"
echo ""
echo "HYBRID mode: Should show '✅ Running in HYBRID mode'"
echo "             Should work (IR path succeeds for Frame operations)"
echo ""
echo "LEGACY mode: Should show '⚠️  Running in LEGACY mode'"
echo "             Uses builtin_dlog_cols (no IR, no optimization)"
echo ""
echo "IR-ONLY mode: Should show '🚧 Running in IR-ONLY mode'"
echo "              Uses planner → NumericFunc::SHF_PTW_NLN_DLOG"
echo ""
