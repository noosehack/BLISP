# BLISP v0.2.0 — Installation from Scratch on Linux

**Audience:** You have a fresh or existing Linux server (Ubuntu 20.04+, Debian 11+, RHEL 8+, or similar) with shell access. This guide takes you from zero to a verified BLISP installation.

**Time required:** 5–10 minutes (depending on network speed).

---

## Table of Contents

1. [Prerequisites — System Packages](#1-prerequisites--system-packages)
2. [Install the Rust Toolchain](#2-install-the-rust-toolchain)
3. [Clone the Repository](#3-clone-the-repository)
4. [Build the Release Binary](#4-build-the-release-binary)
5. [Verify the Installation](#5-verify-the-installation)
6. [Install to PATH (Optional)](#6-install-to-path-optional)
7. [Run the Full Test Suite (Optional)](#7-run-the-full-test-suite-optional)
8. [Run the Smoke Test (Optional)](#8-run-the-smoke-test-optional)
9. [Troubleshooting](#9-troubleshooting)
10. [Uninstall](#10-uninstall)

---

## 1. Prerequisites — System Packages

Cargo compiles Rust code and its dependencies from source. It needs a C linker, OpenSSL headers (for HTTPS registry access), and `pkg-config`. Git is required to clone the repo and to fetch the `blawktrust` dependency.

### Ubuntu / Debian

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev git curl
```

### RHEL / CentOS / Rocky / AlmaLinux

```bash
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y pkg-config openssl-devel git curl
```

### Amazon Linux 2

```bash
sudo yum groupinstall -y "Development Tools"
sudo yum install -y pkgconfig openssl-devel git curl
```

### Verification

Run all four of these. Every one must succeed:

```bash
gcc --version       # Must print a version (any version)
pkg-config --version
openssl version
git --version       # Must be >= 2.0
```

If any command is missing, go back and install the corresponding package.

---

## 2. Install the Rust Toolchain

BLISP pins Rust **1.93.1** via `rust-toolchain.toml`. Rustup will install this exact version automatically when you first build inside the repo.

### Install rustup (the Rust version manager)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

### Load cargo into your current shell

```bash
source "$HOME/.cargo/env"
```

> **Persistence:** The installer adds a line to `~/.bashrc` (or `~/.profile`). If you open a new shell later, cargo will already be on your PATH. If you use `zsh`, add `source "$HOME/.cargo/env"` to `~/.zshrc` manually.

### Verification

```bash
rustup --version    # e.g. rustup 1.28.x
cargo --version     # e.g. cargo 1.93.x (exact version will be installed in step 4)
```

Both commands must print a version. If `cargo` is not found, run `source "$HOME/.cargo/env"` again.

---

## 3. Clone the Repository

Clone the tagged v0.2.0 release directly:

```bash
git clone --branch v0.2.0 https://github.com/noosehack/BLISP.git blisp
cd blisp
```

### Verification

```bash
git describe --tags --exact-match
```

**Expected output:**

```
v0.2.0
```

If you see `fatal: no tag exactly matches`, you are not on the tagged commit. Run:

```bash
git checkout v0.2.0
```

### SSH alternative

If you prefer SSH (requires your SSH key to be registered on GitHub):

```bash
git clone --branch v0.2.0 git@github.com:noosehack/BLISP.git blisp
cd blisp
```

---

## 4. Build the Release Binary

```bash
cargo build --locked --release
```

### What this does

1. Rustup detects `rust-toolchain.toml` and installs Rust 1.93.1 (with `rustfmt` and `clippy` components). This happens automatically on first build — you do not need to install 1.93.1 manually.
2. Cargo fetches all dependencies from crates.io and the `blawktrust` git dependency from GitHub.
3. Cargo compiles everything in release (optimized) mode.
4. The binary lands at `./target/release/blisp`.

### Flags explained

| Flag | Purpose |
|------|---------|
| `--locked` | Enforces the committed `Cargo.lock`. Guarantees bit-for-bit reproducible dependency resolution. **Always use this flag.** |
| `--release` | Enables compiler optimizations. Without it you get an unoptimized debug build. |

### Expected timing

| Scenario | Time |
|----------|------|
| First build (cold, downloads + compiles everything) | 30–90 seconds |
| Incremental rebuild (source changed) | 2–5 seconds |

### Verification

```bash
ls -lh target/release/blisp
```

Must show a file (typically 10–30 MB). If the file does not exist, the build failed — check the error output.

---

## 5. Verify the Installation

Run these three checks in order. All three must pass.

### 5a. Version check

```bash
./target/release/blisp --version
```

**Expected output:**

```
blisp v0.2.0
```

### 5b. Self-tests (6 embedded tripwire tests)

```bash
./target/release/blisp --selftest
```

**Expected output:**

```
Running BLISP self-tests...

  [1/6] IEEE: ln(0) = -inf ... ✅
  [2/6] IEEE: 0/0 = NaN ... ✅
  [3/6] IEEE: Fusion preserves edge cases ... ✅
  [4/6] Orientation: H vs Z different shapes ... ✅
  [5/6] Mask: Weekend detection ... ✅
  [6/6] Platform: f64 size check ... ✅

=== Self-Test Results ===
Total:  6
Passed: 6
Failed: 0

✅ All self-tests PASSED
```

**If any test fails: STOP.** The binary is not safe to use. See [Troubleshooting](#9-troubleshooting).

### 5c. Smoke expression

```bash
./target/release/blisp -e '(+ 1 2)'
```

**Expected output** (the first line may vary):

```
✅ Running in HYBRID mode (IR for Frame ops, legacy fallback)
3
```

The important thing is that the last line says `3`.

---

## 6. Install to PATH (Optional)

If you want `blisp` available as a command from any directory:

```bash
cargo install --locked --path .
```

This copies the binary to `~/.cargo/bin/blisp`. Verify:

```bash
which blisp         # Should print ~/.cargo/bin/blisp
blisp --version     # Should print blisp v0.2.0
```

> **If `which blisp` returns nothing:** `~/.cargo/bin` is not on your PATH. Add it:
>
> ```bash
> echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
> source ~/.bashrc
> ```

### Alternative: manual symlink

If you prefer not to use `cargo install`:

```bash
sudo ln -sf "$(pwd)/target/release/blisp" /usr/local/bin/blisp
```

---

## 7. Run the Full Test Suite (Optional)

```bash
cargo test --locked
```

**Expected result:**

- All tests pass
- Exactly **15 tests ignored** (these are documented and intentional — see `IGNORED_TESTS.md`)

Verify the ignored count:

```bash
cargo test --locked 2>&1 | grep "ignored"
```

You should see `15 ignored` in the output. If the number is higher, something is wrong.

---

## 8. Run the Smoke Test (Optional)

The repository includes an automated smoke script that validates 7 aspects of the installation:

```bash
./scripts/smoke.sh
```

**Expected:** All 7 checks pass. If the script is not executable:

```bash
chmod +x scripts/smoke.sh
./scripts/smoke.sh
```

---

## 9. Troubleshooting

### `error: linker 'cc' not found`

**Cause:** No C compiler installed.

**Fix:**

```bash
# Ubuntu/Debian
sudo apt install -y build-essential

# RHEL/CentOS
sudo dnf groupinstall -y "Development Tools"
```

### `error: failed to run custom build command for 'openssl-sys'`

**Cause:** Missing OpenSSL development headers. Cargo needs these to compile the `openssl-sys` crate (used for HTTPS registry access).

**Fix:**

```bash
# Ubuntu/Debian
sudo apt install -y libssl-dev pkg-config

# RHEL/CentOS
sudo dnf install -y openssl-devel pkg-config
```

### `error: the lock file needs to be updated but --locked was passed`

**Cause:** The `Cargo.lock` file is out of sync. This should not happen on the tagged release.

**Fix:** Make sure you cloned the exact tag:

```bash
git checkout v0.2.0
cargo build --locked --release
```

### `error[E0658]: use of unstable feature` or other compiler errors

**Cause:** Wrong Rust version. The project requires exactly 1.93.1.

**Fix:** Ensure `rust-toolchain.toml` is present and let rustup handle it:

```bash
cat rust-toolchain.toml       # Should show channel = "1.93.1"
rustup show                   # Should show 1.93.1 as active toolchain
```

If the wrong version is active:

```bash
rustup install 1.93.1
rustup override set 1.93.1
```

### `error: failed to fetch` or network timeout during build

**Cause:** Cargo cannot reach crates.io or GitHub (for the `blawktrust` git dependency).

**Fix:**

1. Check internet access: `curl -I https://crates.io`
2. Check GitHub access: `curl -I https://github.com`
3. If behind a proxy, set:
   ```bash
   export HTTPS_PROXY=http://your-proxy:port
   export HTTP_PROXY=http://your-proxy:port
   ```
4. If GitHub SSH is blocked, ensure the clone used HTTPS (not SSH).

### `Permission denied` when running the binary

**Fix:**

```bash
chmod +x target/release/blisp
```

### Self-test fails

**This is a critical failure.** Do not use the binary. Steps:

1. Confirm you are on the correct tag: `git describe --tags --exact-match` should say `v0.2.0`.
2. Rebuild from clean: `cargo clean && cargo build --locked --release`
3. Run self-tests again: `./target/release/blisp --selftest`
4. If it still fails, report the full output.

### `cargo: command not found` after closing and reopening shell

**Fix:** The Rust environment was not loaded. Add to your shell profile:

```bash
# For bash
echo 'source "$HOME/.cargo/env"' >> ~/.bashrc
source ~/.bashrc

# For zsh
echo 'source "$HOME/.cargo/env"' >> ~/.zshrc
source ~/.zshrc
```

### Extremely slow build (>5 minutes)

**Possible causes:**

- Low memory (<1 GB free). Rust compilation is memory-intensive. Ensure at least 1 GB free RAM, or add swap:
  ```bash
  sudo fallocate -l 2G /swapfile
  sudo chmod 600 /swapfile
  sudo mkswap /swapfile
  sudo swapon /swapfile
  ```
- Single-core machine. Cargo compiles in parallel by default. On a 1-vCPU server, expect longer builds.
- Spinning disk I/O. SSD is strongly recommended.

### `git clone` fails with `Repository not found`

**Fix:** The repository is public. Verify the URL is correct:

```bash
git clone https://github.com/noosehack/BLISP.git blisp
```

Note: the repo name is uppercase `BLISP`.

---

## 10. Uninstall

### Remove the binary from PATH

```bash
cargo uninstall blisp        # Removes ~/.cargo/bin/blisp
```

Or if you used a symlink:

```bash
sudo rm /usr/local/bin/blisp
```

### Remove the source directory

```bash
rm -rf /path/to/blisp
```

### Remove Rust entirely (optional)

```bash
rustup self uninstall
```

This removes `~/.cargo` and `~/.rustup` entirely.

---

## Automated Installer (Recommended)

The `install.sh` script automates everything — system packages, Rust, clone, build, verify:

```bash
# Fresh install (latest release):
curl -sSf https://raw.githubusercontent.com/noosehack/BLISP/master/install.sh | bash

# Fresh install (specific version):
curl -sSf https://raw.githubusercontent.com/noosehack/BLISP/master/install.sh | bash -s -- --version v0.2.0

# Upgrade existing installation to latest:
cd ~/blisp && bash install.sh --upgrade

# Upgrade to specific version:
cd ~/blisp && bash install.sh --upgrade --version v0.3.0

# Rollback to previous version:
cd ~/blisp && bash install.sh --upgrade --version v0.2.0
```

The installer auto-detects your OS, handles upgrades with rollback on failure, and verifies every step.

---

## Quick Reference — Manual Copy-Paste Block

For the impatient who prefer doing it by hand. Run this on a fresh Ubuntu server:

```bash
# System packages
sudo apt update && sudo apt install -y build-essential pkg-config libssl-dev git curl

# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# Clone and build
git clone --branch v0.2.0 https://github.com/noosehack/BLISP.git blisp
cd blisp
cargo build --locked --release

# Verify
./target/release/blisp --version
./target/release/blisp --selftest
./target/release/blisp -e '(+ 1 2)'
```

**All three verify commands must succeed.** If they do, BLISP is correctly installed.
