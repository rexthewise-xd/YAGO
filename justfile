# YAGO - Justfile
# Central command hub for all development and release tasks
ROOT := justfile_directory()

# Default recipe - show help
default: help

# Help
help:
    @echo "YAGO - Available Commands"
    @echo ""
    @echo "Development:"
    @echo "  just dev                    Run development server (Dioxus UI)"
    @echo "  just build                  Build release binary for current host"
    @echo "  just check                  Check Rust code across workspace"
    @echo "  just watch                  Watch for changes"
    @echo ""
    @echo "Testing:"
    @echo "  just test                   Run all tests"
    @echo "  just bench                  Run benchmarks"
    @echo "  just review                 Review snapshots"
    @echo ""
    @echo "Quality:"
    @echo "  just lint                   Clippy + format check"
    @echo "  just fmt                    Format code"
    @echo ""
    @echo "Release:"
    @echo "  just release-appimage       Build Linux AppImage (standalone)"
    @echo "  just release-flatpak        Build Flatpak package"
    @echo "  just release-pkgbuild       Build Arch Linux package (makepkg)"
    @echo "  just release-win            Build Windows portable + installer"
    @echo "  just release-win-portable   Build Windows portable executable"
    @echo "  just release-win-installer  Build Windows NSIS setup executable"
    @echo "  just release-linux          Build all Linux artifacts (AppImage, Flatpak, PKGBUILD)"
    @echo "  just release-all            Build all release packages"
    @echo "  just release-list           List generated release artifacts"
    @echo ""
    @echo "Setup:"
    @echo "  just setup                  Initialize dev environment"
    @echo "  just clean                  Clean build artifacts"
    @echo ""

# === Development ===

dev:
    cargo run -p yago-ui

build:
    cargo build -p yago-ui --release

check:
    cargo check --workspace

watch:
    cargo watch -x check -x nextest

# === Testing ===

test:
    cargo nextest run

bench:
    cargo bench

review:
    cargo insta review

# === Quality ===

lint: fmt-check clippy

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

ci: lint test bench

# === Crate-specific tests ===

test-app:
    cargo test -p yago-app

test-ui:
    cargo test -p yago-ui

test-ini:
    cargo test -p ini

test-vfs:
    cargo test -p vfs

test-storage:
    cargo test -p storage

test-mod-patches:
    cargo test -p mod_patches

test-launcher:
    cargo test -p launcher

test-sync:
    cargo test -p sync

test-resources:
    cargo test -p resources

test-loader:
    cargo test -p loader

# === Setup ===

setup:
    @echo "Checking core requirements..."
    @rustc --version
    @cargo --version
    @if [ ! -f .env ] && [ -f .env.example ]; then cp .env.example .env; echo "Created .env"; fi
    @echo "Environment setup ready."

clean:
    cargo clean
    rm -rf src-ui-rs/dist release/build

# === Dioxus CLI ===

dx-serve:
    cd src-ui-rs && dx serve

dx-build:
    cd src-ui-rs && dx build --release

dx-bundle:
    cd src-ui-rs && dx bundle --release

# === Release Infrastructure ===

release-init:
    mkdir -p release/latest release/older release/build

# === Release Packages ===

release-appimage: release-init
    @echo "📦 Building Linux AppImage..."
    @if [ ! -f "$HOME/.local/share/.dx/tools/linuxdeploy-x86_64.AppImage" ]; then \
        mkdir -p "$HOME/.local/share/.dx/tools"; \
        curl -fSL -o "$HOME/.local/share/.dx/tools/linuxdeploy-x86_64.AppImage" "https://github.com/tauri-apps/binary-releases/releases/download/linuxdeploy/linuxdeploy-x86_64.AppImage"; \
        chmod +x "$HOME/.local/share/.dx/tools/linuxdeploy-x86_64.AppImage"; \
    fi
    export NO_STRIP=1
    export APPIMAGE_EXTRACT_AND_RUN=1
    cd src-ui-rs && dx bundle --release --package-types appimage
    mkdir -p release/latest
    find src-ui-rs/dist/ -name "*.AppImage" -exec cp -f {} release/latest/YAGO-x86_64.AppImage \; 2>/dev/null || find target/dx/yago-ui/ -name "*.AppImage" -exec cp -f {} release/latest/YAGO-x86_64.AppImage \; 2>/dev/null
    @echo "🎉 AppImage ready: release/latest/YAGO-x86_64.AppImage"

