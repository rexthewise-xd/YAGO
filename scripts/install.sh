#!/usr/bin/env bash
# ==============================================================================
# YAGO Installer & Updater for Linux
# https://github.com/rexthewise-xd/YAGO
# ==============================================================================
set -euo pipefail

REPO="rexthewise-xd/YAGO"
APP_NAME="yago"
DISPLAY_NAME="YAGO"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info()  { echo -e "${BLUE}==>${NC} $*"; }
ok()    { echo -e "${GREEN}==>${NC} $*"; }
warn()  { echo -e "${YELLOW}==> WARNING:${NC} $*"; }
err()   { echo -e "${RED}==> ERROR:${NC} $*" >&2; exit 1; }

# Parse arguments
ACTION="install"
LOCAL_SOURCE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --update)
      ACTION="update"
      shift
      ;;
    --uninstall)
      ACTION="uninstall"
      shift
      ;;
    --local)
      LOCAL_SOURCE="${2:-}"
      shift 2 || true
      ;;
    -h|--help)
      echo "YAGO Linux Installer / Updater"
      echo "Usage: $0 [OPTIONS]"
      echo ""
      echo "Options:"
      echo "  --update       Check for updates and update if available"
      echo "  --uninstall    Remove YAGO and associated desktop shortcuts"
      echo "  --local <path> Install from local AppImage artifact"
      echo "  -h, --help     Show this help"
      exit 0
      ;;
    *)
      err "Unknown option: $1"
      ;;
  esac
done

# Target directories
if [[ $EUID -eq 0 ]]; then
  INSTALL_BIN_DIR="/usr/local/bin"
  INSTALL_APPS_DIR="/usr/local/share/applications"
  INSTALL_ICONS_DIR="/usr/local/share/icons/hicolor"
else
  INSTALL_BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
  INSTALL_APPS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
  INSTALL_ICONS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor"
fi

BIN_TARGET="$INSTALL_BIN_DIR/$APP_NAME"
DESKTOP_TARGET="$INSTALL_APPS_DIR/$APP_NAME.desktop"

# Handle uninstall
if [[ "$ACTION" == "uninstall" ]]; then
  info "Uninstalling $DISPLAY_NAME..."
  rm -f "$BIN_TARGET"
  rm -f "$DESKTOP_TARGET"
  find "$INSTALL_ICONS_DIR" -name "$APP_NAME.png" -delete 2>/dev/null || true
  ok "$DISPLAY_NAME has been uninstalled."
  exit 0
fi

# Verify architecture
ARCH=$(uname -m)
if [[ "$ARCH" != "x86_64" ]]; then
  err "Unsupported architecture: $ARCH. YAGO currently supports x86_64 Linux."
fi

mkdir -p "$INSTALL_BIN_DIR" "$INSTALL_APPS_DIR"

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

TARGET_APPIMAGE=""

if [[ -n "$LOCAL_SOURCE" && -f "$LOCAL_SOURCE" ]]; then
  info "Installing from local file: $LOCAL_SOURCE"
  TARGET_APPIMAGE="$TMPDIR/yago.AppImage"
  cp "$LOCAL_SOURCE" "$TARGET_APPIMAGE"
else
  info "Checking latest release from GitHub ($REPO)..."
  LATEST_RELEASE=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null || true)

  DOWNLOAD_URL=""
  if [[ -n "$LATEST_RELEASE" ]]; then
    DOWNLOAD_URL=$(echo "$LATEST_RELEASE" | grep -o 'https://[^"]*YAGO[^"]*\.AppImage' | head -n 1 || true)
    if [[ -z "$DOWNLOAD_URL" ]]; then
      DOWNLOAD_URL=$(echo "$LATEST_RELEASE" | grep -o 'https://[^"]*\.AppImage' | head -n 1 || true)
    fi
  fi

  if [[ -z "$DOWNLOAD_URL" ]]; then
    # Fallback to direct latest release asset name
    DOWNLOAD_URL="https://github.com/$REPO/releases/latest/download/YAGO-x86_64.AppImage"
  fi

  info "Downloading YAGO AppImage..."
  TARGET_APPIMAGE="$TMPDIR/yago.AppImage"
  if ! curl -fSL "$DOWNLOAD_URL" -o "$TARGET_APPIMAGE" 2>/dev/null; then
    # Try looking for a local build in the current workspace if running from source repo
    if [[ -f "release/latest/YAGO-x86_64.AppImage" ]]; then
      warn "Remote download failed, using local release/latest/YAGO-x86_64.AppImage"
      cp "release/latest/YAGO-x86_64.AppImage" "$TARGET_APPIMAGE"
    elif [[ -f "src-ui-rs/dist/yago-ui_0.1.0_x86_64.AppImage" ]]; then
      warn "Remote download failed, using local src-ui-rs/dist/yago-ui_0.1.0_x86_64.AppImage"
      cp "src-ui-rs/dist/yago-ui_0.1.0_x86_64.AppImage" "$TARGET_APPIMAGE"
    else
      err "Failed to download $DOWNLOAD_URL and no local build found."
    fi
  fi
fi

chmod +x "$TARGET_APPIMAGE"

# Extract desktop and icons from the AppImage
info "Extracting desktop integration assets..."
(
  cd "$TMPDIR"
  APPIMAGE_EXTRACT_AND_RUN=1 "$TARGET_APPIMAGE" --appimage-extract "usr/share/*" >/dev/null 2>&1 || true
  APPIMAGE_EXTRACT_AND_RUN=1 "$TARGET_APPIMAGE" --appimage-extract "*.desktop" >/dev/null 2>&1 || true
  APPIMAGE_EXTRACT_AND_RUN=1 "$TARGET_APPIMAGE" --appimage-extract "*.png" >/dev/null 2>&1 || true
)

# Install executable
info "Installing binary to $BIN_TARGET..."
cp "$TARGET_APPIMAGE" "$BIN_TARGET"
chmod +x "$BIN_TARGET"

# Install Desktop file
cat > "$DESKTOP_TARGET" <<EOF
[Desktop Entry]
Name=$DISPLAY_NAME
Comment=High-performance game mod manager for 3DMigoto-compatible games
Exec=$BIN_TARGET %u
Icon=$APP_NAME
Terminal=false
Type=Application
Categories=Game;Utility;
MimeType=x-scheme-handler/yago;
StartupWMClass=yago-ui
EOF
chmod 644 "$DESKTOP_TARGET"

# Install icons if extracted
if [[ -d "$TMPDIR/squashfs-root/usr/share/icons/hicolor" ]]; then
  cp -r "$TMPDIR/squashfs-root/usr/share/icons/hicolor/"* "$INSTALL_ICONS_DIR/" 2>/dev/null || true
elif [[ -f "$TMPDIR/squashfs-root/yago-ui.png" ]]; then
  mkdir -p "$INSTALL_ICONS_DIR/512x512/apps"
  cp "$TMPDIR/squashfs-root/yago-ui.png" "$INSTALL_ICONS_DIR/512x512/apps/$APP_NAME.png"
fi

# Update desktop databases if tools are available
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$INSTALL_APPS_DIR" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -f -t "$INSTALL_ICONS_DIR" 2>/dev/null || true

# Check PATH
if [[ ":$PATH:" != *":$INSTALL_BIN_DIR:"* ]]; then
  warn "$INSTALL_BIN_DIR is not in your PATH."
  echo "Add the following line to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
  echo "  export PATH=\"\$PATH:$INSTALL_BIN_DIR\""
fi

ok "Successfully installed $DISPLAY_NAME to $BIN_TARGET!"
ok "You can now run '$APP_NAME' from your terminal or application launcher."
