# agentsense

Rust libraries for recognizing coding agents and interpreting their terminal
signals. Extracted from [Herdr](https://github.com/herdrdev/herdr), maintained
under [alacritree](https://github.com/alacritree).

Pass in process snapshots, live terminal text, OSC bytes or hook reports. Get
agent identity, state evidence, matched rules and validated session metadata.
The libraries perform no process polling, terminal emulation or network access.

## Crates

| Crate | Purpose |
| --- | --- |
| `agentsense` | Main API: screen manifests, detector, explanations and re-exports |
| `agentsense-core` | Shared types, 24 agent identities, process/command parsing |
| `agentsense-signals` | Streaming OSC, hook reports, sessions, metadata and state transition helpers |

The main crate re-exports the shared types and exposes `agentsense::signals`.
Each crate can also be used on its own.

## Use

Requires Rust 1.85 or newer. The crates are available from this repository;
they have not been published to crates.io.

```toml
[dependencies]
agentsense = { git = "https://github.com/alacritree/agentsense" }
```

```rust
use agentsense::{Agent, DetectionInput, Detector};

// Construct once; compiled rules are reused across observations.
let detector = Detector::bundled();
let input = DetectionInput::screen("current bottom-of-buffer text");
let evidence = detector.detect(Some(Agent::Claude), input);
let explanation = detector.explain(Some(Agent::Claude), input);
println!("{:?}: {:?}", evidence.state, explanation.matched_rule);
```

Use plain text from the terminal's live buffer, not raw PTY bytes or a scrolled
viewport. Agent identity comes from process information or an explicit host
hint. The screen rules determine state after identity is known.

### Track OSC title and progress

```rust
use agentsense::{Agent, DetectionInput, Detector};
use agentsense::signals::AgentOscStateTracker;

let mut osc = AgentOscStateTracker::default();
osc.observe(b"\x1b]2;agent title\x07");
osc.observe(b"\x1b]9;4;1;50\x07");
let input = DetectionInput {
    screen: "live terminal text",
    osc_title: osc.latest_title(),
    osc_progress: osc.latest_progress(),
};
let evidence = Detector::bundled().detect(Some(Agent::Codex), input);
```

The OSC parser accepts arbitrary chunk boundaries. Pass all PTY chunks through
it and reset retained evidence when the running agent changes. Its `observe`
return value reports changes to the presentation title; read progress separately.

### Supply custom rules

```rust
use agentsense::{Agent, AgentState, DetectionInput, Detector};

let mut detector = Detector::empty();
detector.set_manifest(Agent::Codex, r#"
id = "codex"
[[rules]]
id = "approval"
state = "blocked"
visible_blocker = true
region = "bottom_non_empty_lines(2)"
contains = ["approve this action"]
"#).unwrap();
let result = detector.detect(
    Some(Agent::Codex),
    DetectionInput::screen("Approve this action"),
);
assert_eq!(result.state, AgentState::Blocked);
```

Manifests support nested AND/OR/NOT gates, substring and regex matchers, screen
regions, OSC inputs, priorities and history-view suppression. Invalid changes
return typed errors and leave the detector's active rules intact. Instances have
independent configuration.

## Scope

The registry includes Claude Code, Codex, Gemini, Cursor, Pi, OpenCode, GitHub
Copilot and 17 other agent kinds. There are 22 bundled screen manifests; Omp and
Mastracode use hook signals. The rules are a pinned snapshot of Herdr's behavior,
and future agent releases may require updates.

A screen result is evidence. Hosts still own hook authority, process lifetimes,
session replacement and final state arbitration. `skip_state_update` means a
history view should preserve the previous state. The transition helpers apply
that rule and confirm ambiguous working-to-idle changes using caller timestamps.

See [extraction notes](docs/extraction.md) for the source revision, complete
source map, behavior differences and integration boundaries.

To inspect a captured live screen from this checkout:

```sh
cargo run -p agentsense --example read_screen -- codex < screen.txt
```

## Develop

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

CI runs tests on Linux, macOS and Windows, plus a Rust 1.85 compatibility check.
Tests cover parser semantics; they do not substitute for testing live agent CLIs.

## License

Apache-2.0. Includes code and manifests adapted from Herdr. See [LICENSE](LICENSE)
and [NOTICE](NOTICE) for attribution.
