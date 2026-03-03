# Production Hardening - Step 7

**Purpose:** Close the gap between "CI green + selftests" and "robust in the wild"

**Status:** 🟡 Gaps identified, ready for implementation

---

## Current State ✅

**What Works:**
- ✅ Master CI green (commit 5bb68ff)
- ✅ GitHub Release v0.2.0 published (tag exists)
- ✅ 8 CI jobs passing (fmt, clippy, test, build, smoke, user-smoke, user-install-fresh, ignored-test-count)
- ✅ Embedded selftests (6 tripwires, <1 second)
- ✅ GLD_NUM verification (6826 rows, matches CLISPI within 5e-07)
- ✅ Benchmark infrastructure exists (`benches/perf_guardrails.rs`)
- ✅ Medium golden test exists (`examples/gld_num_mini.blisp`, 100 rows)
- ✅ Data files bundled (`data/gld_num_mini/`, `data/quickstart/`)

---

## Gap Analysis

### 1. Install-from-Tag ❌

**Current:**
```yaml
# user-install-fresh job uses --branch ${{ github.ref_name }}
cargo install blisp --git ... --branch ${{ github.ref_name }} --locked
```

**Problem:** Users will use `--tag v0.2.0`, not `--branch master`

**Fix Required:**
- Add separate CI job: `user-install-from-tag`
- Test: `cargo install blisp --git ... --tag v0.2.0 --locked`
- Ensures release artifact is what users actually install

**Priority:** 🔴 HIGH - Prevents "branch moved" ambiguity

---

### 2. No-Network / Deterministic Smoke ❌

**Current:** CI downloads dependencies from crates.io during build

**Problem:** Cannot prove "zero-state / pinned deps / no network" claim

**Fix Required:**
- Add env var to `user-install-fresh`: `CARGO_NET_OFFLINE=true` (after initial download)
- OR: Audit CI logs to verify network access patterns
- Validates dependency pinning actually prevents supply chain drift

**Priority:** 🟡 MEDIUM - Strengthens security claims

**Note:** May need two-phase approach:
1. Download phase (network enabled)
2. Build phase (network disabled, proves cache sufficiency)

---

### 3. Golden Test at Medium Scale ❌

**Current:**
- Quickstart examples: 10 rows (in CI via `user-smoke`)
- GLD_NUM full: 6826 rows (NOT in CI, too slow)
- GLD_NUM mini: 100 rows (exists, NOT in CI)

**Problem:** Gap between toy tests (10 rows) and production (6826 rows)

**Fix Required:**
- Add `user-smoke` step for `gld_num_mini.blisp`
- Validates: multi-column pipeline, complex ops (wzs, ur, mapr), NA handling
- Target: 100-2000 rows, <5 seconds

**Priority:** 🔴 HIGH - Catches planner/executor bugs

**Implementation:**
```yaml
- name: Test GLD_NUM mini (100 rows)
  run: |
    ./target/release/blisp run examples/gld_num_mini.blisp
    ./target/release/blisp verify gld_num_mini_output.csv expected/gld_num_mini.csv --tol 1e-6
```

---

### 4. Windows CI ❌

**Current:** Only `ubuntu-latest` tested

**Problem:** "Platform support" section claims Windows works, but untested

**Fix Required:**
- Add thin `windows-smoke` job:
  - `cargo test --locked --lib`
  - `cargo build --locked --release --bin blisp`
  - `./target/release/blisp.exe --selftest`
  - `./target/release/blisp.exe run examples/quickstart/hello.blisp`

**Priority:** 🟡 MEDIUM - Most user pain is Windows differences

**Rationale:** Catches path separators, newline (\r\n vs \n), filesystem quirks

**Note:** Windows runners are slower, so keep minimal (just smoke, not full suite)

---

### 5. Benchmark Gate (Prevent Regressions) ❌

**Current:**
- ✅ `benches/perf_guardrails.rs` exists (criterion benchmarks)
- ❌ NOT run in CI
- ❌ No baseline to compare against

**Problem:** "No performance regressions" claim is not enforced

**Fix Required:**
- Add `benchmark-gate` CI job:
  - Run: `cargo bench --bench perf_guardrails`
  - Store baseline (commit to repo or CI artifact)
  - Fail if >20% slower than baseline

**Priority:** 🟢 LOW - Nice to have, prevents silent decay

**Rationale:** IR optimizations can regress; catch early

**Note:** Criterion can save baselines to `target/criterion/`; need to cache or commit

---

## Implementation Plan

