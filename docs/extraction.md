# Provenance and scope

The agent identity registry originated in
[Herdr](https://github.com/herdrdev/herdr/tree/d5680d84fd1df3b592424eae5c50d54642f0727d),
revision `d5680d84fd1df3b592424eae5c50d54642f0727d` (0.9.1), chiefly
`src/detect/mod.rs`. This repository keeps its known 24 identities and common
aliases. The tracker is tailored to alacritree's event model in
`alacritree/src/process_probe.rs`, `session.rs`, and `app.rs`.

The earlier agentsense extraction also included screen manifests, OSC and hook
protocol parsers, session metadata, and transition helpers. These are removed
from the public library because alacritree already parses terminal events and
receives Herdr pane status through its own integration. The Git history retains
the extraction. Hook or pane status reaches this crate as a normalized
`StatusReport`; installing hooks and validating their transports remain host
responsibilities.

Compared with Herdr, native title completion uses alacritree's Braille spinner
heuristic and its caller-supplied grace period. A bell requests attention but
does not prove the agent is blocked. `WaitingForAction` requires a trusted
`Blocked` report. If an agent lacks usable titles and reports, agent presence
can be known while its exact work state is only an idle fallback. This is a
known evidence limit, not a promise of full lifecycle coverage for every agent.
