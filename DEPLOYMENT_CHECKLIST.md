# BLISP v0.2.0 Deployment Checklist

**Status:** Ready for CI validation and deployment
**Current Branch:** `reconstruct/tableview-only`
**Target:** Merge to `master`, tag `v0.2.0`, create GitHub Release

---

## What We've Built

### ✅ Phase 1: User-Facing Product (Complete)
- CLI enhancements (--version, --help, subcommands)
- Embedded self-tests (6 tripwire tests, <1s)
- CSV verification (IEEE-754 aware)
- Bundled examples + data
- Smoke test scripts (Linux/macOS/Windows)
- Documentation (README, INSTALL, SEMANTICS)

### ✅ Phase 2: Zero-State Proof (Complete)
- CI job: `user-install-fresh`
- Tests `cargo install --git` from clean environment
- No workspace, no checkout
- Validates: --version, --selftest, verify, expressions

### ✅ Phase 3: Semantic Contract (Complete)
- `SEMANTICS.md` documents all verification rules
- IEEE-754 edge cases linked to tripwire tests
- Fusion correctness guarantees
- NA propagation policies
- Verification semantics (NaN==NaN, inf==inf)

### ✅ Phase 4: Security & Platform Docs (Complete)
- Dependency pinning enforced
- No network access during execution
- Memory safety guarantees
- Platform support status (Linux ✅, macOS ✅, Windows ⚠️)

---

## Pre-Deployment Verification

### Must Complete Before Merge

1. **Push to GitHub:**
   ```bash
   cd /home/ubuntu/blisp
   git push origin reconstruct/tableview-only
   ```

2. **Wait for CI to pass:**
   - Check: https://github.com/noosehack/BLISP/actions
   - **CRITICAL:** `user-install-fresh` job must pass
   - Proves `cargo install --git` works from scratch

3. **Verify all jobs green:**
   - [ ] fmt
   - [ ] clippy
   - [ ] test
   - [ ] build
   - [ ] smoke-test
   - [ ] user-smoke
   - [ ] **user-install-fresh** ← Zero-state proof
   - [ ] check-ignored-tests
   - [ ] ci-success

---

## Deployment Steps (After CI Green)

### Step 1: Merge to Master
```bash
cd /home/ubuntu/blisp
git checkout master
git pull origin master
git merge reconstruct/tableview-only
git push origin master
```

### Step 2: Create Tag
```bash
git tag -a v0.2.0 -m "BLISP v0.2.0 - User-Facing Product Release

First production-ready user-facing release with:
- Embedded self-tests (6 tripwire tests)
- CSV verification (IEEE-754 aware)
- Zero-state installation proof (CI)
- Semantic guarantees (SEMANTICS.md)
- Bundled examples and smoke tests

Install: cargo install blisp --git https://github.com/noosehack/BLISP --locked
Verify: blisp --selftest

See RELEASE_NOTES_v0.2.0.md for full details."

git push origin v0.2.0
```

### Step 3: Create GitHub Release
1. Go to: https://github.com/noosehack/BLISP/releases/new
2. Tag: `v0.2.0`
3. Title: `BLISP v0.2.0 - User-Facing Product Release`
4. Description: Copy from `RELEASE_NOTES_v0.2.0.md`
5. Key points to highlight:
   ```markdown
   ## Quick Install
   ```bash
   cargo install blisp --git https://github.com/noosehack/BLISP --locked
   blisp --selftest
   ```

   ## New Features
   - ✅ Embedded self-tests (6/6 tripwire tests, <1s)
   - ✅ CSV verification: `NaN==NaN`, `inf==inf`, `--tol` configurable
   - ✅ Bundled examples (quickstart + golden tests)
   - ✅ Zero-state proof: CI validates clean install

   ## Semantic Guarantees
   - IEEE-754 edge cases: `ln(0)=-inf`, `0/0=NaN`
   - Fusion correctness: bitwise identical for elementwise
   - Verification: NaN-aware, tolerance-based
   - See [SEMANTICS.md](SEMANTICS.md)

   ## Security
   - Dependency pinning (Cargo.lock, --locked)
   - No network access during execution
   - Memory-safe (Rust, no unsafe blocks)

   ## Platform Support
   - Linux ✅ Fully tested
   - macOS ✅ Supported
   - Windows ⚠️ Best effort (PowerShell script provided)
   ```

6. Attach: None (cargo install from git works)
7. Mark as: **Latest release** ✅
8. Publish release

---

## Post-Deployment Validation

### Verify Release Works
```bash
# Clean environment test (local)
cd /tmp
cargo install blisp --git https://github.com/noosehack/BLISP --locked --force
blisp --version     # Should show "blisp v0.2.0"
blisp --selftest    # Should pass 6/6 tests
```

### Verify CI on Master
- Check: https://github.com/noosehack/BLISP/actions
- Ensure all jobs pass on master after merge
- `user-install-fresh` should pass again on master

