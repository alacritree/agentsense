# agentsense

A small Rust crate for tracking coding agents in terminal sessions. It answers
whether an agent is present, which of 24 known agents it is when recognizable,
and whether it is idle, working, finished, or waiting for action.

Alacritree supplies foreground process observations, accepted terminal title
changes, bells, visibility, time, and optional status reports from hooks or a
pane owner such as Herdr. The crate holds one in-memory tracker per terminal.
It does not inspect processes, own a PTY, parse raw escape sequences, install
hooks, or connect to Herdr.

## Event requirements

| Input from alacritree | Why it is needed |
| --- | --- |
| Foreground process identity | Establish agent presence and identify a known agent. Re-probe when the foreground job changes. |
| Terminal title changes | A Braille spinner indicates working; its disappearance starts a possible completion. Pass only titles accepted by the host. |
| Bell | Records a request for attention. A bell alone does not mean the agent is waiting for action. |
| Trusted status report | A hook or pane owner can say idle, working, blocked, done, or unknown. `blocked` maps to waiting for action. |
| Visibility and focus | Clear completed attention when the user views the terminal. |
| Monotonic time and a timer wake-up | Let a completion settle through a grace period; cancel it if work resumes. |
| Child exit | Discard stale identity and state for the ended terminal. |

The host should map `Snapshot::status` to its sidebar. `None` means the state
is unknown or no agent is present; use `agent_present` to distinguish those
cases. Native process presence alone uses an idle fallback, matching
alacritree's current behavior. Accurate waiting-for-action detection needs a
trusted status report. Native completion is a title heuristic and can be
superseded by a report.

## Use

Requires Rust 1.85 or newer. This crate is available from Git and has not been
published to crates.io.

```toml
[dependencies]
agentsense = { git = "https://github.com/alacritree/agentsense" }
```

```rust
use agentsense::{identify_process, Event, Status, Tracker};
use std::time::{Duration, Instant};

let mut tracker = Tracker::new(); // one per terminal
let now = Instant::now();
let grace = Duration::from_millis(500);
let agent = identify_process(Some("claude"), Some("claude"));
tracker.observe(Event::Process(agent), now, grace);
tracker.observe(Event::Title("⠋ Claude"), now, grace);
assert_eq!(tracker.snapshot().status, Some(Status::Working));
```

For a hook or Herdr status, send `Event::Report(Some(StatusReport { agent,
status }))`. Send `Event::Report(None)` when the source detaches. The host decides
which report to trust when multiple sources exist. Pass `Event::Tick` after
`Update::next_check` and notify only when `Update::notify` is true.

## Supported agents

The registry recognizes these **24 agents**:

| Agent | Agent | Agent |
| --- | --- | --- |
| Amp | Antigravity | Claude Code |
| Cline | Codex | Cursor |
| Devin | Droid | Gemini |
| GitHub Copilot | Grok | Hermes |
| Kilo | Kimi | Kiro |
| Letta | Maki | Mastra Code |
| Muse | OpenCode | OMP |
| Pi | Qoder CLI | Qwen |

Identity support does not imply that every agent emits usable titles or offers
hooks. A status report is the reliable route to `WaitingForAction`. The process
recognizer accepts the canonical executable and known aliases; callers can
also pass an `Agent` directly when their own probe resolves a wrapper.

## Develop

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

Apache-2.0. The agent registry was adapted from
[Herdr](https://github.com/herdrdev/herdr). See [NOTICE](NOTICE) and
[extraction notes](docs/extraction.md).
