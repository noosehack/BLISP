# PR0 Tripwire Report

**Date**: 2026-02-26
**Status**: ✅ TRIPWIRES DEPLOYED AND VERIFIED

---

## Tripwire Scripts Created

### 1. `ci/test_no_token_conflicts.sh`
**Purpose**: Detect tokens defined in BOTH `builtins.rs` and `planner.rs` (shadowing bug)

**Location**: `/home/ubuntu/blisp/ci/test_no_token_conflicts.sh`

**Algorithm**:
1. Extract all tokens from `register_builtin("TOKEN", ...)` in builtins.rs
2. Extract all tokens from `"TOKEN" => ...` patterns in planner.rs
3. Compute intersection (comm -12)
4. FAIL if any conflicts found

**Exit codes**:
- `0` = OK (no conflicts)
- `1` = FAIL (conflicts detected)
- `2` = ERROR (missing files)

### 2. `ci/test_no_kernel_dupes.sh`
**Purpose**: Detect local kernel definitions that shadow imported blawktrust kernels

**Location**: `/home/ubuntu/blisp/ci/test_no_kernel_dupes.sh`

**Algorithm**:
1. Search for blawktrust dlog_column imports
2. If found, search for local `fn dlog_column(` definitions
3. FAIL if both exist

**Exit codes**:
- `0` = OK (no duplication)
- `1` = FAIL (duplicate detected)
- `2` = ERROR (missing src/ directory)

---

## Verification Results (Current Codebase)

### Test 1: Token Conflicts ❌ FAILS (Expected)

```bash
$ cd /home/ubuntu/blisp && ./ci/test_no_token_conflicts.sh .
```

**Output**:
```
FAIL: token(s) are defined in BOTH builtins and planner (shadowing IR):
  - *
  - +
  - -
  - /
  - >
  - abs
  - asofr
  - cs1
  - dlog
  - exp
  - locf
  - log
  - mapr
  - mask-weekend
  - shift
  - stdin
  - ur
  - with-mask
  - wkd
  - xminus

Fix: remove builtin registration or rename to legacy/<token>.
```

**Conflicts Detected**: 20 tokens (18 from inventory + stdin + ur)

**Evidence**:
| Token | Builtin Reg | Planner Map | Category |
|-------|-------------|-------------|----------|
| `*` | builtins.rs:65 | planner.rs:521 | Arithmetic |
| `+` | builtins.rs:63 | planner.rs:519 | Arithmetic |
| `-` | builtins.rs:64 | planner.rs:520 | Arithmetic |
| `/` | builtins.rs:66 | planner.rs:522 | Arithmetic |
| `>` | builtins.rs:113 | planner.rs:523 | Comparison |
| `abs` | builtins.rs:71 | planner.rs:128 | Math |
| `asofr` | builtins.rs:147 | planner.rs:527 | Join |
| `cs1` | builtins.rs:138 | planner.rs:132 | Core |
| `dlog` | builtins.rs:74 | planner.rs:123 | Core |
| `exp` | builtins.rs:70 | planner.rs:126 | Math |
| `locf` | builtins.rs:123 | planner.rs:130 | Core |
| `log` | builtins.rs:69 | planner.rs:125 | Math |
| `mapr` | builtins.rs:146 | planner.rs:526 | Join |
| `mask-weekend` | builtins.rs:130 | planner.rs:557 | Schema |
| `shift` | builtins.rs:75 | planner.rs:135 | Core |
| `stdin` | builtins.rs:94 | planner.rs:108 | I/O |
| `ur` | builtins.rs:148 | planner.rs:407 | Composite |
| `with-mask` | builtins.rs:131 | planner.rs:586 | Schema |
| `wkd` | builtins.rs:129 | planner.rs:131 | Core |
| `xminus` | builtins.rs:137 | planner.rs:530 | Schema |

### Test 2: Kernel Duplication ❌ FAILS (Expected)

```bash
$ cd /home/ubuntu/blisp && ./ci/test_no_kernel_dupes.sh .
```

**Output**:
```
FAIL: local dlog_column() exists but blawktrust dlog_column is imported.

Local definitions:
  src/exec.rs:1092:fn dlog_column(col: &Column, _lag: usize) -> Column {

Imports:
  src/builtins.rs:12:use blawktrust::builtins::ops::{dlog_column, wstd, wstd0, wzscore};

Fix: delete local dlog_column and call blawktrust::builtins::ops::dlog_column everywhere.
```

**Duplication Confirmed**:
- **Local**: exec.rs:1092
- **Import**: builtins.rs:12 (from blawktrust)

**Risk**: Divergent implementations, undefined behavior depending on which is called.

---

## CI Integration

### GitHub Actions (`.github/workflows/ci.yml`)

Add step before tests:

```yaml
- name: Architecture Tripwires
  run: |
    bash ci/test_no_token_conflicts.sh .
    bash ci/test_no_kernel_dupes.sh .
```

### Local Pre-Push Hook (`.git/hooks/pre-push`)

```bash
#!/bin/bash
cd "$(git rev-parse --show-toplevel)"
bash ci/test_no_token_conflicts.sh . || exit 1
bash ci/test_no_kernel_dupes.sh . || exit 1
```

### Make Integration

Add to `Makefile` (if exists):

```makefile
.PHONY: tripwire
tripwire:
	@bash ci/test_no_token_conflicts.sh .
	@bash ci/test_no_kernel_dupes.sh .

test: tripwire
	# ... existing test commands
```

---

## Expected Behavior After PR1

After removing the 20 conflict builtin registrations, tripwire 1 should pass:

```bash
$ ./ci/test_no_token_conflicts.sh .
OK: no planner/builtin token conflicts.
```

---

## Expected Behavior After PR2

After deleting `exec.rs:1092` dlog_column, tripwire 2 should pass:

```bash
$ ./ci/test_no_kernel_dupes.sh .
OK: no dlog_column kernel duplication detected.
```

---

## Tripwire Maintenance

### Adding More Kernel Checks

To detect duplication for other kernels, extend `test_no_kernel_dupes.sh`:

```bash
# Check multiple kernels
for kernel in dlog_column shift_column locf_column rolling_mean rolling_std; do
  imports="$(grep -rn "blawktrust.*$kernel" "$SRC" || true)"
  if [[ -n "$imports" ]]; then
    locals="$(grep -rn "^\s*fn\s\+$kernel\s*(" "$SRC" || true)"
    if [[ -n "$locals" ]]; then
      echo "FAIL: $kernel duplicated"
      exit 1
    fi
  fi
done
```

### False Positive Handling

If legitimate shadowing is needed (unlikely), whitelist specific tokens:

```bash
# In test_no_token_conflicts.sh, before conflict check:
whitelist="legacy-token-name another-whitelist"
conflicts="$(comm -12 <(...) <(...) | grep -vE "^($whitelist)$" || true)"
```

---

## Summary

✅ **Tripwire 1**: Detects 20 token conflicts (catches the bug)
✅ **Tripwire 2**: Detects dlog_column duplication (catches the bug)
✅ **Scripts**: Executable, deterministic, CI-ready
✅ **Documentation**: This report + inline comments

**Next Step**: PR1 (remove 20 builtin registrations)

---

**Files Created**:
- `/home/ubuntu/blisp/ci/test_no_token_conflicts.sh`
- `/home/ubuntu/blisp/ci/test_no_kernel_dupes.sh`
- `/home/ubuntu/blisp/PR0_TRIPWIRE_REPORT.md` (this file)
