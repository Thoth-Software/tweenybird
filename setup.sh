#!/bin/bash
set -euo pipefail

# GP AI Inbetween — macOS setup script
# Builds Rust binary, packages addon, installs into Blender

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}[✓]${NC} $1"; }
warn()  { echo -e "${YELLOW}[!]${NC} $1"; }
error() { echo -e "${RED}[✗]${NC} $1"; exit 1; }

# --- Detect project root ---
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR"

if [[ ! -d "$PROJECT_ROOT/gp_inbetween" ]]; then
    error "Run this script from the project root (directory containing gp_inbetween/)"
fi

# --- Check prerequisites ---
echo ""
echo "=== Checking prerequisites ==="

command -v cargo >/dev/null 2>&1 || error "Rust not installed. Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
info "Rust: $(rustc --version)"

command -v ffmpeg >/dev/null 2>&1 || error "ffmpeg not installed. Run: brew install ffmpeg"
info "ffmpeg: $(ffmpeg -version 2>&1 | head -1)"

if [[ -z "${REPLICATE_API_TOKEN:-}" ]]; then
    warn "REPLICATE_API_TOKEN not set. Addon will need API key configured in Blender preferences."
else
    info "Replicate API token: set"
fi

# --- Detect Blender ---
echo ""
echo "=== Detecting Blender ==="

BLENDER_ADDON_DIR=""
BLENDER_APP="/Applications/Blender.app"

if [[ -d "$BLENDER_APP" ]]; then
    # Find highest installed Blender version's addon directory
    BLENDER_SUPPORT="$HOME/Library/Application Support/Blender"
    if [[ -d "$BLENDER_SUPPORT" ]]; then
        BLENDER_VERSION=$(ls -1 "$BLENDER_SUPPORT" | sort -V | tail -1)
        BLENDER_ADDON_DIR="$BLENDER_SUPPORT/$BLENDER_VERSION/scripts/addons"
        mkdir -p "$BLENDER_ADDON_DIR"
        info "Blender $BLENDER_VERSION detected"
    else
        warn "Blender app found but no support directory. Will create zip only."
    fi
else
    warn "Blender.app not found in /Applications. Will create zip only."
fi

# --- Build Rust binary ---
echo ""
echo "=== Building Rust binary ==="

cd "$PROJECT_ROOT/gp_inbetween"
cargo build --release
info "Binary built: $(du -h target/release/gp_inbetween | cut -f1) at target/release/gp_inbetween"
cd "$PROJECT_ROOT"

# --- Package addon ---
echo ""
echo "=== Packaging addon ==="

ADDON_DIR="$PROJECT_ROOT/gp_ai_inbetween"

if [[ ! -d "$ADDON_DIR" ]]; then
    error "Addon directory not found at $ADDON_DIR"
fi

# Copy binary into addon
mkdir -p "$ADDON_DIR/bin"
cp "$PROJECT_ROOT/gp_inbetween/target/release/gp_inbetween" "$ADDON_DIR/bin/gp_inbetween_mac"
chmod +x "$ADDON_DIR/bin/gp_inbetween_mac"
xattr -cr "$ADDON_DIR/bin/gp_inbetween_mac" 2>/dev/null || true
info "Binary copied to addon/bin/"

# Create distributable zip
ZIP_PATH="$PROJECT_ROOT/gp_ai_inbetween.zip"
rm -f "$ZIP_PATH"
cd "$PROJECT_ROOT"
zip -r "$ZIP_PATH" gp_ai_inbetween -x "*.DS_Store" "*.pyc" "__pycache__/*" >/dev/null
info "Addon zip: $(du -h "$ZIP_PATH" | cut -f1) at $ZIP_PATH"

# --- Install into Blender ---
echo ""
echo "=== Installing addon ==="

if [[ -n "$BLENDER_ADDON_DIR" ]]; then
    # Remove old install
    rm -rf "$BLENDER_ADDON_DIR/gp_ai_inbetween"

    # Copy addon
    cp -r "$ADDON_DIR" "$BLENDER_ADDON_DIR/gp_ai_inbetween"

    # Clear quarantine on installed binary
    xattr -cr "$BLENDER_ADDON_DIR/gp_ai_inbetween/bin/gp_inbetween_mac" 2>/dev/null || true
    chmod +x "$BLENDER_ADDON_DIR/gp_ai_inbetween/bin/gp_inbetween_mac"

    info "Addon installed to: $BLENDER_ADDON_DIR/gp_ai_inbetween"
    warn "Restart Blender if it's running (Python module cache)"
else
    warn "Skipped Blender install — install manually from $ZIP_PATH"
fi

# --- CLI smoke test ---
echo ""
echo "=== Quick smoke test ==="

BINARY="$PROJECT_ROOT/gp_inbetween/target/release/gp_inbetween"

if "$BINARY" --help >/dev/null 2>&1; then
    info "Binary runs OK"
else
    error "Binary failed to execute"
fi

# --- Done ---
echo ""
echo "=== Done ==="
echo ""
info "Build + install complete."
echo ""
echo "  Next steps:"
echo "    1. Open Blender"
echo "    2. Edit → Preferences → Add-ons → enable 'GP AI Inbetween'"
echo "    3. Set your Replicate API key in addon preferences"
echo "    4. Create GP keyframes, select GP object, press N → GP AI tab"
echo ""

if [[ -n "${REPLICATE_API_TOKEN:-}" ]]; then
    echo "  CLI test:"
    echo "    $BINARY generate --frame-a a.png --frame-b b.png --num-frames 4 --output-dir ./out"
    echo ""
fi#!/usr/bin/env sh
