# Architecture & Internal Design

YAGO is built as a high-performance modular Rust workspace paired with a reactive Dioxus 0.7 desktop client (`src-ui-rs`).

---

## Workspace Crates

```
YAGO/
├── crates/
│   ├── app/           # Application coordinator (AppCore, command handlers, event bus)
│   ├── ini/           # Nom-based AST parser & serializer for 3DMigoto INI syntax
│   ├── launcher/      # Wine/Proton runner resolution, umu-run, graphics audit, input routing
│   ├── loader/        # LoaderSpec abstraction, multi-strategy deployment, proxy DLL injection, RabbitWrapper
│   ├── mod_patches/   # Router compilation, DVT sync, diagnostic parser, patch fixer, WebGL mesh extractor
│   ├── resources/     # Multi-registry SHA-256 sync engine, GitHub release downloads, GameBanana API v11
│   ├── scripting/     # Sandboxed Rhai scripting engine, capability-jailed mod_fs/INI APIs, trust gate
│   ├── storage/       # Decentralized database (Librarian), loadout bundles, clean exporter, wish extractor
│   ├── sync/          # Sophon protocol client, Kuro Games CDN client, streaming MD5 verifier, delta patcher
│   └── vfs/           # Virtual filesystem (symlinks/junctions), multi-format archive extraction, DDS transcoding
├── src-ui-rs/         # Dioxus 0.7 desktop application
└── resources/         # Embedded game templates, shader patches, and hash maps
```

---

## Core Systems & Pipelines

### 1. 3DMigoto Nom AST Compiler (`crates/ini` & `crates/mod_patches`)
- **Abstract Syntax Tree**: Parses INI files into structured sections, key-value pairs, comments, and flow control commands (`if`, `elif`, `else`, `endif`, `pre run`, `post run`).
- **Namespacer**: Rewrites variables and section headers to avoid cross-mod collisions, while respecting external namespaces (`$\YAGO\...`).
- **Validation Engine**: Performs AST checks for balanced flow control, circular command list cycles, and undeclared ghost variables.

### 2. Wardrobe Router & Compile Pipeline (`crates/mod_patches/src/compiler.rs`)
- Compiles multiple character mods into a single consolidated `22_Characters/{Character}/merged.ini`.
- Injects on-screen character presence tracking (`$wardrobe_active_{char} = 1` in overrides, reset in `[Present]`).
- Compiles compound logic gates for live SFW/NSFW switching and Land/Aqua context swaps.

### 3. Virtual File System (`crates/vfs`)
- Deploys mods into the target `Mods/` directory using zero-copy links:
  - **Linux**: POSIX symbolic links.
  - **Windows**: NTFS directory junctions and hardlinks, preventing disk bloat.
- Handles atomic unpacking of `.zip`, `.7z`, and `.tar.gz` archives with path sanitization and texture transcoding.

### 4. Decentralized Storage (`crates/storage`)
- **Per-Game Databases**: Each title maintains an isolated `game.json` database under `~/.local/share/yago/games/{game_id}/` (or Windows `%APPDATA%/yago`).
- **Immutable Mod UUIDs**: Every mod is indexed by a UUID, protecting against filename collisions.
- **Portability**: Supports exporting and importing loadout configurations via portable JSON Loadout Bundles.

### 5. Cross-Platform Runtime & Launcher (`crates/launcher` & `crates/loader`)
- Dispatches games with fine-grained platform options:
  - Linux: direct Proton, Wine, or `umu-run` integration with Vulkan/Wayland environment variables.
  - Windows: Native process spawn or Shell execution (`cmd /C start "" /D <dir> <exe>`).
- Cascading F10 keystroke router targeting X11, XWayland, and pure Wayland window surfaces.
- Automated graphics configuration audits detecting Unreal Engine streaming bottlenecks and conflicting mod INIs.

### 6. Creator Dev Suite (`crates/mod_patches` & `src-ui-rs`)
- **DVT Blender Draw-Value Transfer**: Scans Blender exports and updates draw counts and `.buf`/`.ib` files without touching mod script logic.
- **Diagnostics Parser**: Streams the tail of `d3d11_log.txt` to isolate syntax errors and missing asset references.
- **Offline WebGL 3D Mesh Engine**: Inline WebGL renderer extracting vertex and index buffers from 3DMigoto mods for responsive 3D viewport inspection.
- **Ctrl+K Command Palette**: Global fuzzy action switcher for seamless keyboard navigation.

### 7. Multi-Registry Sync Engine (`crates/resources/src/registry.rs`)
- **Deterministic SHA-256 Verification**: Connects to official and third-party registries (`https://` or `file://`), downloads `index.json`, and verifies SHA-256 digests before writing.
- **Runtime Overlays**: Registry game manifests, character hashes, automated fixer rulesets, effect specs, and runner definitions dynamically overlay bundled assets without requiring an application release.
- **Atomic Caching**: Synced definitions write to `.tmp` files and rename atomically into `app_data/registry/<id>/`.

### 8. Declarative LoaderSpec & Multi-Strategy Deployment (`crates/loader/src/spec.rs`)
- **Decoupled Loader Specifications**: Manifests define proxy DLLs, support directories, target mod paths, config files, reload keybinds, and Wine DLL overrides.
- **Multi-Strategy Mod Deployment**:
  - `ini_ast_3dmigoto`: Standard 3DMigoto namespacing, AST consolidation, and compound logic gates.
  - `vfs_direct`: Pure virtual zero-copy symlinking of mod directories without INI parsing.
  - `pak_merge`: Direct deployment of Unreal Engine `.pak`, `.ucas`, and `.utoc` archives to target mod directories.
  - `scripted`: External hook-driven deployment pipelines.

### 9. Sandboxed Rhai Scripting Engine & Trust Controls (`crates/scripting`)
- **Pure Rust Sandbox**: Evaluates community Rhai scripts without native C dependencies or runtime vulnerabilities.
- **Hard Resource Quotas**: Configurable execution limits (500k operations, 32 call stack levels, 2 MiB string bounds, collection caps).
- **Jailed Host APIs**:
  - `mod_fs`: Strictly jailed to the target directory with traversal protection (`..` prevention) and in-memory dry-run virtual overlays.
  - `ini`: Safe parsing and editing (`ini_replace_hash`, `ini_get_value`, `ini_set_value`).
  - `log` & `audit`: Structured warning emission.
- **Trust Gate**: Official registry scripts are trusted by default; 3rd-party community scripts require explicit user opt-in in Settings. Automatic `.yago_backups/` snapshots protect mod files prior to execution.
