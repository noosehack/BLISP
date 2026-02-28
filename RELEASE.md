# Release Process

## Critical Rules

1. **Tags are immutable** - NEVER move or delete a pushed tag
2. **CI must pass** - Only tag commits that pass all CI checks
3. **blawktrust must be locked** - Dependency must use git tag, not path or branch

---

## Before Creating a Tag

### Run Release Gate

```bash
# Linux/macOS
./scripts/release_check.sh v0.2.0

# Windows
.\scripts\release_check.ps1 v0.2.0
```

This script verifies:
- ✅ Working directory is clean
- ✅ blawktrust dependency is locked to a specific tag
- ✅ Code is formatted (`cargo fmt`)
- ✅ No clippy warnings
- ✅ All tests pass (including integration test)
- ✅ Release build succeeds
- ✅ Smoke test passes (orientation system)
- ✅ Tag doesn't already exist

**The script MUST pass before creating any tag.**

---

## Dependency Locking

**CRITICAL**: BLISP's `Cargo.toml` must use a locked blawktrust dependency:

```toml
# ✅ CORRECT - Locked to specific tag
blawktrust = { git = "https://github.com/noosehack/blawktrust", tag = "v0.1.0-orientation-stable" }

# ❌ WRONG - Path dependency (changes without notice)
blawktrust = { path = "../blawktrust" }

# ❌ WRONG - Branch dependency (changes without notice)
blawktrust = { git = "...", branch = "main" }
```

The release gate script will reject path or branch dependencies.

---

## Creating a Release

1. **Update blawktrust dependency** to latest stable tag:
   ```toml
   blawktrust = { git = "https://github.com/noosehack/blawktrust", tag = "v0.2.0" }
   ```

2. **Test locally**:
   ```bash
   cargo test --test blawktrust_api_integration
   ```

3. **Ensure CI is green** on GitHub

4. **Run release gate**:
   ```bash
   ./scripts/release_check.sh v0.2.0
   ```

5. **Create and push tag** (only if gate passed):
   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

---

## Integration Test (Critical Safety Net)

`tests/blawktrust_api_integration.rs` is the **critical tripwire** that prevents API breaks.

This test would have caught the 2026-02-28 incident where blawktrust removed `Column::Date` and `Column::Timestamp`, causing 48 compile errors in BLISP.

The test verifies:
- All Column types exist (`Date`, `Timestamp`, `F64`, `Ts`)
- All constructors exist (`new_date`, `new_timestamp`, etc.)
- All NULL sentinels are exported
- Pattern matching works
- Orientation system works (H, Z, R)
- TableView and operations exist

**If this test fails, DO NOT release until blawktrust is fixed or BLISP is updated.**

---

## Semantic Versioning

- **Major (1.0.0 → 2.0.0)**: Incompatible with previous blawktrust API
- **Minor (1.0.0 → 1.1.0)**: New features, backward compatible
- **Patch (1.0.0 → 1.0.1)**: Bug fixes

---

## What to Do If a Tag is Broken

**NEVER move or delete the tag.** Instead:

1. **Fix the issue** in a new commit
2. **Create a new tag**: `v0.2.1` or `v0.2.0-fixed`
3. **Document** what was fixed

---

## CI Workflow

GitHub Actions runs on every push and PR:

1. **Format check** - `cargo fmt --check`
2. **Clippy** - `cargo clippy -- -D warnings`
3. **Lib tests** - `cargo test --lib`
4. **Integration test** - `cargo test --test blawktrust_api_integration` (CRITICAL)
5. **Build** - `cargo build --release`
6. **Smoke test** - Test orientation system works

All must pass before merging.

---

## Updating blawktrust Dependency

When blawktrust releases a new version:

1. **Check blawktrust CHANGELOG** for breaking changes
2. **Update Cargo.toml** to new tag
3. **Run integration test**:
   ```bash
   cargo test --test blawktrust_api_integration
   ```
4. **If test fails**:
   - blawktrust introduced breaking changes
   - Either update BLISP code or downgrade blawktrust
5. **Run full release gate** before tagging

---

## Checklist

Before pushing a tag:

- [ ] blawktrust dependency uses `tag = "vX.Y.Z"` (not path/branch)
- [ ] `cargo test --test blawktrust_api_integration` passes
- [ ] CI is green on GitHub Actions
- [ ] `./scripts/release_check.sh` passes
- [ ] Smoke test passes
- [ ] Tag doesn't already exist

---

## Emergency: Yanking a Release

If a critical bug is found after release:

1. **DO NOT** delete the tag
2. **Create hotfix** in new commit
3. **Tag hotfix**: `v0.2.1` (patch bump)
4. **Announce** in CHANGELOG

---

## Incident Report: 2026-02-28

blawktrust removed `Column::Date` and `Column::Timestamp` without updating BLISP, causing:
- 48 compile errors
- Broken v0.1.0-orientation-stable tag
- Emergency hotfix required

**Root cause**: No integration test, no CI checking BLISP compilation.

**Prevention**: The `blawktrust_api_integration.rs` test was created to ensure this never happens again.

---

## Questions?

- Check CI logs: https://github.com/noosehack/BLISP/actions
- Review blawktrust API: `/tests/blawktrust_api_integration.rs`
- See incident report: `/OPTION_A_COMPLETE.md`
