# Safety & Streamer Mode

YAGO is designed with privacy, account security, and streamer protection as first-class architectural priorities.

---

## Streamer Mode & Privacy Protection

- **NSFW Content Blur**: Automatic tagging and visual blurring of sensitive mod cards in the gallery and mod inspector.
- **Text & Metadata Obfuscation**: Mod titles and description text can be blurred during live broadcast sessions.
- **Emergency Panic Switch (Alt+F12)**:
  - Instantly unlinks all deployed virtual mods from the game's `Mods/` directory.
  - Sends a hot-reload signal to 3DMigoto to clear all in-memory textures and meshes.
  - Displays a clean panic overlay allowing you to resume or dismiss once safe.

---

## Account Security & Non-Destructive Modding

- **Zero-Copy Virtual Deployments**: YAGO deploys mods via symbolic links (Linux) or directory junctions (Windows). The original game folder and official game archives (`.pak`, `.pck`, `.blk`) are never modified or written to.
- **Unsafe Mode Protection**: Protects custom proxy files (`d3d11.dll`, `d3dx.ini`) from being overwritten or unlinked during updates and uninstalls.
- **Protected Paths in Reparation Sweeps**: Cleanup sweeps and repair operations strictly protect user directories (e.g., `Client/Saved/` in Wuthering Waves) to preserve savegames and custom `Engine.ini` settings.
- **Clean Mod Export**: Removes internal tracking and backup files (`.yago_backups/`) when sharing or publishing mods.

---

## Wish / Signal / Convene URL Security

- YAGO's 1-Click Wish URL extractor reads temporary gacha authentication tokens from Chromium `data_2` web caches locally on your machine.
- Extracted URLs are copied directly to your clipboard or displayed in a local modal; tokens are never uploaded or transmitted to external servers.

---

## Disclaimer

YAGO is an open-source tool provided for personal customization.
1. You use this software at your own discretion.
2. Modifying game visuals may violate the Terms of Service of certain online game publishers.
3. The developers are not liable for any actions taken on user accounts.
4. Review the [LICENSE](https://github.com/rexthewise-xd/YAGO/blob/main/LICENSE) for warranty and terms.