release-flatpak: release-init build
    @echo "📦 Building Flatpak bundle..."
    mkdir -p release/build/flatpak release/latest
    rm -rf release/build/flatpak/build
    flatpak-builder --force-clean --user --install-deps-from=flathub --repo=release/build/flatpak/repo release/build/flatpak/build packaging/flatpak/io.github.rexthewise_xd.YAGO.json || true
    flatpak build-bundle release/build/flatpak/repo release/latest/yago.flatpak io.github.rexthewise_xd.YAGO || true
    @echo "🎉 Flatpak ready: release/latest/yago.flatpak"

release-pkgbuild: release-init
    @echo "📦 Building Arch Linux package (PKGBUILD)..."
    mkdir -p release/build/pkgbuild release/latest
    cp packaging/pkgbuild/PKGBUILD packaging/pkgbuild/yago.desktop release/build/pkgbuild/
    cd release/build/pkgbuild && makepkg -f --nodeps
    cp -f release/build/pkgbuild/*.pkg.tar.zst release/latest/ 2>/dev/null || true
    cp -f packaging/pkgbuild/PKGBUILD release/latest/
    @echo "🎉 Arch package ready: release/latest/"

release-win-portable: release-init
    @echo "📦 Building Windows Portable..."
    mkdir -p release/latest
    if [ "$OS" = "Windows_NT" ]; then \
        cargo build --release -p yago-ui -p launcher --bin win_helper; \
        cp -f target/release/yago-ui.exe release/latest/yago-portable.exe; \
    else \
        cargo xwin build --release --target x86_64-pc-windows-msvc -p yago-ui -p launcher --bin win_helper; \
        cp -f target/x86_64-pc-windows-msvc/release/yago-ui.exe release/latest/yago-portable.exe; \
    fi
    @echo "🎉 Windows portable ready: release/latest/yago-portable.exe"

release-win-installer: release-init
    @echo "📦 Building Windows NSIS Setup Installer..."
    mkdir -p release/latest
    if [ "$OS" = "Windows_NT" ]; then \
        cargo build --release -p yago-ui -p launcher --bin win_helper; \
        makensis -DVERSION=0.1.0 -DBUILD_DIR="{{ROOT}}/target/release" -DOUTPUT_EXE="{{ROOT}}/release/latest/yago-setup.exe" -DICON_PATH="{{ROOT}}/src-ui-rs/assets/icons/icon.ico" packaging/windows/installer.nsi; \
    else \
        cargo xwin build --release --target x86_64-pc-windows-msvc -p yago-ui -p launcher --bin win_helper; \
        makensis -DVERSION=0.1.0 -DBUILD_DIR="{{ROOT}}/target/x86_64-pc-windows-msvc/release" -DOUTPUT_EXE="{{ROOT}}/release/latest/yago-setup.exe" -DICON_PATH="{{ROOT}}/src-ui-rs/assets/icons/icon.ico" packaging/windows/installer.nsi; \
    fi
    @echo "🎉 Windows setup installer ready: release/latest/yago-setup.exe"

release-win: release-win-portable release-win-installer

release-linux: release-appimage release-flatpak release-pkgbuild

release-all: release-appimage release-flatpak release-pkgbuild release-win

release-list:
    @ls -lh release/latest/ 2>/dev/null || echo "No releases found."

# === Utilities ===

update:
    cargo update