---

## Known Gaps (Post-Release TODO)

### 1. Medium Golden Test (Not Blocking)
**Status:** Deferred to v0.2.1
**Need:** 2-10k row dataset with:
- Weekend masks
- Shifts
- dlog_obs + rolling mean + zscore
- Multiple columns
- Join/time alignment
- Expected output committed

**Why deferred:** Quickstart (10 rows) + mini (100 rows) + selftest (embedded) provide adequate coverage for v0.2.0.

### 2. Windows CI Validation (Not Blocking)
**Status:** Best effort
**Need:** Test `scripts/smoke.ps1` on GitHub Actions `windows-latest`
**Current:** PowerShell script provided but not CI-tested

**Action:** Add this to CI if Windows users report issues:
```yaml
windows-smoke:
  runs-on: windows-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - name: Build and test
      run: |
        cargo build --locked --release --bin blisp
        .\scripts\smoke.ps1
```

### 3. Example Bundling in cargo install (Not Blocking)
**Status:** Works as designed
**Current:** Examples require `git clone`
**Alternative:** Use cargo-binstall or release artifacts (future enhancement)

**Workaround documented:** Users clone repo for examples, but selftest works without repo.

---

## Success Criteria (Must Pass)

- ✅ CI passes on `reconstruct/tableview-only` (especially `user-install-fresh`)
- ✅ Merge to master succeeds
- ✅ CI passes on master
- ✅ Tag `v0.2.0` created
- ✅ GitHub Release published
- ✅ Clean install test works: `cargo install --git`
- ✅ Selftest passes: `blisp --selftest` (6/6)
- ✅ Documentation accurate (README, INSTALL, SEMANTICS)

---

## Risk Assessment

### Low Risk
- ✅ No breaking changes (fully backward compatible)
- ✅ All new features are additive
- ✅ Existing workflows continue to work
- ✅ CI validates clean installation

### Medium Risk
- ⚠️ Windows support untested in CI
  - **Mitigation:** Documented as best-effort, PowerShell script provided
- ⚠️ Examples not bundled in `cargo install`
  - **Mitigation:** Documented clearly, selftest works without examples

### Blockers
- ❌ CI must pass (especially `user-install-fresh`) before merge
- ❌ Cannot claim "production-ready" without zero-state proof

---

## Communication Plan

### Announcement (Post-Release)
```markdown
🎉 BLISP v0.2.0 Released: User-Facing Product

BLISP is now a production-ready tool for high-performance columnar operations.

Quick install:
  cargo install blisp --git https://github.com/noosehack/BLISP --locked

Validate:
  blisp --selftest

New features:
- ✅ Embedded self-tests (IEEE-754, orientation, masks)
- ✅ CSV verification (NaN-aware, tolerance-based)
- ✅ Bundled examples (quickstart + golden tests)
- ✅ Zero-state proof (CI validates clean install)

Semantic guarantees:
- ln(0) = -inf (not NaN)
- NaN == NaN (bitwise)
- Fusion correctness enforced
- See SEMANTICS.md for full contract

Security:
- No network access during execution
- Dependency pinning enforced
- Memory-safe (Rust)

Platform support:
- Linux ✅ Fully tested
- macOS ✅ Supported
- Windows ⚠️ Best effort

Release notes: https://github.com/noosehack/BLISP/releases/tag/v0.2.0
```

---

## Rollback Plan (If Needed)

If critical issues found post-release:

1. **Revert tag:**
   ```bash
   git tag -d v0.2.0
   git push origin :refs/tags/v0.2.0
   ```

2. **Delete GitHub Release**

3. **Revert master merge:**
   ```bash
   git checkout master
   git revert <merge-commit>
   git push origin master
   ```

4. **Fix issue on branch**

5. **Re-release as v0.2.1**

---

## Final Checklist

**Before pushing to GitHub:**
- [x] All commits made
- [x] RELEASE_NOTES_v0.2.0.md complete
- [x] SEMANTICS.md complete
- [x] DEPLOYMENT_CHECKLIST.md (this file) complete
- [ ] Push to origin

**Before merging to master:**
- [ ] CI green on `reconstruct/tableview-only`
- [ ] `user-install-fresh` passed (zero-state proof)
- [ ] All 8 CI jobs passed

**After merge to master:**
- [ ] CI green on master
- [ ] Tag v0.2.0 created
- [ ] GitHub Release published
- [ ] Clean install test validated locally

**Post-release:**
- [ ] Announcement made (if applicable)
- [ ] Monitor for issues
- [ ] Plan v0.2.1 enhancements (medium golden test, Windows CI)

---

## Current Status

**✅ Ready for:** Push to GitHub and CI validation
**⬜ Waiting for:** CI to pass (especially `user-install-fresh`)
**⬜ Next step:** `git push origin reconstruct/tableview-only`
