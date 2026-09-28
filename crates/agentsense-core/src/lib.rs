// SPDX-License-Identifier: Apache-2.0
// Derived from Herdr; see the workspace NOTICE for provenance.

//! Agent identities and pure process parsing extracted from Herdr.
//!
//! Callers supply process snapshots; the crate performs no filesystem,
//! environment, terminal, or operating-system queries. Screen and integration
//! protocol parsing live in the other agentsense crates.

mod process;
mod registry;
mod types;

pub use process::{
    identify_agent, identify_agent_in_job, identify_agent_in_job_with_resolver,
    parse_agent_env_hint,
};
pub use registry::{
    agent_label, full_lifecycle_hook_authority, interactive_agent_executable, parse_agent_label,
    parse_canonical_agent_label, session_identity_only_integration,
};
pub use types::{Agent, AgentDetection, AgentState, ForegroundJob, ForegroundProcess};
