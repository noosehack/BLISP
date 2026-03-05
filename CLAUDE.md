# BLISP Book of Laws

> Permanent architecture rules for the BLISP project.
> Every contributor (human or AI) must follow these without exception.

---

## 1. Code Is the Source of Truth

- IR enum variants (`NumericFunc`, `BinaryFunc`, etc.) are the canonical operation IDs.
- YAML files (`OPS_CURRENT.yml`, `OPS_PLANNED.yml`) validate against enums, never the reverse.
- If code and docs disagree, code wins. Fix the doc.

## 2. Canonical Pipeline

Every expression flows through exactly these stages, in order:

```
parse -> normalize -> canonicalize -> plan -> optimize -> execute
```

No stage may be skipped or reordered.

## 3. Three Layers

| Layer  | Meaning                                  |
|--------|------------------------------------------|
| IR     | Finance operations with IR plan nodes    |
| GLUE   | Language constructs (let, if, lambda)    |
| LEGACY | Builtins without IR support (yet)        |

Every public finance op must eventually reach IR. LEGACY is a temporary state.

## 4. PUBLIC_FINANCE_OPS Is the Policy Gate

- Every name in `PUBLIC_FINANCE_OPS` must be plannable as IR.
- Tripwire tests enforce this. If you add a name, you must also add its IR path.

## 5. Normalize Aliases Are Free

- Adding a normalize alias (e.g., `"add" -> "+"`) costs nothing at runtime.
- Aliases live in `NORMALIZE_ALIASES` in `normalize.rs`.
- The alias must map to a canonical name that is already plannable.

## 6. Composite IR Is the Migration Tool

- Operations like `diff` and `ecs1` decompose into multiple IR nodes in the planner.
- Each node uses an existing kernel. No new kernel needed unless semantics demand it.
- Example: `diff(x, k)` = `SUB(x, SHF_PTW_LIN_SHF{k}(x))` (3 nodes).

## 7. Introspection Tools Are Live

- `blisp --dic` shows the full operation matrix (CSV, semicolon-separated).
- These tools are the ground truth. If they disagree with docs, update the docs.

## 8. Pipeline Inspector

The command:

```
blisp -e '(expression)' --pipe
```

shows the complete execution pipeline.

Examples:

```
blisp -e '(-> (stdin) (locf) (dlog))' --pipe
cat data.csv | blisp -e '(-> (stdin) (locf) (dlog))' --pipe
```

Stages:

- PARSE
- NORMALIZE
- CANONICALIZE
- PLAN
- OPTIMIZE
- EXECUTE (implicit runtime stage)

Notes:

- The `-e` flag supplies the expression.
- `--pipe` prints the pipeline analysis for that expression.
- The EXECUTE stage may not appear as a labeled block in output but is part of the conceptual pipeline.

## 9. Fusion Rules

- Only elementwise operations may be fused.
- Shift/prefix operations (cumsum, shift, etc.) break fusion boundaries.
- The fusion optimizer in `ir_fusion.rs` enforces this automatically.

## 10. Testing Requirements

- `cargo test` must pass before any commit.
- `cargo clippy --all-targets --all-features -- -D warnings` must be clean.
- `cargo fmt` must produce no changes.
- Tripwire tests in `tests/orientation_tripwires.rs` guard orientation semantics.
- The `blisp --dic` matrix is verified by regression tests in `dic.rs`.

## 11. Commit Discipline

- Split unrelated changes into separate commits.
- Commit messages follow: `category: short description`
- Categories: `feat`, `fix`, `refactor`, `docs`, `test`, `ci`

## 12. No Hacks, No Workarounds

- If semantics are wrong, fix the kernel or add a new IR variant.
- Never use multiplicative corrections (e.g., `exp(-1)`) to patch semantic mismatches.
- Never add comment-only "fixes" for real bugs.

## 13. YAML Tracks Status, Not Behavior

- `OPS_CURRENT.yml`: aliases that resolve today (tripwire-enforced, 0 failures allowed).
- `OPS_PLANNED.yml`: roadmap items (failures expected and acceptable).
- Never put aspirational items in CURRENT.

## 14. Output Conventions

