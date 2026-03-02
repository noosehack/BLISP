#!/usr/bin/env bash
# Release gate for BLISP
# Run this before creating any release tag
# Exit code 0 = ready to release, non-zero = not ready

set -e  # Exit on any error

echo "=== BLISP Release Gate ==="
echo ""

# Check we're on a clean commit
if ! git diff-index --quiet HEAD --; then
    echo "❌ FAIL: Working directory has uncommitted changes"
    echo "Commit or stash changes before releasing"
    exit 1
fi
echo "✅ Working directory is clean"

# Check blawktrust dependency is locked to a tag (not path or branch)
echo ""
echo "Checking blawktrust dependency..."
if grep -q 'blawktrust.*path.*=' Cargo.toml; then
    echo "❌ FAIL: blawktrust is using path dependency"
    echo "Change to: blawktrust = { git = \"...\", tag = \"v0.x.y\" }"
    exit 1
fi

if grep -q 'blawktrust.*git.*tag' Cargo.toml; then
    TAG=$(grep 'blawktrust.*git' Cargo.toml | grep -o 'tag = "[^"]*"' | cut -d'"' -f2)
    echo "✅ blawktrust locked to tag: $TAG"
else
    echo "❌ FAIL: blawktrust dependency must be locked to a specific tag"
    echo "Change to: blawktrust = { git = \"https://github.com/noosehack/blawktrust\", tag = \"v0.x.y\" }"
    exit 1
fi

# Run formatting check
echo ""
echo "Running cargo fmt check..."
if ! cargo fmt --all -- --check; then
    echo "❌ FAIL: Code is not formatted"
    echo "Run: cargo fmt --all"
    exit 1
fi
echo "✅ Formatting check passed"

# Run clippy
echo ""
echo "Running clippy..."
if ! cargo clippy --all-targets --all-features -- -D warnings; then
    echo "❌ FAIL: Clippy found issues"
    exit 1
fi
echo "✅ Clippy check passed"

# Run lib tests
echo ""
echo "Running library tests..."
if ! cargo test --lib; then
    echo "❌ FAIL: Library tests failed"
    exit 1
fi
echo "✅ Library tests passed"

# Run critical integration test (blawktrust API surface)
echo ""
echo "Running blawktrust API integration test..."
if ! cargo test --test blawktrust_api_integration; then
    echo "❌ FAIL: blawktrust API integration test failed"
    echo "This means blawktrust's API has changed in a way that breaks BLISP"
    exit 1
fi
echo "✅ Integration test passed"

# Build release binary
echo ""
echo "Building release binary..."
if ! cargo build --release --bin blisp; then
    echo "❌ FAIL: Release build failed"
    exit 1
fi
echo "✅ Release build succeeded"

# Run smoke test
echo ""
echo "Running smoke test..."
cat > /tmp/blisp_smoke_test.lisp << 'EOF'
(defparameter df (stdin))
(print (sum df))
(print (sum (o 'Z df)))
(print (sum (o 'R df)))
EOF
if ! echo -e "a;b\n1;2\n3;4" | ./target/release/blisp /tmp/blisp_smoke_test.lisp > /dev/null; then
    echo "❌ FAIL: Smoke test failed"
    exit 1
fi
echo "✅ Smoke test passed"

# Check if tag already exists
if [ -n "$1" ]; then
    TAG_NAME=$1
    if git rev-parse "$TAG_NAME" >/dev/null 2>&1; then
        echo ""
        echo "⚠️  WARNING: Tag $TAG_NAME already exists"
        echo "NEVER move existing tags! Create a new tag instead (e.g., $TAG_NAME-fixed or increment version)"
        exit 1
    fi
    echo "✅ Tag $TAG_NAME does not exist yet"
fi

# All checks passed
echo ""
echo "========================================="
echo "✅ ALL CHECKS PASSED - READY TO RELEASE"
echo "========================================="
echo ""
echo "Next steps:"
echo "  1. Create tag:  git tag v0.x.y"
echo "  2. Push tag:    git push origin v0.x.y"
echo "  3. NEVER move or delete tags once pushed"
echo ""
echo "Remember: Tags are immutable contracts!"

exit 0
