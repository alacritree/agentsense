// SPDX-License-Identifier: Apache-2.0
// Adapted from Herdr for agentsense; see the package NOTICE.

//! Typed parse and validation failures with the original parser error attached.

use thiserror::Error;

use crate::Agent;

/// A manifest could not be parsed, validated, or installed for an agent.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ManifestError {
    #[error("failed to parse manifest TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("manifest must contain at least one rule")]
    EmptyRules,
    #[error("manifest contains {count} rules, maximum is {max}")]
    TooManyRules { count: usize, max: usize },
    #[error("manifest rule id must not be empty")]
    EmptyRuleId,
    #[error("rule {rule}: {source}")]
    Rule {
        rule: String,
        #[source]
        source: ValidationError,
    },
    #[error("manifest id {id:?} does not match agent {agent:?}")]
    AgentMismatch { id: String, agent: Agent },
    #[error("versioned manifest must include version")]
    MissingVersion,
    #[error("versioned manifest must include min_engine_version")]
    MissingEngineVersion,
    #[error("manifest requires engine {required}, current engine is {supported}")]
    UnsupportedEngine { required: u32, supported: u32 },
}

/// A rule violates a semantic or complexity constraint.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ValidationError {
    #[error("skip_state_update requires state = \"unknown\"")]
    SkipState,
    #[error("skip_state_update cannot supply visible state evidence")]
    SkipVisibility,
    #[error("invalid region {region:?}")]
    Region { region: String },
    #[error("top_non_empty_lines requires min_engine_version of at least {required}")]
    RegionEngine { required: u32 },
    #[error("{context} exceeds maximum gate depth {max}")]
    GateDepth { context: &'static str, max: usize },
    #[error("manifest exceeds maximum gate count {max}")]
    GateCount { max: usize },
    #[error("{context} must contain a positive matcher")]
    MissingPositiveMatcher { context: &'static str },
    #[error("not gate must contain a matcher")]
    EmptyNotGate,
    #[error("{context} has {count} direct matchers, maximum is {max}")]
    MatcherCount {
        context: &'static str,
        count: usize,
        max: usize,
    },
    #[error("manifest exceeds maximum matcher count {max}")]
    TotalMatchers { max: usize },
    #[error("{context} matcher exceeds maximum length {max}")]
    MatcherLength { context: &'static str, max: usize },
    #[error("invalid {field} pattern {pattern:?}: {source}")]
    Regex {
        field: &'static str,
        pattern: String,
        #[source]
        source: regex::Error,
    },
}
