# Release gate for BLISP (PowerShell version)
# Run this before creating any release tag
# Exit code 0 = ready to release, non-zero = not ready

$ErrorActionPreference = "Stop"

Write-Host "=== BLISP Release Gate ===" -ForegroundColor Cyan
Write-Host ""

# Check we're on a clean commit
$gitStatus = git diff-index --quiet HEAD --
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ FAIL: Working directory has uncommitted changes" -ForegroundColor Red
    Write-Host "Commit or stash changes before releasing"
    exit 1
}
Write-Host "✅ Working directory is clean" -ForegroundColor Green

# Check blawktrust dependency is locked to a tag
Write-Host ""
Write-Host "Checking blawktrust dependency..."
$cargoContent = Get-Content Cargo.toml -Raw
if ($cargoContent -match 'blawktrust.*path.*=') {
    Write-Host "❌ FAIL: blawktrust is using path dependency" -ForegroundColor Red
    Write-Host "Change to: blawktrust = { git = `"...`", tag = `"v0.x.y`" }"
    exit 1
}

if ($cargoContent -match 'blawktrust.*git.*tag\s*=\s*"([^"]+)"') {
    $tag = $Matches[1]
    Write-Host "✅ blawktrust locked to tag: $tag" -ForegroundColor Green
} else {
    Write-Host "❌ FAIL: blawktrust dependency must be locked to a specific tag" -ForegroundColor Red
    Write-Host "Change to: blawktrust = { git = `"https://github.com/noosehack/blawktrust`", tag = `"v0.x.y`" }"
    exit 1
}

# Run formatting check
Write-Host ""
Write-Host "Running cargo fmt check..."
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ FAIL: Code is not formatted" -ForegroundColor Red
    Write-Host "Run: cargo fmt --all"
    exit 1
}
Write-Host "✅ Formatting check passed" -ForegroundColor Green

# Run clippy
Write-Host ""
Write-Host "Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ FAIL: Clippy found issues" -ForegroundColor Red
    exit 1
}
Write-Host "✅ Clippy check passed" -ForegroundColor Green

# Run lib tests
Write-Host ""
Write-Host "Running library tests..."
cargo test --lib
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ FAIL: Library tests failed" -ForegroundColor Red
    exit 1
}
Write-Host "✅ Library tests passed" -ForegroundColor Green

# Run critical integration test
Write-Host ""
Write-Host "Running blawktrust API integration test..."
cargo test --test blawktrust_api_integration
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ FAIL: blawktrust API integration test failed" -ForegroundColor Red
    Write-Host "This means blawktrust's API has changed in a way that breaks BLISP"
    exit 1
}
Write-Host "✅ Integration test passed" -ForegroundColor Green

# Build release binary
Write-Host ""
Write-Host "Building release binary..."
cargo build --release --bin blisp
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ FAIL: Release build failed" -ForegroundColor Red
    exit 1
}
Write-Host "✅ Release build succeeded" -ForegroundColor Green

# Run smoke test
Write-Host ""
Write-Host "Running smoke test..."
$smokeTest = @'
(defparameter df (stdin))
(print (sum df))
(print (sum (o 'Z df)))
(print (sum (o 'R df)))
'@
$smokeTest | Out-File -FilePath "$env:TEMP\blisp_smoke_test.lisp" -Encoding ASCII
"a;b`n1;2`n3;4" | .\target\release\blisp.exe "$env:TEMP\blisp_smoke_test.lisp" | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ FAIL: Smoke test failed" -ForegroundColor Red
    exit 1
}
Write-Host "✅ Smoke test passed" -ForegroundColor Green

# Check if tag already exists
if ($args.Count -gt 0) {
    $tagName = $args[0]
    git rev-parse $tagName 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Host ""
        Write-Host "⚠️  WARNING: Tag $tagName already exists" -ForegroundColor Yellow
        Write-Host "NEVER move existing tags! Create a new tag instead (e.g., $tagName-fixed or increment version)"
        exit 1
    }
    Write-Host "✅ Tag $tagName does not exist yet" -ForegroundColor Green
}

# All checks passed
Write-Host ""
Write-Host "=========================================" -ForegroundColor Green
Write-Host "✅ ALL CHECKS PASSED - READY TO RELEASE" -ForegroundColor Green
Write-Host "=========================================" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:"
Write-Host "  1. Create tag:  git tag v0.x.y"
Write-Host "  2. Push tag:    git push origin v0.x.y"
Write-Host "  3. NEVER move or delete tags once pushed"
Write-Host ""
Write-Host "Remember: Tags are immutable contracts!"

exit 0
