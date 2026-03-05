#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
# BLISP Installer & Upgrader
# ═══════════════════════════════════════════════════════════════════════════
#
# Fresh install:
#   bash install.sh
#   bash install.sh --version v0.3.0
#   bash install.sh --prefix /opt/blisp
#
# Upgrade existing install:
#   bash install.sh --upgrade
#   bash install.sh --upgrade --version v0.3.0
#
# One-liner (fresh install, latest release):
#   curl -sSf https://raw.githubusercontent.com/noosehack/BLISP/master/install.sh | bash
#
# One-liner (specific version):
#   curl -sSf https://raw.githubusercontent.com/noosehack/BLISP/master/install.sh | bash -s -- --version v0.3.0
#
# ═══════════════════════════════════════════════════════════════════════════
set -euo pipefail

# ── Defaults ─────────────────────────────────────────────────────────────────
BLISP_REPO="https://github.com/noosehack/BLISP.git"
INSTALL_DIR="${HOME}/blisp"
TARGET_VERSION=""          # empty = auto-detect latest
UPGRADE_MODE=false
BOLD='\033[1m'
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# ── Parse arguments ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --prefix)
            INSTALL_DIR="$2"
            shift 2
            ;;
        --version)
            TARGET_VERSION="$2"
            # Normalize: ensure leading "v"
            if [[ "$TARGET_VERSION" != v* ]]; then
                TARGET_VERSION="v${TARGET_VERSION}"
            fi
            shift 2
            ;;
        --upgrade)
            UPGRADE_MODE=true
            shift
            ;;
        --help|-h)
            cat <<'HELPEOF'
BLISP Installer & Upgrader

Usage: bash install.sh [OPTIONS]

Options:
  --prefix DIR      Install/find source in DIR (default: ~/blisp)
  --version VER     Target a specific release tag (e.g. v0.3.0)
                    If omitted, installs the latest tagged release
  --upgrade         Upgrade an existing installation in-place
  --help            Show this help

Examples:
  bash install.sh                          # Fresh install, latest release
  bash install.sh --version v0.2.0         # Fresh install, specific version
  bash install.sh --upgrade                # Upgrade to latest release
  bash install.sh --upgrade --version v0.3.0  # Upgrade to specific version
HELPEOF
            exit 0
            ;;
        *)
            echo "Unknown option: $1 (try --help)"
            exit 1
            ;;
    esac
done

