// SPDX-License-Identifier: Apache-2.0
// Adapted from Herdr for agentsense; see the package NOTICE.

//! Coding agent recognition and terminal state detection, without terminal ownership.
//!
//! [`Detector`] owns compiled manifests. Callers supply terminal text and optional
//! OSC title/progress strings; no detection operation reads files, environment
//! variables, processes or the network. See [`signals`] for incremental OSC parsing
//! and hook/session helpers, and [`agentsense_core`] for process identification.
//!
//! ```
//! use agentsense::{Agent, AgentState, DetectionInput, Detector};
//!
//! let detector = Detector::bundled();
//! let input = DetectionInput::screen("ordinary terminal output");
//! assert_eq!(detector.detect(None, input).state, AgentState::Unknown);
//! let explanation = detector.explain(Some(Agent::Codex), input);
//! assert_eq!(explanation.fallback_reason.as_deref(), Some("codex_state_ambiguous"));
//! ```

pub mod catalog;
mod engine;
mod error;
mod manifest;
mod regions;
mod version;

pub use agentsense_core::*;
pub use agentsense_signals as signals;
pub use engine::{
    DetectionExplain, DetectionInput, Detector, EvaluatedRule, ManifestSource, ManifestSummary,
    MatchedRule, RuleEvidence, DEFAULT_KNOWN_AGENT_IDLE_FALLBACK,
};
pub use error::{ManifestError, ValidationError};
pub use manifest::{bundled_manifest_text, Manifest, BUNDLED_MANIFESTS};
pub use version::{ManifestVersion, VersionError};

/// Highest supported manifest engine version.
pub const MANIFEST_ENGINE_VERSION: u32 = 3;

#[cfg(test)]
mod tests;
