#!/usr/bin/env bash
# Extract complete operation inventory from BLISP source code

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "# BLISP Operation Inventory"
echo "# Generated: $(date -u +"%Y-%m-%d %H:%M:%S UTC")"
echo ""
echo "## 1. Legacy Builtins (src/builtins.rs)"
echo ""

# Extract builtin function names
grep -n "^fn builtin_" src/builtins.rs | sed 's/fn builtin_//' | sed 's/(.*$//' | sort | nl

echo ""
echo "## 2. IR Planner Tokens (src/planner.rs)"
echo ""

# Extract IR-recognized tokens from planner match statements
grep -A 2 "\"[a-z-]*\".*=>" src/planner.rs | grep "\"" | sed 's/.*"\([^"]*\)".*/\1/' | sort -u | nl

echo ""
echo "## 3. Canonical IDs (NumericFunc in src/ir.rs)"
echo ""

# Extract canonical operation IDs
grep -E "^\s+(SHF_|CUM_|WIN_|MSK_|BIN_)" src/ir.rs | sed 's/,.*$//' | sed 's/^\s*//' | sort | nl

echo ""
echo "## 4. Builtin Registry Tokens (get_builtin function)"
echo ""

# Extract registered builtin tokens
awk '/pub fn get_builtin/,/^}/' src/builtins.rs | grep "\"" | sed 's/.*"\([^"]*\)".*/\1/' | sort -u | nl

echo ""
echo "## 5. Cross-Reference Analysis"
echo ""

# Find tokens that appear in both planner and builtins
PLANNER_TOKENS=$(mktemp)
BUILTIN_TOKENS=$(mktemp)

grep -A 2 "\"[a-z-]*\".*=>" src/planner.rs | grep "\"" | sed 's/.*"\([^"]*\)".*/\1/' | sort -u > "$PLANNER_TOKENS"
awk '/pub fn get_builtin/,/^}/' src/builtins.rs | grep "\"" | sed 's/.*"\([^"]*\)".*/\1/' | sort -u > "$BUILTIN_TOKENS"

echo "### Tokens in BOTH planner and builtins (hybrid):"
comm -12 "$PLANNER_TOKENS" "$BUILTIN_TOKENS" | nl

echo ""
echo "### Tokens ONLY in planner (IR-only):"
comm -13 "$BUILTIN_TOKENS" "$PLANNER_TOKENS" | nl

echo ""
echo "### Tokens ONLY in builtins (legacy-only):"
comm -23 "$BUILTIN_TOKENS" "$PLANNER_TOKENS" | nl

rm "$PLANNER_TOKENS" "$BUILTIN_TOKENS"

echo ""
echo "## 6. Macro Expansions (src/normalize.rs)"
echo ""

# Extract macro expansion patterns
grep "expand_" src/normalize.rs | grep "fn " | sed 's/fn expand_//' | sed 's/(.*$//' | sort | nl
