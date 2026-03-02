#!/bin/bash
# BLISP Smoke Test - Linux/macOS
# Validates installation and basic functionality

set -e

echo "=== BLISP Smoke Test ==="
echo ""

# Determine BLISP binary location
if [ -f "./target/release/blisp" ]; then
    BLISP="./target/release/blisp"
elif command -v blisp &> /dev/null; then
    BLISP="blisp"
else
    echo "❌ BLISP binary not found"
    echo "   Please build with: cargo build --locked --release"
    exit 1
fi

echo "Using binary: $BLISP"
echo ""

# 1. Build (if in source directory)
if [ -f "Cargo.toml" ]; then
    echo "[1/7] Building BLISP..."
    cargo build --locked --release --bin blisp
    echo "      ✅ Build successful"
    echo ""
fi

# 2. Version check
echo "[2/7] Testing --version flag..."
VERSION=$($BLISP --version 2>&1)
if [[ "$VERSION" =~ ^blisp\ v[0-9]+\.[0-9]+\.[0-9]+ ]]; then
    echo "      ✅ $VERSION"
else
    echo "      ❌ Unexpected version output: $VERSION"
    exit 1
fi
echo ""

# 3. Self-test
echo "[3/7] Testing --selftest flag..."
if $BLISP --selftest > /dev/null 2>&1; then
    echo "      ✅ Self-tests passed"
else
    echo "      ❌ Self-tests failed"
    exit 1
fi
echo ""

# 4. Hello world
echo "[4/7] Testing basic expression evaluation..."
RESULT=$($BLISP -e '(+ 1 2)' 2>&1 | grep -v "Running in" | tail -1)
if [[ "$RESULT" == "3" ]]; then
    echo "      ✅ Expression evaluation works"
else
    echo "      ❌ Expected '3', got '$RESULT'"
    exit 1
fi
echo ""

# 5. Quickstart examples (if available)
if [ -d "examples/quickstart" ]; then
    echo "[5/7] Testing quickstart examples..."
    if $BLISP run examples/quickstart/hello.blisp > /dev/null 2>&1; then
        echo "      ✅ hello.blisp runs successfully"
    else
        echo "      ❌ hello.blisp failed"
        exit 1
    fi

    if $BLISP run examples/quickstart/load_csv.blisp > /dev/null 2>&1; then
        echo "      ✅ load_csv.blisp runs successfully"
    else
        echo "      ❌ load_csv.blisp failed"
        exit 1
    fi
    echo ""
else
    echo "[5/7] Quickstart examples not found (skipping)"
    echo ""
fi

# 6. Verify subcommand
echo "[6/7] Testing verify subcommand..."
# Create test CSVs
cat > /tmp/blisp_test1.csv << 'EOF'
a;b;c
1;2;3
4;5;6
EOF

cat > /tmp/blisp_test2.csv << 'EOF'
a;b;c
1;2;3
4;5;6
EOF

if $BLISP verify /tmp/blisp_test1.csv /tmp/blisp_test2.csv > /dev/null 2>&1; then
    echo "      ✅ Verify passes for matching CSVs"
else
    echo "      ❌ Verify failed for matching CSVs"
    exit 1
fi

# Test with mismatched CSVs (should fail)
cat > /tmp/blisp_test3.csv << 'EOF'
a;b;c
1;2;3
4;5.5;6
EOF

if ! $BLISP verify /tmp/blisp_test3.csv /tmp/blisp_test2.csv > /dev/null 2>&1; then
    echo "      ✅ Verify correctly detects differences"
else
    echo "      ❌ Verify should have failed for different CSVs"
    exit 1
fi

# Cleanup
rm -f /tmp/blisp_test*.csv
echo ""

# 7. Example verification (if available)
if [ -f "examples/quickstart/load_csv.blisp" ] && [ -f "expected/quickstart_load_csv.csv" ]; then
    echo "[7/7] Testing example output verification..."
    $BLISP run examples/quickstart/load_csv.blisp 2>&1 | grep -v "Running in" > /tmp/blisp_output.csv
    if $BLISP verify /tmp/blisp_output.csv expected/quickstart_load_csv.csv --tol 1e-6 > /dev/null 2>&1; then
        echo "      ✅ Example output matches expected"
    else
        echo "      ❌ Example output verification failed"
        exit 1
    fi
    rm -f /tmp/blisp_output.csv
    echo ""
else
    echo "[7/7] Example verification data not found (skipping)"
    echo ""
fi

echo "✅ All Smoke Tests PASSED"
echo ""
