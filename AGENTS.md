# agentsense

A single Rust crate for tracking coding agents in terminal sessions. Read each
module's `//!` header before editing. Keep recognition and tracking independent
of UI, PTYs, filesystem discovery, subprocesses and networking. Callers own
observations, report authority and time.

The public contract is agent presence, optional identity and the four statuses:
Idle, Working, Finished, WaitingForAction. Do not infer WaitingForAction from a
bell. Keep the 24-agent list in the README aligned with `Agent::ALL`.

Preserve Herdr provenance in `NOTICE` and document deliberate behavior changes
in `docs/extraction.md`. Run `cargo fmt --all --check`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`, and
`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`.

Commits use Conventional Commits.