# ── Helpers ──────────────────────────────────────────────────────────────────
info()  { echo -e "${BOLD}[INFO]${NC}  $*"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
fail()  { echo -e "${RED}[FAIL]${NC}  $*"; exit 1; }
step()  { echo -e "${CYAN}[STEP]${NC}  $*"; }

check_cmd() {
    command -v "$1" &>/dev/null
}

# ── Resolve latest tag from remote ──────────────────────────────────────────
resolve_latest_tag() {
    # If we have a local clone, use it (faster)
    if [[ -d "$1/.git" ]]; then
        (cd "$1" && git fetch --tags --quiet 2>/dev/null)
        (cd "$1" && git tag --sort=-version:refname | grep '^v[0-9]' | head -1)
    else
        # Query remote without cloning
        git ls-remote --tags --sort=-version:refname "$BLISP_REPO" 'v*' 2>/dev/null \
            | awk -F/ '{print $NF}' \
            | grep '^v[0-9]' \
            | grep -v '\^{}' \
            | head -1
    fi
}

# Verify a tag exists on the remote
verify_tag_exists() {
    local tag="$1"
    local found
    if [[ -d "$INSTALL_DIR/.git" ]]; then
        (cd "$INSTALL_DIR" && git fetch --tags --quiet 2>/dev/null)
        found=$(cd "$INSTALL_DIR" && git tag -l "$tag")
    else
        found=$(git ls-remote --tags "$BLISP_REPO" "refs/tags/${tag}" 2>/dev/null | awk '{print $2}')
    fi
    [[ -n "$found" ]]
}

# Get the currently installed version from existing clone
get_current_version() {
    if [[ -d "$INSTALL_DIR/.git" ]]; then
        (cd "$INSTALL_DIR" && git describe --tags --exact-match 2>/dev/null || echo "unknown")
    else
        echo "none"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════
# Banner
# ═══════════════════════════════════════════════════════════════════════════
echo ""
echo -e "${BOLD}═══════════════════════════════════════════════════════${NC}"
if [[ "$UPGRADE_MODE" == true ]]; then
    echo -e "${BOLD}  BLISP Upgrader${NC}"
else
    echo -e "${BOLD}  BLISP Installer${NC}"
fi
echo -e "${BOLD}═══════════════════════════════════════════════════════${NC}"
echo ""

# ═══════════════════════════════════════════════════════════════════════════
# Detect mode: fresh install vs upgrade
# ═══════════════════════════════════════════════════════════════════════════
CURRENT_VERSION="none"
IS_FRESH=true

if [[ -d "$INSTALL_DIR/.git" ]]; then
    CURRENT_VERSION=$(get_current_version)
    IS_FRESH=false
fi

if [[ "$UPGRADE_MODE" == true ]] && [[ "$IS_FRESH" == true ]]; then
    fail "No existing BLISP installation found at ${INSTALL_DIR}. Run without --upgrade for a fresh install."
fi

if [[ "$UPGRADE_MODE" == false ]] && [[ "$IS_FRESH" == false ]]; then
    info "Existing installation detected at ${INSTALL_DIR} (${CURRENT_VERSION})"
    info "Switching to upgrade mode automatically"
    UPGRADE_MODE=true
fi

# ═══════════════════════════════════════════════════════════════════════════
# Resolve target version
# ═══════════════════════════════════════════════════════════════════════════
if [[ -z "$TARGET_VERSION" ]]; then
    step "Resolving latest release tag..."
    TARGET_VERSION=$(resolve_latest_tag "$INSTALL_DIR")
    if [[ -z "$TARGET_VERSION" ]]; then
        fail "Could not determine latest release. Use --version to specify one explicitly."
    fi
    ok "Latest release: ${TARGET_VERSION}"
else
    step "Verifying tag ${TARGET_VERSION} exists..."
    if ! verify_tag_exists "$TARGET_VERSION"; then
        fail "Tag '${TARGET_VERSION}' not found in ${BLISP_REPO}"
    fi
    ok "Tag ${TARGET_VERSION} found"
fi

# ── Already up to date? ─────────────────────────────────────────────────────
if [[ "$UPGRADE_MODE" == true ]] && [[ "$CURRENT_VERSION" == "$TARGET_VERSION" ]]; then
    ok "Already at ${TARGET_VERSION} — nothing to do"
    echo ""
    echo "  Current binary: ${INSTALL_DIR}/target/release/blisp"
    if check_cmd blisp; then
        echo "  Command:        blisp ($(which blisp))"
    fi
    echo ""
    exit 0
fi

if [[ "$UPGRADE_MODE" == true ]]; then
    info "Upgrading: ${CURRENT_VERSION} -> ${TARGET_VERSION}"
else
    info "Installing: ${TARGET_VERSION}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════════════════
# Step 1: Platform detection
# ═══════════════════════════════════════════════════════════════════════════
OS="unknown"
PKG_MANAGER="unknown"

if [[ -f /etc/os-release ]]; then
    . /etc/os-release
    case "$ID" in
        ubuntu|debian|pop|linuxmint|elementary)
            OS="debian"; PKG_MANAGER="apt" ;;
        rhel|centos|rocky|almalinux|ol)
            OS="rhel"; PKG_MANAGER="dnf"
            check_cmd dnf || PKG_MANAGER="yum"
            ;;
        fedora)
            OS="rhel"; PKG_MANAGER="dnf" ;;
        amzn)
            OS="rhel"
            [[ "${VERSION_ID:-}" == "2" ]] && PKG_MANAGER="yum" || PKG_MANAGER="dnf"
            ;;
        arch|manjaro)
            OS="arch"; PKG_MANAGER="pacman" ;;
        opensuse*|sles)
            OS="suse"; PKG_MANAGER="zypper" ;;
        *)
            OS="$ID" ;;
    esac
fi

if [[ "$OS" == "unknown" ]]; then
    if [[ "$(uname -s)" == "Darwin" ]]; then
        OS="macos"; PKG_MANAGER="brew"
    else
        fail "Cannot detect OS. Install manually — see INSTALL_FROM_SCRATCH.md"
    fi
fi

info "Detected OS: ${ID:-$OS} (package manager: ${PKG_MANAGER})"

# ═══════════════════════════════════════════════════════════════════════════
# Step 2: System dependencies
# ═══════════════════════════════════════════════════════════════════════════
step "Checking system dependencies..."

DEPS_MISSING=false
for cmd in gcc pkg-config git curl; do
    if ! check_cmd "$cmd"; then
        DEPS_MISSING=true
        break
    fi
done
if ! pkg-config --exists openssl 2>/dev/null; then
    DEPS_MISSING=true
fi