- Data goes to stdout, diagnostics go to stderr.
- CSV output uses `;` as separator.
- This enables clean piping: `blisp --dic 2>/dev/null | cut -d';' -f1,5`

## 15. Test Data Integrity

- All tests and investigations must use **canonical data files** or **repository fixtures**.
- No ad-hoc `printf`, `echo`, or heredoc-generated CSV for validation.
- All CSV must use semicolon delimiter (`;`), project-standard headers, and NA conventions.

### Canonical Data Files (in `/home/ubuntu/`)

| File | Description |
|------|-------------|
| `ES1I.csv` | Single-column (ES1 Index), ~9500 rows. Use for single-series tests. |
| `At.csv` | Multi-column (~500 columns), ~5100 rows. Use for multi-asset tests. |
| `smallAt.csv` | Small multi-column (6 columns), 9 rows. Use when you need small/fast. |

Always test with these first. If none fits, use `tests/fixtures/*.csv`.

### Rules

- Use `./scripts/pipe_fixture.sh <fixture> '<expr>'` for reproducible validation.
- Tripwire tests in `tests/fixture_integrity.rs` enforce format compliance in CI.
- A claim like "op X returns all NA" is invalid without a fixture path and exact `blisp -e` command.
- Any new fixture requires an explicit commit and review.

## 16. GLD_NUM Golden Test

The GLD_NUM test is the end-to-end numerical accuracy test. It runs an identical
finance pipeline in both CLISPI (reference) and BLISP, then compares outputs.

### Reference pipeline (CLISPI)

The reference script is `GLD_NUM_CLISPI.sh`. It requires `source Adyton.sh` for
shell utilities like `cgrep` (column-grep from CSV). The shell handles data
selection (`cgrep ../RAW_FUT_PRC.csv BZ1 TP1`) and pipes into CLISPI.

### BLISP replication

BLISP replicates **only the inner blisp expressions**, not the shell utilities.
The BLISP script (`GLD_NUM_BLISP.sh`) must also `source Adyton.sh` to get `cgrep`.
BLISP does not reimplement `cgrep` — it receives the same stdin.

### The pipeline

```
s = (-> (stdin) (w5) (dlog) (x- 1) (cs1) (wzs 25 1) (> -1) (shift 2))
result = (-> (file "../GC1C.csv") (mapr s) (dlog) (ur 250 5) (* s) (cs1))
```

### Validation rules

- **Compare values on non-weekend dates only.** CLISPI `w5` deletes weekend rows;
  BLISP `wkd` (canonical `MSK_WKE`) masks weekends as NA but keeps the rows.
  The output files will have different row counts. This is expected.
- Match criterion: values on shared weekday timestamps must agree within tolerance
  (default 5e-07).
- Row count mismatch alone is NOT a failure. Only value mismatch on weekday rows
  is a failure.
- Use `blisp verify` with `--tol` for automated comparison, filtering to weekday
  rows only.

### Data files (in `/home/ubuntu/`)

| File | Role |
|------|------|
| `RAW_FUT_PRC.csv` | Source prices (multi-asset, `cgrep` selects BZ1 + TP1) |
| `GC1C.csv` | Gold continuous contract (single column) |
| `GLD_NUM_CLISPI.csv` | Reference output (weekday rows only) |
| `GLD_NUM_BLISP.csv` | BLISP output (all rows, weekends masked as NA) |

## 17. The Matrix Columns

`blisp --dic` outputs these columns:

| Column     | Meaning                                      |
|------------|----------------------------------------------|
| NAME       | Operation name as typed by user               |
| ACCEPT     | Whether this name is accepted (yes/-)         |
| ACCEPT_WHY | Why it's accepted (normalize, plan, builtin)  |
| PUB        | Whether it's in PUBLIC_FINANCE_OPS            |
| CANON      | Canonical form after normalization            |
| USE        | Preferred spelling (if deprecated)            |
| LAYER      | IR / GLUE / LEGACY                           |
| IR_VARIANT | Which IR enum variant handles it              |
| FUSABLE    | Whether it participates in fusion             |
| NOTES      | Additional flags (dep, composite, etc.)       |

---

## Final Principle

When in doubt, run `blisp --pipe` and `blisp --dic`.
The code tells you what is true. Everything else is opinion.
