// SPDX-License-Identifier: Apache-2.0
// Adapted from Herdr; see the workspace NOTICE.

//! Track coding agents in terminal sessions from events supplied by a host.
//!
//! Keep one [`Tracker`] per terminal. The host owns process polling, the PTY,
//! terminal title decoding, pane transport, visibility, and time. An agent
//! status is reported only when process identity or an attached pane establishes
//! agent presence; a spinner or bell by itself cannot identify an agent.

mod registry;
mod tracker;
mod types;

pub use registry::{agent_label, identify_process, parse_agent_label};
pub use tracker::{Event, ReportedStatus, Snapshot, Status, StatusReport, Tracker, Update};
pub use types::Agent;