if [[ "$DEPS_MISSING" == true ]]; then
    info "Installing missing system packages..."

    case "$PKG_MANAGER" in
        apt)
            sudo apt-get update -qq
            sudo apt-get install -y -qq build-essential pkg-config libssl-dev git curl
            ;;
        dnf)
            sudo dnf groupinstall -y "Development Tools"
            sudo dnf install -y pkg-config openssl-devel git curl
            ;;
        yum)
            sudo yum groupinstall -y "Development Tools"
            sudo yum install -y pkgconfig openssl-devel git curl
            ;;
        pacman)
            sudo pacman -Sy --noconfirm --needed base-devel openssl pkg-config git curl
            ;;
        zypper)
            sudo zypper install -y -t pattern devel_C_C++
            sudo zypper install -y libopenssl-devel pkg-config git curl
            ;;
        brew)
            if ! xcode-select -p &>/dev/null; then
                info "Installing Xcode Command Line Tools..."
                xcode-select --install
                echo "Press Enter after Xcode CLI tools finish installing..."
                read -r
            fi
            check_cmd brew || fail "Homebrew not found. Install from https://brew.sh"
            brew install openssl pkg-config
            ;;
        *)
            fail "Unsupported package manager: ${PKG_MANAGER}. Install manually: gcc, pkg-config, openssl-dev, git, curl"
            ;;
    esac
    ok "System dependencies installed"
else
    ok "All system dependencies present"
fi

for cmd in gcc git curl; do
    check_cmd "$cmd" || fail "'${cmd}' still not found after install attempt. Fix manually and re-run."
done

# ═══════════════════════════════════════════════════════════════════════════
# Step 3: Rust toolchain
# ═══════════════════════════════════════════════════════════════════════════
step "Checking Rust toolchain..."

if [[ -f "$HOME/.cargo/env" ]]; then
    source "$HOME/.cargo/env"
fi

if check_cmd rustup; then
    ok "rustup already installed"
else
    info "Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none
    source "$HOME/.cargo/env"
    check_cmd cargo || fail "cargo not found after rustup install. Check ~/.cargo/env"
    ok "Rust installed"
fi

# ═══════════════════════════════════════════════════════════════════════════
# Step 4: Clone or update repository
# ═══════════════════════════════════════════════════════════════════════════
if [[ "$IS_FRESH" == true ]]; then
    step "Cloning BLISP ${TARGET_VERSION}..."
    git clone "$BLISP_REPO" "$INSTALL_DIR"
    cd "$INSTALL_DIR"
    git checkout "$TARGET_VERSION" --quiet
    ok "Cloned and checked out ${TARGET_VERSION}"
else
    step "Updating repository to ${TARGET_VERSION}..."
    cd "$INSTALL_DIR"

    # Save the current version in case we need to rollback
    ROLLBACK_TAG="$CURRENT_VERSION"

    # Fetch latest tags and objects
    git fetch --tags --quiet

    # Ensure the working tree is clean before switching
    if [[ -n "$(git status --porcelain 2>/dev/null)" ]]; then
        warn "Working tree has local changes — stashing them"
        git stash --quiet
        STASHED=true
    else
        STASHED=false
    fi

    # Checkout the target version
    git checkout "$TARGET_VERSION" --quiet
    ok "Checked out ${TARGET_VERSION}"
fi

# Confirm we are where we expect
TAG_CHECK=$(git describe --tags --exact-match 2>/dev/null || echo "NONE")
if [[ "$TAG_CHECK" != "$TARGET_VERSION" ]]; then
    fail "Expected tag ${TARGET_VERSION}, got '${TAG_CHECK}'. Repository may be corrupt."
fi

# ═══════════════════════════════════════════════════════════════════════════
# Step 5: Build
# ═══════════════════════════════════════════════════════════════════════════
step "Building release binary..."

# Memory check (Linux only)
if [[ -f /proc/meminfo ]]; then
    MEM_KB=$(grep MemAvailable /proc/meminfo | awk '{print $2}')
    if [[ "$MEM_KB" -lt 524288 ]]; then
        warn "Low memory ($(( MEM_KB / 1024 ))MB free). Build may be slow or fail."
        warn "Add swap: sudo fallocate -l 2G /swapfile && sudo chmod 600 /swapfile && sudo mkswap /swapfile && sudo swapon /swapfile"
    fi
fi

# Clean previous build artifacts if upgrading to avoid stale objects
if [[ "$UPGRADE_MODE" == true ]]; then
    info "Cleaning previous build..."
    cargo clean --quiet 2>/dev/null || true
fi

if ! cargo build --locked --release 2>&1; then
    if [[ "$UPGRADE_MODE" == true ]] && [[ "${ROLLBACK_TAG:-}" != "unknown" ]] && [[ "${ROLLBACK_TAG:-}" != "none" ]]; then
        warn "Build failed. Rolling back to ${ROLLBACK_TAG}..."
        git checkout "$ROLLBACK_TAG" --quiet
        cargo build --locked --release 2>&1 || true
        fail "Build of ${TARGET_VERSION} failed. Rolled back to ${ROLLBACK_TAG}."
    fi
    fail "Build failed. Check errors above."
fi

if [[ ! -f target/release/blisp ]]; then
    fail "Build completed but binary not found at target/release/blisp"
fi

ok "Binary built: ${INSTALL_DIR}/target/release/blisp"

