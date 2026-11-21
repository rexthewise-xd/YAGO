# Testing Guide

YAGO uses a multi-tiered testing strategy across its workspace crates.

## Running Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p yago-app
cargo test -p yago-ui
cargo test -p ini
cargo test -p vfs
cargo test -p storage
cargo test -p launcher
```

## Test Structure

### Unit Tests

Each crate contains tests for its specific domain:

- `app`: Initialization, settings, assets, and emergency tests
- `ini`: Parser and compiler tests
- `vfs`: Archive extraction and safety tests
- `storage`: Database and discovery tests
- `mod_patches`: Merger and validator tests
- `launcher`: Launch configuration tests

### Snapshot Tests

Uses `insta` for regression testing:

```bash
cargo test --test '*snapshot*'
```

## Code Quality

```bash
# Format code
cargo fmt --all

# Format check
cargo fmt --all -- --check

# Clippy linting
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Common Commands

```bash
just fmt     # Format code
just clippy  # Lint
just test    # Run all tests
just check   # Check workspace
```
