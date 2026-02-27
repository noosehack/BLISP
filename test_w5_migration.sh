#!/bin/bash
# Test w5 alias migration - verify double-fail is eliminated

set -e

BLISP="./target/debug/blisp"
TEST_DATA="/tmp/test_dates.csv"

echo "=== W5 Alias Migration Test Suite ==="
echo ""
echo "Goal: Verify w5 routes through IR planner and eliminates double-fail"
echo ""

# Create test data
cat > "$TEST_DATA" << 'EOF'
date,PRC,VOL,RET
2024-01-01,100.0,1000000,0.01
2024-01-02,102.0,1100000,0.02
2024-01-03,101.0,900000,-0.01
2024-01-04,105.0,1200000,0.04
2024-01-05,103.0,950000,-0.02
2024-01-08,107.0,1300000,0.04
2024-01-09,106.0,1150000,-0.01
2024-01-10,110.0,1400000,0.04
2024-01-11,108.0,1250000,-0.02
2024-01-12,112.0,1500000,0.04
EOF

echo "Test 1: (dlog (w5 X))"
echo "Expected: Deprecation warning + IR routing (no 'Unknown function' error)"
echo -n "Result: "
$BLISP -e "(dlog (w5 (file \"$TEST_DATA\")))" 2>&1 | grep -q "Warning: 'w5' is deprecated" && echo "✅ PASS - Warning emitted" || echo "❌ FAIL"
$BLISP -e "(dlog (w5 (file \"$TEST_DATA\")))" 2>&1 | grep -q "Unknown function" && echo "❌ FAIL - Unknown function error" || echo "✅ PASS - No unknown function error"
echo ""

echo "Test 2: (ur 250 1 (w5 X))"
echo "Expected: Deprecation warning + IR routing"
echo -n "Result: "
$BLISP -e "(ur 250 1 (w5 (file \"$TEST_DATA\")))" 2>&1 | grep -q "Warning: 'w5' is deprecated" && echo "✅ PASS - Warning emitted" || echo "❌ FAIL"
$BLISP -e "(ur 250 1 (w5 (file \"$TEST_DATA\")))" 2>&1 | grep -q "Unknown function" && echo "❌ FAIL - Unknown function error" || echo "✅ PASS - No unknown function error"
echo ""

echo "Test 3: (shift 1 (w5 X))"
echo "Expected: Deprecation warning + IR routing"
echo -n "Result: "
$BLISP -e "(shift 1 (w5 (file \"$TEST_DATA\")))" 2>&1 | grep -q "Warning: 'w5' is deprecated" && echo "✅ PASS - Warning emitted" || echo "❌ FAIL"
$BLISP -e "(shift 1 (w5 (file \"$TEST_DATA\")))" 2>&1 | grep -q "Unknown function" && echo "❌ FAIL - Unknown function error" || echo "✅ PASS - No unknown function error"
echo ""

echo "Test 4: (dlog (shift 1 (w5 X))) - Nested"
echo "Expected: Deprecation warning + IR routing for all operations"
echo -n "Result: "
$BLISP -e "(dlog (shift 1 (w5 (file \"$TEST_DATA\"))))" 2>&1 | grep -q "Warning: 'w5' is deprecated" && echo "✅ PASS - Warning emitted" || echo "❌ FAIL"
$BLISP -e "(dlog (shift 1 (w5 (file \"$TEST_DATA\"))))" 2>&1 | grep -q "Unknown function" && echo "❌ FAIL - Unknown function error" || echo "✅ PASS - No unknown function error"
echo ""

echo "Test 5: Verify w5 is in planner.rs"
echo -n "Result: "
grep -q '"w5"' src/planner.rs && echo "✅ PASS - w5 alias found in planner.rs" || echo "❌ FAIL"
echo ""

echo "Test 6: Verify w5 builtin still registered (backward compat)"
echo -n "Result: "
grep -q 'register_builtin.*"w5"' src/builtins.rs && echo "✅ PASS - w5 builtin still registered" || echo "❌ FAIL"
echo ""

echo "=== Summary ==="
echo "If all tests show ✅ PASS:"
echo "  - w5 routes through IR planner"
echo "  - Deprecation warnings work"
echo "  - No 'Unknown function' errors (double-fail eliminated)"
echo "  - Backward compatibility maintained (builtin still exists)"
echo ""
echo "Note: Actual computation may still fail due to data requirements"
echo "      (e.g., wkd needs Date index), but dispatch routing is correct."
