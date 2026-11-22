# Launcher & Runtime Features

YAGO provides a high-performance cross-platform game launcher and runtime environment tailored for 3DMigoto mod loaders and modern desktop gaming.

---

## Cross-Platform Execution

### Linux Runtime Support
- **Runner Discovery**: Automatic detection of Proton-GE, Proton-CachyOS, DW-Proton, and custom Wine builds.
- **UMU-Launcher**: Optional `umu-run` dispatch with configurable `GAMEID`, `PROTONPATH`, and `WINEPREFIX`.
- **Advanced Display & Optimization**:
  - **Native Wayland Driver** (`PROTON_ENABLE_WAYLAND=1`): Zero-overhead display on Wayland compositors.
  - **DXVK Low Latency** (`PROTON_DXVK_LOWLATENCY=1`): Enhanced frame pacing and reduced input lag.
  - **OBS VkCapture** (`OBS_VKCAPTURE=1`): Direct Vulkan frame capture for streaming and recording.
  - **LSFG-VK** (`ENABLE_LSFG_VK=1`): Vulkan frame generation via Lossless Scaling.
  - **Gamescope & Gamemode**: High-fidelity micro-compositing, resolution scaling, and system CPU/IO priority.
- **Compositor-Agnostic Hot-Reload Routing**: Pressing F10 in YAGO hot-reloads 3DMigoto in-game using a cascading fallback:
  1. `xdotool` for X11 and XWayland windows
  2. `wtype` for pure Wayland sessions
  3. `enigo` synthetic input fallback

### Windows Runtime Support
- **Execution Methods**:
  - **Native Execution**: Direct process spawn with argument pass-through.
  - **Shell Execution** (`cmd /C start "" /D <dir> <exe>`): Launches titles through the Windows shell, matching official launcher environments (required for titles like Genshin Impact).
- **Unsafe Mode (DLL Pinning)**: Prevents YAGO from overwriting or unlinking manually placed `d3d11.dll` and `d3dx.ini` files during deployment and teardown.
- **RabbitWrapper Chaining**: Chains `RabbitWrapper.dll` as `dxgi.dll` alongside `ReShade.dll` for advanced graphics injection.

---

## Lifecycle Commands & Automation

- **Pre-Launch Shell Commands**: Run custom setup scripts, shader prep, or companion utilities asynchronously before the game launches.
- **Post-Exit Shell Commands**: Automatically trigger backup scripts, cloud sync, or cleanup tasks immediately after the game process terminates.
- **Playtime Tracking**: Tracks cumulative game time in seconds (`Xh Ym`) and records the timestamp of the last active play session.

---

## Graphics Compatibility Audits

YAGO includes an integrated graphics audit engine (`audit_game_graphics`):
- **Wuthering Waves `Engine.ini` Audit**: Verifies that required streaming parameters (`r.Streaming.MinBoost`, `DistanceLODBaseFOV`) are configured to prevent texture popping and stuttering, with a 1-click "APPLY WUWA ENGINE TWEAKS" auto-fix.
- **Rogue `d3dx.ini` Scanner**: Detects conflicting `d3dx.ini` configuration files placed inside mod subfolders that override global loader settings.
- **Quality Advisories**: Recommends highest character quality settings for Genshin Impact and Zenless Zone Zero to avoid mesh LOD distortions.

---

## CDN Delta Sync & Multi-Region Support

- **Sophon Delta Patching**: Integrates `getPatchBuild` querying to download and apply LDiff delta chunk manifests when upgrading between game versions.
- **Selectable Regions**: Region selection for Global, CN, Bilibili, and SEA CDN endpoints.
- **Kuro Games CDN Client**: Streaming 1 MiB chunk MD5 verifier backed by an in-memory `(size, mtime)` cache to prevent OOM errors on large `.pak` files without redundant re-hashing on startup.
- **Post-Install Voice-Over Pack Manager**: Inspect, download, and remove multi-language audio packs (English, Japanese, Chinese, Korean, Traditional Chinese) to optimize disk space.
