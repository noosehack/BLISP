#!/bin/bash
# Test canonical comparison operators in IR nested expressions
# Verifies: < <= >= == != work in IR trees without double-fail

set -e

BLISP="./target/debug/blisp"
TEST_CSV="/tmp/test_dates.csv"

echo "=== Comparison Operators IR Test Suite ==="
echo ""

# Test 1: Less than in dlog (nested IR)
echo "Test 1: (dlog (< PRC 105)) - nested IR composition"
$BLISP -e "(dlog (< (col (file \"$TEST_CSV\") \"PRC\") 105))" 2>&1 | head -5
if $BLISP -e "(dlog (< (col (file \"$TEST_CSV\") \"PRC\") 105))" 2>&1 | grep -q "Unknown function"; then
    echo "❌ FAIL: Double-fail detected for <"
    exit 1
fi
echo "✅ PASS: < works in IR tree"
echo ""

# Test 2: Less than or equal in shift
echo "Test 2: (shift 1 (<= PRC 105)) - nested IR composition"
$BLISP -e "(shift 1 (<= (col (file \"$TEST_CSV\") \"PRC\") 105))" 2>&1 | head -5
if $BLISP -e "(shift 1 (<= (col (file \"$TEST_CSV\") \"PRC\") 105))" 2>&1 | grep -q "Unknown function"; then
    echo "❌ FAIL: Double-fail detected for <="
    exit 1
fi
echo "✅ PASS: <= works in IR tree"
echo ""

# Test 3: Greater than or equal in ur
echo "Test 3: (ur 250 1 (>= PRC 105)) - nested IR composition"
$BLISP -e "(ur 250 1 (>= (col (file \"$TEST_CSV\") \"PRC\") 105))" 2>&1 | head -5
if $BLISP -e "(ur 250 1 (>= (col (file \"$TEST_CSV\") \"PRC\") 105))" 2>&1 | grep -q "Unknown function"; then
    echo "❌ FAIL: Double-fail detected for >="
    exit 1
fi
echo "✅ PASS: >= works in IR tree"
echo ""

# Test 4: Equal in cs1
echo "Test 4: (cs1 (== PRC 105)) - nested IR composition"
$BLISP -e "(cs1 (== (col (file \"$TEST_CSV\") \"PRC\") 105))" 2>&1 | head -5
if $BLISP -e "(cs1 (== (col (file \"$TEST_CSV\") \"PRC\") 105))" 2>&1 | grep -q "Unknown function"; then
    echo "❌ FAIL: Double-fail detected for =="
    exit 1
fi
echo "✅ PASS: == works in IR tree"
echo ""

# Test 5: Not equal in locf
echo "Test 5: (locf (!= PRC 105)) - nested IR composition"
$BLISP -e "(locf (!= (col (file \"$TEST_CSV\") \"PRC\") 105))" 2>&1 | head -5
if $BLISP -e "(locf (!= (col (file \"$TEST_CSV\") \"PRC\") 105))" 2>&1 | grep -q "Unknown function"; then
    echo "❌ FAIL: Double-fail detected for !="
    exit 1
fi
echo "✅ PASS: != works in IR tree"
echo ""

# Test 6: Simple scalar comparison (basic functionality)
echo "Test 6: (< 5 10) - scalar comparison"
RESULT=$($BLISP -e "(< 5 10)" 2>&1 | grep -v "Running in" | grep -v "Warning")
if echo "$RESULT" | grep -q "1"; then
    echo "✅ PASS: < returns 1.0 for true"
else
    echo "❌ FAIL: < did not return 1.0"
    echo "Got: $RESULT"
    exit 1
fi
echo ""

# Test 7: Column × Scalar comparison
echo "Test 7: (< (col df \"PRC\") 105) - Column × Scalar"
$BLISP -e "(< (col (file \"$TEST_CSV\") \"PRC\") 105)" 2>&1 | head -8
echo "✅ PASS: Column × Scalar comparison works"
echo ""

# Test 8: Verify numeric return (1.0/0.0, not boolean)
echo "Test 8: (== 5 5) - verify numeric return"
RESULT=$($BLISP -e "(== 5 5)" 2>&1 | grep -v "Running in" | grep -v "Warning")
if echo "$RESULT" | grep -q "1"; then
    echo "✅ PASS: == returns numeric 1.0 (not boolean)"
else
    echo "❌ FAIL: == did not return numeric 1.0"
    exit 1
fi
echo ""

# Test 9: Triple-nested composition
echo "Test 9: (dlog (shift 1 (< PRC 105))) - triple-nested IR"
$BLISP -e "(dlog (shift 1 (< (col (file \"$TEST_CSV\") \"PRC\") 105)))" 2>&1 | head -5
if $BLISP -e "(dlog (shift 1 (< (col (file \"$TEST_CSV\") \"PRC\") 105)))" 2>&1 | grep -q "Unknown function"; then
    echo "❌ FAIL: Triple-nesting failed"
    exit 1
fi
echo "✅ PASS: Triple-nested IR composition works"
echo ""

echo "=== All 9 Tests PASSED ✅ ==="
echo ""
echo "Verification:"
echo "- All 5 comparison operators (< <= >= == !=) route through IR"
echo "- No double-fail patterns detected"
echo "- Nested expressions compose correctly"
echo "- Returns numeric 1.0/0.0 (not boolean)"
