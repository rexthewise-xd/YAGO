# YAGO - Yet Another Game Organizer

A high-performance, cross-platform game mod manager and launcher built with Dioxus 0.7 and modular Rust crates.

---

## Features Overview

### 🎮 Supported Titles & Ecosystem
- **HoYoverse**: *Genshin Impact*, *Honkai: Star Rail*, *Zenless Zone Zero*, *Honkai Impact 3rd*
- **Kuro Games**: *Wuthering Waves* (with streaming 1 MiB chunk MD5 integrity caching & protected save paths)
- **Hypergryph**: *Arknights: Endfield* (EFMI template & loader ecosystem ready)

### 🧩 3DMigoto AST Compiler & Wardrobe Router
- **Advanced Syntax**: Full AST compiler support for `elif`, `else if`, `pre run`, `post run`, and cross-namespace variables (`$\YAGO\...`).
- **Collision-Free Wardrobe Cycling**: Generates standard 3DMigoto `[KeyCycle*]` bindings with active on-screen character tracking (`$wardrobe_active_{char}`) so pressing `[` / `]` cycles only the character currently in the viewport.
- **Context-Aware Swaps**: Segregates character wardrobes into Land vs. Swimming (Aqua) pools (`$\global\tracking\swimming == 1`).
- **Global In-Game SFW Switch**: Live single-key toggle (`F9`) hiding NSFW skins on-the-fly with automatic fallback to primary SFW variants.
- **Shuffle-on-Launch**: Automatically rolls 1 random unlocked skin per character on start, respecting per-mod shuffle locks.

### 🛠️ Data-Driven Patch Fixer & Effect Libraries
- **Patch Fixer Engine**: Data-driven automated model repair (ORFix, FaceFix `ps-t0`, slot remapping, and hash migrations) with dry-run diffs, `NOAPPLIER` suppression, and 1-click transactional rollback (`.yago_backups/`).
- **Shared Effect Libraries**: Centralized installation, versioning, and toggling of shared shader libraries (*ORFix*, *TexFx*, *RabbitFX*).
- **Variant Presets & INI Anchors**: Capture live configurations directly from `d3dx_user.ini` into named presets with automated `[Present]` bridges, persistent section overrides, and on-screen banner suppression (`$creditinfo`).
- **Interactive Keybinds**: Visual key rebinding with direct file rewriting and conflict awareness.

### 🌐 GameBanana & Community Ecosystem
- **GameBanana Browser**: Built-in API v11 client with fuzzy searching, preview art, categories, and direct downloads.
- **`yago://` 1-Click Protocol**: Native URI handler for instant 1-click mod installation from browsers.
- **Update Tracking**: Non-destructive version tracking preserving custom names, tags, presets, and anchors.
- **Shareable JSON Loadout Bundles**: 1-click export and import of full loadout configurations matching local mods.
- **Clean Mod Exporter**: Strips internal `.yago_backups/` and uncloaks `*.ini.yago_source` files to standard `*.ini` archives ready for vanilla sharing or GameBanana publishing.

### ⚡ Cross-Platform Runtime & Launcher
- **Linux Execution**: Direct support for `umu-run`, GE-Proton, Proton-CachyOS, and DW-Proton; compositor-agnostic F10 hot-reload routing (X11 / XWayland via `xdotool`, Wayland via `wtype`, and `enigo` fallback); per-profile toggles for native Wayland, DXVK Low Latency, OBS VkCapture, and LSFG-VK.
- **Windows Runtime**: Choice between Native process and Shell execution (`cmd /C start ""`) matching official launcher behavior; Unsafe Mode DLL pinning (`d3d11.dll` / `d3dx.ini`); RabbitWrapper proxy chaining.
- **Pre-Launch & Post-Exit Lifecycle**: Asynchronous pre-launch and post-exit shell hooks; automated graphics compatibility audits (detecting missing Unreal Engine LOD streaming parameters and rogue mod `d3dx.ini` overrides with 1-click auto-fixes).
- **Multi-CDN Delta Patching & Voice Packs**: Sophon `getPatchBuild` LDiff delta patching, selectable game regions (Global, CN, Bilibili, SEA), and post-install multi-language Voice-Over Pack manager.

### 🎨 Creator Dev Suite
- **DVT Blender Draw-Value Transfer**: Automatically transfers updated `drawindexed` / `draw` counts and `.buf` / `.ib` geometry from new Blender exports into existing mods while strictly preserving toggle logic, comments, and variables.
- **1-Click Hunting Mode & `gui_collect` Bridge**: Toggles `[Hunting] hunting = 2` for dumping draw calls and provides a persistent bridge for `gui_collect auto_generated.ini`.
- **Live `d3d11_log.txt` Diagnostic Console**: Parses tail log records into categorized syntax errors, missing resources, duplicate hashes, and performance warnings.
- **Per-Mod ShaderFixes Lifecycle**: Detects and symlinks mod `.hlsl` and vertex/pixel shader txt files into active `ShaderFixes/`, cleaning them up upon mod deactivation.
- **Embedded WebGL 3D Mesh Previewer**: Pure offline WebGL canvas inside Mod Inspector parsing `.buf` / `.ib` geometry with orbit rotation, zoom, wireframe toggle, and vertex/triangle metrics.
- **Global Ctrl+K Command Palette**: Fast keyboard-driven command palette for game switching, navigation, audits, and quick actions.

---

## Tech Stack

- **UI**: Dioxus 0.7 Desktop (`src-ui-rs`, custom WebGL inline shaders, responsive theme system)
- **Backend / Core**: Rust 2021 Edition, Tokio async runtime, Prost Protobuf, Zstd, Nom INI parser

---

## Architecture & Modular Crates

| Crate | Purpose |
|---|---|
| **app** (`yago-app`) | Application coordinator, command handlers, event bus, and runtime state |
| **yago-ui** (`src-ui-rs`) | Dioxus 0.7 desktop client with custom asset protocol (`yago-asset://`) |
| **storage** | Decentralized database (`Librarian`), loadout bundles, clean mod exporter, wish extractor |
| **ini** | Nom-based AST compiler with flow control (`if`/`elif`/`else`/`endif`) and variable validation |
| **mod_patches** | Router compilation, DVT sync, diagnostic parser, patch fixer engine, WebGL mesh extractor |
| **vfs** | Virtual filesystem (symlinks/junctions), multi-format extractors, DDS transcoder |
| **launcher** | Wine/Proton runner resolution, umu-run dispatch, graphics audits, input routing |
| **loader** | DLL proxy injection, RabbitWrapper chaining, Linux shield interceptor |
| **sync** | Sophon protocol, Kuro Games CDN client, streaming MD5 verifier, HDiff/KrDiff patcher |
| **resources** | Remote asset management (GE-Proton, ReShade, shared effect libraries, GameBanana API) |

---

## Getting Started

### Prerequisites
- [Rust](https://rustup.rs/) (latest stable)
- System dependencies (Linux): `gcc`, `protoc`, GTK3, `webkit2gtk-4.1`

### Build & Run
```bash
# Run desktop UI in development
cargo run -p yago-ui

# Hot-reload UI development
cd src-ui-rs && dx serve

# Run test suite across entire workspace
cargo test --workspace

# Build optimized release binary
cargo build -p yago-ui --release
```

---

## License & Safety
Licensed under MIT. See [LICENSE](LICENSE) for details. YAGO uses zero-copy symlinks/junctions and operates non-destructively without modifying game binaries. Use responsibly.
