#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-.}"
SRC="$ROOT/src"

if [[ ! -d "$SRC" ]]; then
  echo "ERROR: expected src/ under $ROOT" >&2
  exit 2
fi

# If blawktrust dlog_column is imported anywhere...
imports="$(grep -rn 'blawktrust::builtins::ops::.*dlog_column\|{[^}]*dlog_column[^}]*}' "$SRC" || true)"
if [[ -n "$imports" ]]; then
  # ...then forbid a local fn dlog_column definition.
  locals="$(grep -rn '^\s*fn\s\+dlog_column\s*(' "$SRC" || true)"
  if [[ -n "$locals" ]]; then
    echo "FAIL: local dlog_column() exists but blawktrust dlog_column is imported."
    echo
    echo "Local definitions:"
    echo "$locals" | sed 's/^/  /'
    echo
    echo "Imports:"
    echo "$imports" | sed 's/^/  /'
    echo
    echo "Fix: delete local dlog_column and call blawktrust::builtins::ops::dlog_column everywhere."
    exit 1
  fi
fi

echo "OK: no dlog_column kernel duplication detected."