# ═══════════════════════════════════════════════════════════════════════════
# Step 6: Verify
# ═══════════════════════════════════════════════════════════════════════════
step "Verifying installation..."

# 6a: Version — extract version string, compare with target
VERSION_OUTPUT=$(./target/release/blisp --version 2>&1)
# TARGET_VERSION is e.g. "v0.2.0", binary prints e.g. "blisp v0.2.0"
if [[ "$VERSION_OUTPUT" != *"${TARGET_VERSION}"* ]]; then
    warn "Version string '${VERSION_OUTPUT}' does not contain '${TARGET_VERSION}'"
    warn "This may be expected if the binary version was not bumped for this tag"
fi
ok "Version: ${VERSION_OUTPUT}"

# 6b: Self-tests
info "Running self-tests..."
SELFTEST_OUTPUT=$(./target/release/blisp --selftest 2>&1) || true
if [[ "$SELFTEST_OUTPUT" == *"All self-tests PASSED"* ]]; then
    ok "All self-tests passed"
else
    echo "$SELFTEST_OUTPUT"
    if [[ "$UPGRADE_MODE" == true ]] && [[ "${ROLLBACK_TAG:-}" != "unknown" ]] && [[ "${ROLLBACK_TAG:-}" != "none" ]]; then
        warn "Self-tests failed! Rolling back to ${ROLLBACK_TAG}..."
        git checkout "$ROLLBACK_TAG" --quiet
        cargo build --locked --release --quiet 2>/dev/null || true
        fail "Self-tests failed for ${TARGET_VERSION}. Rolled back to ${ROLLBACK_TAG}."
    fi
    fail "Self-tests failed. Do not use this binary."
fi

# 6c: Smoke expression
SMOKE_RESULT=$(./target/release/blisp -e '(+ 1 2)' 2>&1 | grep -v "Running in" | tail -1)
if [[ "$SMOKE_RESULT" != "3" ]]; then
    fail "Smoke test failed. Expected '3', got: '${SMOKE_RESULT}'"
fi
ok "Smoke test: (+ 1 2) = 3"

# ═══════════════════════════════════════════════════════════════════════════
# Step 7: Install to PATH
# ═══════════════════════════════════════════════════════════════════════════
step "Installing to PATH..."

cargo install --locked --path . --force --quiet 2>&1

CARGO_BIN="$HOME/.cargo/bin"
if [[ -f "${CARGO_BIN}/blisp" ]]; then
    # Ensure it's on PATH for this session
    export PATH="${CARGO_BIN}:${PATH}"
    ok "blisp installed to ${CARGO_BIN}/blisp"
else
    warn "cargo install did not produce the binary. Use ${INSTALL_DIR}/target/release/blisp directly."
fi

# Check if PATH is persisted in shell profile
SHELL_RC=""
if [[ -f "$HOME/.bashrc" ]]; then
    SHELL_RC="$HOME/.bashrc"
elif [[ -f "$HOME/.zshrc" ]]; then
    SHELL_RC="$HOME/.zshrc"
elif [[ -f "$HOME/.profile" ]]; then
    SHELL_RC="$HOME/.profile"
fi

if [[ -n "$SHELL_RC" ]]; then
    if ! grep -q '.cargo/bin' "$SHELL_RC" 2>/dev/null; then
        if ! grep -q '.cargo/env' "$SHELL_RC" 2>/dev/null; then
            warn "${CARGO_BIN} is not in your shell profile."
            warn "Run:  echo 'source \"\$HOME/.cargo/env\"' >> ${SHELL_RC}"
        fi
    fi
fi

# ═══════════════════════════════════════════════════════════════════════════
# Done
# ═══════════════════════════════════════════════════════════════════════════
echo ""
echo -e "${BOLD}═══════════════════════════════════════════════════════${NC}"
if [[ "$UPGRADE_MODE" == true ]]; then
    echo -e "${GREEN}${BOLD}  BLISP upgraded: ${CURRENT_VERSION} -> ${TARGET_VERSION}${NC}"
else
    echo -e "${GREEN}${BOLD}  BLISP ${TARGET_VERSION} installed successfully${NC}"
fi
echo -e "${BOLD}═══════════════════════════════════════════════════════${NC}"
echo ""
echo "  Binary:    ${INSTALL_DIR}/target/release/blisp"
if check_cmd blisp; then
echo "  Command:   blisp ($(which blisp))"
fi
echo "  Source:    ${INSTALL_DIR}"
echo ""
echo "  Quick test:"
echo "    blisp --selftest"
echo "    blisp -e '(+ 1 2)'"
echo "    blisp --dic"
echo ""
if [[ "$UPGRADE_MODE" == true ]]; then
echo "  Rollback:  bash install.sh --upgrade --version ${CURRENT_VERSION}"
fi
echo ""
