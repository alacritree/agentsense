# Extraction from Herdr

Source: [herdrdev/herdr](https://github.com/herdrdev/herdr/tree/d5680d84fd1df3b592424eae5c50d54642f0727d),
revision `d5680d84fd1df3b592424eae5c50d54642f0727d` (package version 0.9.1).
The upstream checkout is a reference; this project does not modify Herdr.

## Source map

| Herdr source | agentsense destination | Responsibility |
| --- | --- | --- |
| `src/detect/mod.rs` | `agentsense-core` | Agent registry, labels, process names, command lines, wrapper identification and hook source capabilities |
| `src/platform/mod.rs` | `agentsense-core` | Plain foreground process/job inputs and agent environment hints |
| `src/detect/manifest.rs` | `agentsense` | TOML validation, compiled rules, region extraction, matching, priorities and explanations |
| `src/detect/manifests/*.toml` | `agentsense/src/manifests/` | All 22 bundled screen manifests, unchanged |
| `src/detect/manifest_update.rs` | `agentsense` | Numeric manifest versions and catalog parsing |
| `src/pane/osc.rs` | `agentsense-signals::osc` | Bounded, incremental OSC title/progress parser |
| `src/pane/agent_detection.rs` | `agentsense-signals::transition` | Idle confirmation, publication decisions, change sequences and Codex startup recognition |
| `src/agent_resume.rs` | `agentsense-signals::session` | Session references, launch/resume arguments, source identity and resume plans |
| `src/metadata_tokens.rs` | `agentsense-signals::metadata` | Metadata patches, sequence freshness and token expiry |
| `src/app/api_helpers.rs`, `src/app/api/panes.rs` | `agentsense-signals` | Hook labels and metadata request normalization/validation |
| `src/terminal/title.rs` | `agentsense-signals::title` | Activity glyph and title normalization |

## Library boundaries

The caller supplies live terminal text, process information, output bytes and
clock values. Use the terminal's current bottom-of-buffer snapshot; a viewport
that the user has scrolled back is not current agent evidence. Screen input is
plain rendered text, not a raw PTY byte stream. OSC tracking can consume raw
bytes separately. A terminal emulator remains responsible for escape sequences,
carriage returns, alternate screens and the grid.

Each `Detector` owns its manifests. Construct it once and reuse the compiled
rules. Installing a supplied manifest is explicit and affects only that
instance. Parsing failure leaves its prior rules in place. Callers decide which
bundled, downloaded or local text to supply. There is no implicit config search,
network updater, process polling or environment mutation.

Process parsing accepts caller-collected foreground jobs. Symlink resolution is
an explicit caller callback. Cross-platform command syntax is parsed on every
platform; agentsense does not enumerate OS processes or spawn commands.

The original `herdr:*` hook source identifiers are protocol values and remain
unchanged. Parsing them does not authenticate the sender. Applications own the
transport and decide which input sources to trust. A generated resume plan is
an argument vector; agentsense never executes it.

## Behavior preserved

The extraction retains agent aliases and wrapper handling; AND/OR/NOT gates;
case-insensitive substring matching; line-wise regex matching; all upstream screen
regions; rule priority and tie order; visible-state evidence; history-view
suppression; manifest complexity limits; and Codex's ambiguous-state fallback.
Other known agents keep the upstream idle fallback. An unidentified process
produces `Unknown`; text alone does not identify an agent.

Publication helpers enforce history-view suppression themselves. Herdr applied
that check in its caller. An observed process exit still takes precedence. This
makes the public helper safe to call directly while preserving the upstream
pipeline's behavior.

Two inherited boundary issues are corrected: resume planning revalidates public
or deserialized session references before producing arguments, and an oversized
OSC string ending with ESC BEL releases discard mode before the next sequence.
Screen region offsets count original line-ending bytes, fixing upstream CRLF
slicing that could cut through a Unicode character. These changes have
regression tests. Supplied manifests now reject any declared
engine version newer than the library supports; upstream enforced that policy
only on downloaded manifests. Catalog paths also reject backslashes and colon/drive
syntax on every platform, keeping downloaded paths relative to the host's catalog root. Explanations are a Rust library model with serde
support, not a drop-in replacement for Herdr's CLI JSON schema; state enum names
serialize with the core `AgentState` casing. Title normalization can also explicitly
strip Windows elevation decoration when a host inspects a remote Windows pane.

Unit tests are adapted from upstream parser tests and extended at the new public
boundaries. Synthetic manifests verify engine semantics. Bundled manifests are
validated as a set. These checks do not establish compatibility with future
versions of each agent CLI.

## Host responsibilities

Herdr's UI, PTYs, process collectors, event loop, integration installers,
transport/schema routing and automatic manifest distribution stay outside the
library. Its `TerminalState` tracks live hook authority, process lifetimes,
managed launches and stale-session suppression. That application state machine
is not part of this parser extraction. The agent-side shell/JS/TS/Python hook
adapters in `src/integration/assets/` also remain with Herdr; they install and
transport reports whose Rust parsing is extracted here. Callers combine parsed screen evidence
with hook authority and process state according to their own runtime policy.

There is no transcript/conversation parser in this source set. Metadata
“tokens” are named presentation values, not language-model token usage.

## Updating from upstream

1. Record the next source revision and compare the source-map files.
2. Port parser behavior changes and their tests.
3. Copy changed bundled manifests without silently altering their rule semantics.
4. Run the workspace tests, lint, docs and packaging checks.
5. Update this document and `NOTICE` in the root and each package.

The code remains Apache-2.0. `LICENSE` and `NOTICE` accompany each crate.
