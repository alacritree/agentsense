# agentsense

Reusable Rust parsing extracted from Herdr. Read each module's `//!` header before
editing. Keep parsing independent of UI, PTYs, filesystem discovery, subprocesses,
networking and global mutable configuration. Callers own observations and time.

- `agentsense-core`: shared types, agent registry and process identification.
- `agentsense-signals`: OSC, session, metadata and transition parsing.
- `agentsense`: manifest engine and facade for the other crates.

Preserve upstream parser semantics and attribution. Use synthetic manifests for
engine tests. Bundled manifest schema tests do not prove compatibility with live
agent CLIs. Explain source mappings and deliberate differences in
`docs/extraction.md`. Public errors use `thiserror` and preserve parser sources.

Run `cargo fmt --all --check`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`.

Commits use Conventional Commits.
