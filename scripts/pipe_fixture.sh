#!/usr/bin/env bash
# Usage: ./scripts/pipe_fixture.sh <fixture.csv> '<blisp-expression>' [--pipe]
#
# Runs a BLISP expression against a checked-in fixture file.
# All validation must use this script or direct fixture paths — never ad-hoc CSV.
#
# Examples:
#   ./scripts/pipe_fixture.sh tests/fixtures/clean_8row.csv '(-> (stdin) (rolling-std 3))'
#   ./scripts/pipe_fixture.sh tests/fixtures/clean_8row.csv '(-> (stdin) (dlog))' --pipe

set -euo pipefail

if [ $# -lt 2 ]; then
    echo "Usage: $0 <fixture.csv> '<expression>' [--pipe]" >&2
    exit 1
fi

FIXTURE="$1"
EXPR="$2"
shift 2

if [ ! -f "$FIXTURE" ]; then
    echo "ERROR: fixture not found: $FIXTURE" >&2
    exit 1
fi

# Validate fixture uses semicolon delimiter (check header)
HEADER=$(head -1 "$FIXTURE")
if echo "$HEADER" | grep -q ',' && ! echo "$HEADER" | grep -q ';'; then
    echo "ERROR: fixture uses comma delimiter, not semicolon: $FIXTURE" >&2
    exit 1
fi

BLISP="${BLISP:-./target/release/blisp}"
if [ ! -x "$BLISP" ]; then
    BLISP="./target/debug/blisp"
fi
if [ ! -x "$BLISP" ]; then
    echo "ERROR: blisp binary not found. Run 'cargo build --release' first." >&2
    exit 1
fi

cat "$FIXTURE" | "$BLISP" -e "$EXPR" "$@"
