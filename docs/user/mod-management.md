# Mod Management & Creator Tools

YAGO pairs zero-copy virtual filesystem deployments with an advanced 3DMigoto AST compiler and an integrated creator development suite.

---

## 3DMigoto AST Compiler & Wardrobe Router

### Collision-Free Character Cycling
- Instead of brittle hotkey overwrites, YAGO isolates multiple character skins into a unified `22_Characters/{Character}/merged.ini`.
- Emits standard 3DMigoto cycle syntax (`[KeyCycle{Char}] key = ] back = [ type = cycle $wardrobe_{char} = 0, 1, 2...`).
- **On-Screen Character Tracking**: Emits `$wardrobe_active_{char} = 1` inside texture overrides and `post $wardrobe_active_{char} = 0` in `[Present]`, ensuring that pressing `[` / `]` cycles only the character currently visible on-screen.

### Context-Aware Wardrobe Pools
- Configure mod context pools (`Both`, `Land`, or `Aqua`).
- Automatically gates swimming/aqua skins on `$\global\tracking\swimming == 1` and land skins on `$\global\tracking\swimming == 0`.

### Global In-Game SFW Switch
- Enable the SFW toggle (default key `F9`) in Profile settings.
- Automatically compiles `00_Configs/yago_sfw_switch.ini` (`$\YAGO\SFW\sfw_mode = 0, 1`).
- When toggled in-game, NSFW skins immediately disappear, and the character falls back seamlessly to their primary SFW outfit.

### Shuffle-on-Launch & Mod Locks
- Automatically selects a random unlocked skin for every character upon game launch.
- Use the **Lock from Shuffle** toggle in Mod Inspector to pin favorite skins permanently.

---

## Data-Driven Patch Fixer Engine

Automatically repairs legacy or broken character mods:
- **ORFix Ordering**: Properly places face and normal map overrides before lighting passes.
- **Slot Remapping & Hash Migration**: Translates outdated vertex/texture hashes to current game versions.
- **Dry-Run Previews**: Inspect exact line-by-line unified diffs before applying any patch.
- **Transactional Rollback**: Every fix creates a timestamped snapshot under `.yago_backups/`, enabling 1-click restore.
- **`NOAPPLIER` Protection**: Honors `NOAPPLIER` marker files to skip intentional custom shader layouts.

---

## Variant Presets & INI Anchors

- **d3dx_user.ini Capture**: Captures live in-game slider adjustments from `d3dx_user.ini` and converts them into named Variant Presets.
- **Preset Bridge**: Automatically injects preset variables into mod logic under `[Present]`.
- **INI Anchors**: Pin permanent section/key/value overrides directly from the UI without manually modifying mod text.
- **Banner Suppression**: 1-click suppression of annoying `$creditinfo` on-screen text banners.

---

## GameBanana & Community Ecosystem

- **GameBanana Browser**: In-app browser for browsing mods, viewing previews, and initiating downloads.
- **`yago://` 1-Click Protocol**: Protocol handler enabling 1-click mod downloads from GameBanana mod pages.
- **Mod Update Tracking**: Tracks remote mod updates without overwriting customized mod metadata, tags, or variant presets.
- **Shareable JSON Loadout Bundles**: Export and import complete mod loadout configurations as portable JSON files.
- **Clean Mod Exporter**: Packages mods into `.zip` archives, uncloaking any `.ini.yago_source` files and removing `.yago_backups/` so the archive is ready for vanilla sharing or publishing.

---

## Creator Dev Suite

- **DVT Blender Draw-Value Transfer**: Synchronizes updated `drawindexed` / `draw` counts and `.buf` / `.ib` geometry from Blender exports into existing mods while preserving all conditional toggle logic, comments, and variables.
- **1-Click Hunting Mode**: Quickly enables `[Hunting] hunting = 2` in `d3dx.ini` for dumping textures and shaders.
- **`gui_collect` Bridge**: Bridges external `gui_collect` auto-generated INIs into 3DMigoto without risk of loader overwrites.
- **`d3d11_log.txt` Diagnostic Console**: Analyzes 3DMigoto log output, categorizing syntax errors, duplicate hashes, and missing resources.
- **Per-Mod ShaderFixes Tracking**: Tracks custom `.hlsl` and vertex/pixel shader files, ensuring shaders are deployed only when their parent mod is active.
- **Embedded WebGL 3D Mesh Previewer**: Visualizes 3DMigoto 3D mesh geometry (`.buf` + `.ib`) with orbit rotation, zoom, wireframe toggle, and vertex/triangle counts directly inside YAGO.
- **Global Ctrl+K Command Palette**: Fast keyboard-driven command palette for instant game switching, view navigation, audits, and quick actions.
