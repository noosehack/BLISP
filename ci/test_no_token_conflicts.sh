#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-.}"
BUILTINS="$ROOT/src/builtins.rs"
PLANNER="$ROOT/src/planner.rs"

if [[ ! -f "$BUILTINS" || ! -f "$PLANNER" ]]; then
  echo "ERROR: expected src/builtins.rs and src/planner.rs under $ROOT" >&2
  exit 2
fi

# Extract builtin tokens: register_builtin("token", ...)
builtin_tokens="$(grep -oP 'register_builtin\("\K[^"]+' "$BUILTINS" | sort -u)"

# Extract planner tokens: "token" => ...
planner_tokens="$(grep -oP '^\s*"\K[^"]+(?="\s*=>)' "$PLANNER" | sort -u)"

# Compute intersection
conflicts="$(comm -12 <(printf "%s\n" "$builtin_tokens") <(printf "%s\n" "$planner_tokens") || true)"

if [[ -n "$conflicts" ]]; then
  echo "FAIL: token(s) are defined in BOTH builtins and planner (shadowing IR):"
  echo "$conflicts" | sed 's/^/  - /'
  echo
  echo "Fix: remove builtin registration or rename to legacy/<token>."
  exit 1
fi

echo "OK: no planner/builtin token conflicts."
