#!/usr/bin/env bash
# PR1: Remove 20 builtin registrations that shadow IR mappings
# Execute from blisp repo root

set -euo pipefail

BUILTINS="src/builtins.rs"

if [[ ! -f "$BUILTINS" ]]; then
  echo "ERROR: expected $BUILTINS in current directory" >&2
  exit 1
fi

# Backup
cp "$BUILTINS" "${BUILTINS}.pr1_backup"
echo "✓ Backup created: ${BUILTINS}.pr1_backup"

# Delete lines in REVERSE order (high to low) to avoid line number shifts
# Line numbers are from original file
sed -i '148d' "$BUILTINS"  # ur
sed -i '147d' "$BUILTINS"  # asofr
sed -i '146d' "$BUILTINS"  # mapr
sed -i '138d' "$BUILTINS"  # cs1
sed -i '137d' "$BUILTINS"  # xminus
sed -i '131d' "$BUILTINS"  # with-mask
sed -i '130d' "$BUILTINS"  # mask-weekend
sed -i '129d' "$BUILTINS"  # wkd
sed -i '123d' "$BUILTINS"  # locf
sed -i '113d' "$BUILTINS"  # >
sed -i '94d' "$BUILTINS"   # stdin
sed -i '75d' "$BUILTINS"   # shift
sed -i '74d' "$BUILTINS"   # dlog
sed -i '71d' "$BUILTINS"   # abs
sed -i '70d' "$BUILTINS"   # exp
sed -i '69d' "$BUILTINS"   # log
sed -i '66d' "$BUILTINS"   # /
sed -i '65d' "$BUILTINS"   # *
sed -i '64d' "$BUILTINS"   # -
sed -i '63d' "$BUILTINS"   # +

echo "✓ Deleted 20 builtin registrations"

# Verify tripwire passes
echo
echo "Running tripwire..."
if bash ci/test_no_token_conflicts.sh .; then
  echo "✅ SUCCESS: No token conflicts detected"
else
  echo "❌ FAIL: Tripwire still detects conflicts"
  echo "   Restoring backup..."
  mv "${BUILTINS}.pr1_backup" "$BUILTINS"
  exit 1
fi

# Test compilation
echo
echo "Testing compilation..."
if cargo build --quiet 2>&1 | grep -i error; then
  echo "❌ FAIL: Compilation errors detected"
  echo "   Restoring backup..."
  mv "${BUILTINS}.pr1_backup" "$BUILTINS"
  exit 1
else
  echo "✅ SUCCESS: Compilation passed"
fi

echo
echo "======================================"
echo "PR1 EDITS COMPLETE"
echo "======================================"
echo "Deleted: 20 builtin registrations"
echo "Backup: ${BUILTINS}.pr1_backup"
echo "Tripwire: PASS"
echo "Compile: PASS"
echo
echo "Next steps:"
echo "1. Run tests: cargo test"
echo "2. Review diff: git diff $BUILTINS"
echo "3. Commit: git commit -am 'PR1: Remove builtin shadowing for 20 IR-mapped tokens'"