### Phase A: Critical Fixes (before claiming "production-ready")

1. **Install-from-tag** (30 minutes)
   - Add `user-install-from-tag` job to `.github/workflows/ci.yml`
   - Test `--tag v0.2.0` installation
   - Verify selftest + basic expression

2. **Medium golden test** (45 minutes)
   - Add `gld_num_mini.blisp` to `user-smoke` job
   - Create expected output: `expected/gld_num_mini.csv`
   - Add verification step with `--tol 1e-6`

### Phase B: Platform Hardening (before Windows users arrive)

3. **Windows CI** (1 hour)
   - Add `windows-smoke` job (minimal: test, build, selftest, hello.blisp)
   - Fix any path/newline issues that surface
   - Update INSTALL.md platform support table

### Phase C: Long-term Guardrails (prevent backsliding)

4. **Benchmark gate** (2 hours)
   - Store baseline: commit `target/criterion/` to `.baselines/`
   - Add `benchmark-gate` job: fail if >20% slower
   - Document in `docs/perf/README.md`

5. **No-network audit** (1 hour)
   - Add `CARGO_NET_OFFLINE=true` to build phase (after deps downloaded)
   - OR: Audit CI logs for network access patterns
   - Document in SEMANTICS.md or INSTALL.md

---

## Acceptance Criteria

**Before claiming "production-ready" publicly:**

- [x] Master CI green (8/8 jobs)
- [x] GitHub Release v0.2.0 exists
- [x] Selftests embedded (6/6 tripwires)
- [x] GLD_NUM verified locally
- [ ] **Install-from-tag tested in CI** (Phase A.1)
- [ ] **Medium golden test in CI** (Phase A.2)
- [ ] Windows CI passing (Phase B.3) OR platform support documented as "best effort, untested"

**Before promoting to external users:**

- [ ] All Phase A + B complete
- [ ] Benchmark gate (Phase C.4) OR document "no automated perf regression tests"
- [ ] No-network audit (Phase C.5) OR clarify "network needed for install, not runtime"

---

## Timeline Estimate

| Phase | Tasks | Time | Blocker? |
|-------|-------|------|----------|
| **A** | Install-from-tag + Medium golden test | 1.5 hours | 🔴 Yes - v0.2.0 legitimacy |
| **B** | Windows CI | 1 hour | 🟡 Depends on user base |
| **C** | Benchmark gate + No-network | 3 hours | 🟢 No - nice to have |

**Total:** 5.5 hours for full hardening

**Minimum viable:** Phase A only (1.5 hours) to defend v0.2.0 claims

---

## Risk Assessment

### What happens if we skip these?

| Gap | Risk if Skipped | Mitigation |
|-----|----------------|------------|
| Install-from-tag | Users install wrong commit (branch diverges) | Document: "use --tag v0.2.0" in INSTALL.md |
| Medium golden test | Bugs in complex pipelines slip through | Run `verify_gld_num.sh` manually before release |
| Windows CI | Windows users hit path/newline bugs | Document: "Windows support best-effort" |
| Benchmark gate | Performance regressions go unnoticed | Manual benchmarking before major releases |
| No-network | Supply chain claims not provable | Soften claim: "minimal dependencies, pinned" |

**Recommendation:** Do Phase A (1.5 hours) before external promotion. Phases B+C can wait.

---

## Next Actions

**User decision required:**

1. **Immediate:** Implement Phase A (install-from-tag + medium golden test)?
   - If YES: Start with install-from-tag CI job (30 min)
   - If NO: Document limitations in RELEASE_NOTES_v0.2.0.md

2. **Short-term:** Add Windows CI (Phase B)?
   - If YES: Expect 1-2 hours for setup + bug fixes
   - If NO: Update INSTALL.md to say "Windows: community-tested only"

3. **Long-term:** Benchmark gate (Phase C.4)?
   - If YES: Need baseline + CI job + docs (2 hours)
   - If NO: Document "manual perf testing before releases"

---

## Files to Create/Modify

**CI Changes:**
- `.github/workflows/ci.yml` (add jobs: install-from-tag, windows-smoke, benchmark-gate)

**Expected Outputs:**
- `expected/gld_num_mini.csv` (for medium golden test)

**Documentation:**
- `INSTALL.md` (clarify platform support, tag-based install)
- `docs/perf/README.md` (benchmark gate policy, if implemented)

**Baselines (if doing Phase C):**
- `.baselines/criterion/perf_guardrails/` (committed benchmark baselines)

---

**Status:** Ready for user decision on which phases to implement.
