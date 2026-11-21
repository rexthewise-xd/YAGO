# YAGO - Yet Another Game Organizer

A high-performance game mod manager built with Dioxus and Rust.

## Features

- **Multi-Game Support** - Manage mods for multiple games from a single interface
- **Smart INI Handling** - Parse and merge 3DMigoto configurations with conflict resolution
- **Profile System** - Multiple mod configurations per game with easy switching
- **Game Launching** - Launch games with Wine, Proton, or native runners
- **Mod Import** - Drag-and-drop mod import with automatic categorization
- **Modern UI** - Clean glassmorphism design with smooth animations built with Dioxus

## Supported Games

- **HoYoverse**: Genshin Impact, Honkai: Star Rail, Zenless Zone Zero
- **Wuthering Waves**

More games will be added in the future.

## Tech Stack

- **UI**: Dioxus (Desktop)
- **Backend / Core**: Rust (Edition 2021, Tokio)

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [Dioxus CLI](https://dioxuslabs.com/) (`cargo install dioxus-cli`)

### Run & Build

```bash
# Run development mode
cargo run -p yago-ui

# Or with Dioxus CLI (hot reload enabled)
cd src-ui-rs && dx serve

# Build for production
cargo build -p yago-ui --release
```

## Architecture

The core logic and UI are split into modular Rust crates:

| Crate | Purpose |
|-------|---------|
| **app** (`yago-app`) | Core application state, commands, and event system |
| **yago-ui** (`src-ui-rs`) | Dioxus desktop UI |
| **ini** | INI file parser with 3DMigoto syntax support |
| **vfs** | Virtual file system for mod deployment via symlinks |
| **storage** | Game library and database management |
| **mod_patches** | INI patching and conflict resolution |
| **launcher** | Game launching with Wine/Proton support |
| **sync** | Asset synchronization and delta updates |
| **resources** | Resource downloading and management |
| **loader** | Mod loader interface |

## Project Structure

```
yago/
├── crates/           # Rust backend crates & application core
├── src-ui-rs/        # Dioxus desktop UI
├── resources/        # App resources and game templates
└── scripts/          # Build and utility scripts
```

## Documentation

See [docs/index.md](docs/index.md) for user and developer guides.

## License

Licensed under MIT. See [LICENSE](LICENSE) for details.

YAGO is a non-destructive tool. It does not modify game binaries. Use responsibly.

**Happy Modding!**
