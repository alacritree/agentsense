// SPDX-License-Identifier: Apache-2.0
// Derived from Herdr; see the workspace NOTICE for provenance.

//! Parse agent signals without owning a terminal, process, or transport.
//!
//! Inputs are caller-provided bytes, reports, and timestamps. Historical
//! `herdr:*` source identifiers remain intact for hook protocol compatibility.

pub mod hook;
pub mod metadata;
pub mod osc;
pub mod session;
pub mod title;
pub mod transition;

pub use osc::AgentOscStateTracker;
