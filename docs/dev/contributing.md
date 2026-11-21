# Contributing

Thank you for your interest in contributing to YAGO!

## Setup

```bash
# Clone the repository
git clone https://github.com/rexthewise-xd/YAGO.git
cd YAGO

# Run development server (Dioxus Desktop UI)
cargo run -p yago-ui

# Or with Dioxus CLI
cd src-ui-rs && dx serve
```

## Code Standards

### Rust & Dioxus

- Run `cargo fmt --all` before committing
- Run `cargo clippy --workspace -- -D warnings` to catch issues
- Add tests for new functionality
- Follow Dioxus 0.7 idioms for UI components and hooks

## Workflow

1. **Fork** the repository
2. **Branch**: Create a feature branch (`git checkout -b feature/my-feature`)
3. **Code**: Make your changes
4. **Test**: Verify with `cargo test` and `cargo check --workspace`
5. **PR**: Open a pull request

## Project Structure

```
yago/
├── crates/           # Rust backend & core application crates
│   ├── app/          # App state, commands, events, init
│   ├── ini/          # INI parsing
│   ├── vfs/          # Virtual file system
│   ├── storage/      # Database
│   ├── mod_patches/  # INI patching
│   ├── launcher/     # Game launching
│   ├── sync/         # Asset sync
│   ├── resources/    # Downloads
│   └── loader/       # Mod loader
├── src-ui-rs/        # Dioxus desktop UI application
└── justfile          # Development commands
```

## Reporting Issues

Include:
- Steps to reproduce
- Expected vs actual behavior
- Environment (OS, Rust version)
