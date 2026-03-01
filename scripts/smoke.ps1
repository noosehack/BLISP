# BLISP Smoke Test - Windows PowerShell
# Validates installation and basic functionality

$ErrorActionPreference = "Stop"

Write-Host "=== BLISP Smoke Test ===" -ForegroundColor Cyan
Write-Host ""

# Determine BLISP binary location
$BLISP = $null
if (Test-Path ".\target\release\blisp.exe") {
    $BLISP = ".\target\release\blisp.exe"
} elseif (Get-Command blisp -ErrorAction SilentlyContinue) {
    $BLISP = "blisp"
} else {
    Write-Host "❌ BLISP binary not found" -ForegroundColor Red
    Write-Host "   Please build with: cargo build --locked --release"
    exit 1
}

Write-Host "Using binary: $BLISP"
Write-Host ""

# 1. Build (if in source directory)
if (Test-Path "Cargo.toml") {
    Write-Host "[1/7] Building BLISP..."
    cargo build --locked --release --bin blisp
    Write-Host "      ✅ Build successful" -ForegroundColor Green
    Write-Host ""
}

# 2. Version check
Write-Host "[2/7] Testing --version flag..."
$VERSION = & $BLISP --version 2>&1 | Out-String
if ($VERSION -match "blisp v\d+\.\d+\.\d+") {
    Write-Host "      ✅ $VERSION" -ForegroundColor Green
} else {
    Write-Host "      ❌ Unexpected version output: $VERSION" -ForegroundColor Red
    exit 1
}
Write-Host ""

# 3. Self-test
Write-Host "[3/7] Testing --selftest flag..."
& $BLISP --selftest > $null 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "      ✅ Self-tests passed" -ForegroundColor Green
} else {
    Write-Host "      ❌ Self-tests failed" -ForegroundColor Red
    exit 1
}
Write-Host ""

# 4. Hello world
Write-Host "[4/7] Testing basic expression evaluation..."
$RESULT = (& $BLISP -e '(+ 1 2)' 2>&1 | Select-String -Pattern "^\d+$" | Select-Object -First 1).ToString()
if ($RESULT -eq "3") {
    Write-Host "      ✅ Expression evaluation works" -ForegroundColor Green
} else {
    Write-Host "      ❌ Expected '3', got '$RESULT'" -ForegroundColor Red
    exit 1
}
Write-Host ""

# 5. Quickstart examples (if available)
if (Test-Path "examples\quickstart") {
    Write-Host "[5/7] Testing quickstart examples..."
    & $BLISP run examples\quickstart\hello.blisp > $null 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "      ✅ hello.blisp runs successfully" -ForegroundColor Green
    } else {
        Write-Host "      ❌ hello.blisp failed" -ForegroundColor Red
        exit 1
    }

    & $BLISP run examples\quickstart\load_csv.blisp > $null 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "      ✅ load_csv.blisp runs successfully" -ForegroundColor Green
    } else {
        Write-Host "      ❌ load_csv.blisp failed" -ForegroundColor Red
        exit 1
    }
    Write-Host ""
} else {
    Write-Host "[5/7] Quickstart examples not found (skipping)"
    Write-Host ""
}

# 6. Verify subcommand
Write-Host "[6/7] Testing verify subcommand..."
# Create test CSVs
Set-Content -Path "$env:TEMP\blisp_test1.csv" -Value @"
a;b;c
1;2;3
4;5;6
"@

Set-Content -Path "$env:TEMP\blisp_test2.csv" -Value @"
a;b;c
1;2;3
4;5;6
"@

& $BLISP verify "$env:TEMP\blisp_test1.csv" "$env:TEMP\blisp_test2.csv" > $null 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "      ✅ Verify passes for matching CSVs" -ForegroundColor Green
} else {
    Write-Host "      ❌ Verify failed for matching CSVs" -ForegroundColor Red
    exit 1
}

# Test with mismatched CSVs (should fail)
Set-Content -Path "$env:TEMP\blisp_test3.csv" -Value @"
a;b;c
1;2;3
4;5.5;6
"@

& $BLISP verify "$env:TEMP\blisp_test3.csv" "$env:TEMP\blisp_test2.csv" > $null 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "      ✅ Verify correctly detects differences" -ForegroundColor Green
} else {
    Write-Host "      ❌ Verify should have failed for different CSVs" -ForegroundColor Red
    exit 1
}

# Cleanup
Remove-Item "$env:TEMP\blisp_test*.csv" -ErrorAction SilentlyContinue
Write-Host ""

# 7. Example verification (if available)
if ((Test-Path "examples\quickstart\load_csv.blisp") -and (Test-Path "expected\quickstart_load_csv.csv")) {
    Write-Host "[7/7] Testing example output verification..."
    & $BLISP run examples\quickstart\load_csv.blisp 2>&1 | Select-String -Pattern "Running in" -NotMatch | Out-File "$env:TEMP\blisp_output.csv"
    & $BLISP verify "$env:TEMP\blisp_output.csv" expected\quickstart_load_csv.csv --tol 1e-6 > $null 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "      ✅ Example output matches expected" -ForegroundColor Green
    } else {
        Write-Host "      ❌ Example output verification failed" -ForegroundColor Red
        exit 1
    }
    Remove-Item "$env:TEMP\blisp_output.csv" -ErrorAction SilentlyContinue
    Write-Host ""
} else {
    Write-Host "[7/7] Example verification data not found (skipping)"
    Write-Host ""
}

Write-Host "✅ All Smoke Tests PASSED" -ForegroundColor Green
Write-Host ""
