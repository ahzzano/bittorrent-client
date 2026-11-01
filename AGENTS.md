This project is a BitTorrent client

GitHub Repo: https://github.com/ahzzano/bittorrent-client/

# Structure
## Modules
- `bencode.rs` - Bencode parser

## Implementation
- `lib.rs` - Library code. Used to export modules to the main binaries
- `gui.rs` - GUI binary for the GUI build of the app
- `cli.rs` - Used for the CLI build of the app

# Essential Commands
- Run via the CLI: `cargo run --bin bt`
- Run via the GUI: `cargo run --features gui --bin bt-gui` (the GUI is opt-in; the default build is CLI only)
- Run all tests: `cargo test --locked --all-features`
- Format: `cargo +nightly fmt` (nightly is required because `rustfmt.toml` uses `imports_granularity`; CI runs `cargo fmt --check` on nightly)

# PR Rules
- Keep PRs under 1000 lines. If the PR goes beyond 1000 lines, this means that the work is way too masive relative to the change.

    You may skip this rule if the PR means a new version release

    This rule only applies to code (.rs files), project files (Cargo.toml, rustfmt.toml, .gitignore, .github/), and agent related files (AGENTS.md). 

    Skip this check if the file change are lock files (Cargo.lock)

- When writing PRs, always use the PR template
