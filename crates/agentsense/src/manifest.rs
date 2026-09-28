// SPDX-License-Identifier: Apache-2.0
// Adapted from Herdr for agentsense; see the package NOTICE.

//! Parse, validate and compile the manifest rule language before installation.
//! Compiled matchers belong to each manifest; parsing never reads files or globals.

use regex::Regex;
use serde::Deserialize;

use crate::error::{ManifestError, ValidationError};
use crate::regions::{region_count, top_region_count, TOP_NON_EMPTY_LINES_ENGINE_VERSION};
use crate::{agent_label, parse_agent_label, Agent, AgentState, ManifestVersion};
use crate::{RuleEvidence, MANIFEST_ENGINE_VERSION};

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct AgentManifest {
    id: String,
    version: Option<ManifestVersion>,
    min_engine_version: Option<u32>,
    updated_at: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    rules: Vec<ManifestRule>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManifestRule {
    pub(crate) id: String,
    pub(crate) state: Option<ManifestState>,
    #[serde(default)]
    pub(crate) priority: i32,
    #[serde(default = "default_region")]
    pub(crate) region: String,
    #[serde(default)]
    pub(crate) visible_idle: bool,
    #[serde(default)]
    pub(crate) visible_blocker: bool,
    #[serde(default)]
    pub(crate) visible_working: bool,
    #[serde(default)]
    pub(crate) skip_state_update: bool,
    #[serde(default)]
    all: Vec<ManifestGate>,
    #[serde(default)]
    any: Vec<ManifestGate>,
    #[serde(default, rename = "not")]
    not_gate: Vec<ManifestGate>,
    #[serde(default)]
    contains: Vec<String>,
    #[serde(default)]
    regex: Vec<String>,
    #[serde(default)]
    line_regex: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct ManifestGate {
    #[serde(default)]
    all: Vec<ManifestGate>,
    #[serde(default)]
    any: Vec<ManifestGate>,
    #[serde(default, rename = "not")]
    not_gate: Vec<ManifestGate>,
    #[serde(default)]
    contains: Vec<String>,
    #[serde(default)]
    regex: Vec<String>,
    #[serde(default)]
    line_regex: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledRule {
    gate: CompiledGate,
}

#[derive(Debug, Clone)]
struct CompiledGate {
    all: Vec<CompiledGate>,
    any: Vec<CompiledGate>,
    not_gate: Vec<CompiledGate>,
    contains: Vec<String>,
    regex: Vec<Regex>,
    line_regex: Vec<Regex>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ManifestState {
    Idle,
    Working,
    Blocked,
    Unknown,
}

impl From<ManifestState> for AgentState {
    fn from(value: ManifestState) -> Self {
        match value {
            ManifestState::Idle => AgentState::Idle,
            ManifestState::Working => AgentState::Working,
            ManifestState::Blocked => AgentState::Blocked,
            ManifestState::Unknown => AgentState::Unknown,
        }
    }
}

fn default_region() -> String {
    "whole_recent".to_string()
}

/// Bundled `(canonical agent label, TOML source)` pairs.
pub const BUNDLED_MANIFESTS: &[(&str, &str)] = &[
    ("amp", include_str!("manifests/amp.toml")),
    ("agy", include_str!("manifests/antigravity.toml")),
    ("claude", include_str!("manifests/claude.toml")),
    ("cline", include_str!("manifests/cline.toml")),
    ("codex", include_str!("manifests/codex.toml")),
    ("cursor", include_str!("manifests/cursor.toml")),
    ("devin", include_str!("manifests/devin.toml")),
    ("droid", include_str!("manifests/droid.toml")),
    ("gemini", include_str!("manifests/gemini.toml")),
    ("grok", include_str!("manifests/grok.toml")),
    ("hermes", include_str!("manifests/hermes.toml")),
    ("kilo", include_str!("manifests/kilo.toml")),
    ("kimi", include_str!("manifests/kimi.toml")),
    ("kiro", include_str!("manifests/kiro.toml")),
    ("letta", include_str!("manifests/letta.toml")),
    ("maki", include_str!("manifests/maki.toml")),
    ("muse", include_str!("manifests/muse.toml")),
    ("opencode", include_str!("manifests/opencode.toml")),
    ("pi", include_str!("manifests/pi.toml")),
    ("qodercli", include_str!("manifests/qodercli.toml")),
    ("qwen", include_str!("manifests/qwen.toml")),
    ("copilot", include_str!("manifests/github-copilot.toml")),
];

fn manifest_matches_agent(manifest: &AgentManifest, agent: Agent) -> bool {
    let id = agent_label(agent);
    manifest.id == id
        || manifest.aliases.iter().any(|alias| alias == id)
        || parse_agent_label(&manifest.id) == Some(agent)
        || manifest
            .aliases
            .iter()
            .any(|alias| parse_agent_label(alias) == Some(agent))
}

fn manifest_gate_from_rule(rule: &ManifestRule) -> ManifestGate {
    ManifestGate {
        all: rule.all.clone(),
        any: rule.any.clone(),
        not_gate: rule.not_gate.clone(),
        contains: rule.contains.clone(),
        regex: rule.regex.clone(),
        line_regex: rule.line_regex.clone(),
    }
}

fn compile_manifest(manifest: &AgentManifest) -> Result<Vec<CompiledRule>, ManifestError> {
    manifest
        .rules
        .iter()
        .map(|rule| {
            compile_gate(&manifest_gate_from_rule(rule))
                .map(|gate| CompiledRule { gate })
                .map_err(|source| ManifestError::Rule {
                    rule: rule.id.clone(),
                    source,
                })
        })
        .collect()
}

fn compile_gate(gate: &ManifestGate) -> Result<CompiledGate, ValidationError> {
    Ok(CompiledGate {
        all: gate
            .all
            .iter()
            .map(compile_gate)
            .collect::<Result<_, _>>()?,
        any: gate
            .any
            .iter()
            .map(compile_gate)
            .collect::<Result<_, _>>()?,
        not_gate: gate
            .not_gate
            .iter()
            .map(compile_gate)
            .collect::<Result<_, _>>()?,
        contains: gate
            .contains
            .iter()
            .map(|needle| needle.to_lowercase())
            .collect(),
        regex: compile_patterns(&gate.regex, "regex")?,
        line_regex: compile_patterns(&gate.line_regex, "line_regex")?,
    })
}

fn compile_patterns(
    patterns: &[String],
    field: &'static str,
) -> Result<Vec<Regex>, ValidationError> {
    patterns
        .iter()
        .map(|pattern| {
            Regex::new(pattern).map_err(|source| ValidationError::Regex {
                field,
                pattern: pattern.clone(),
                source,
            })
        })
        .collect()
}

pub(crate) fn compiled_rule_matches(rule: &CompiledRule, text: &str) -> bool {
    let lower_text = text.to_lowercase();
    compiled_gate_matches(&rule.gate, text, &lower_text)
}

pub(crate) fn rule_evidence(rule: &ManifestRule, region_text: &str) -> RuleEvidence {
    RuleEvidence {
        contains: rule.contains.clone(),
        regex: rule.regex.clone(),
        line_regex: rule.line_regex.clone(),
        all_count: rule.all.len(),
        any_count: rule.any.len(),
        not_count: rule.not_gate.len(),
        region_bytes: region_text.len(),
        region_preview: bounded_preview(region_text),
    }
}

fn bounded_preview(text: &str) -> String {
    const MAX_CHARS: usize = 240;
    let mut preview: String = text.chars().take(MAX_CHARS).collect();
    if text.chars().count() > MAX_CHARS {
        preview.push_str("...");
    }
    preview
}

fn compiled_gate_matches(gate: &CompiledGate, text: &str, lower_text: &str) -> bool {
    if !gate
        .contains
        .iter()
        .all(|needle| lower_text.contains(needle))
    {
        return false;
    }

    if !gate.regex.iter().all(|regex| regex.is_match(text)) {
        return false;
    }

    if !gate
        .line_regex
        .iter()
        .all(|regex| text.lines().any(|line| regex.is_match(line)))
    {
        return false;
    }

    if !gate
        .all
        .iter()
        .all(|nested| compiled_gate_matches(nested, text, lower_text))
    {
        return false;
    }

    if !gate.any.is_empty()
        && !gate
            .any
            .iter()
            .any(|nested| compiled_gate_matches(nested, text, lower_text))
    {
        return false;
    }

    if gate
        .not_gate
        .iter()
        .any(|nested| compiled_gate_matches(nested, text, lower_text))
    {
        return false;
    }

    true
}

/// Engine limits apply across the entire manifest, including nested gates.
const MAX_RULES_PER_MANIFEST: usize = 128;
const MAX_GATE_DEPTH: usize = 8;
const MAX_TOTAL_GATES: usize = 512;
const MAX_MATCHERS_PER_GATE: usize = 32;
const MAX_TOTAL_MATCHERS: usize = 1024;
const MAX_MATCHER_CHARS: usize = 512;

/// A validated manifest with compiled regular expressions.
///
/// Parsing compiles matchers once. Detection borrows them without reparsing.
#[derive(Debug, Clone)]
pub struct Manifest {
    data: AgentManifest,
    pub(crate) compiled_rules: Vec<CompiledRule>,
}

impl Manifest {
    /// Parse and validate a TOML manifest, including any declared engine version.
    pub fn parse(content: &str) -> Result<Self, ManifestError> {
        let data: AgentManifest = toml::from_str(content)?;
        validate_manifest(&data)?;
        let compiled_rules = compile_manifest(&data)?;
        Ok(Self {
            data,
            compiled_rules,
        })
    }

    /// Parse a manifest and verify its ID or aliases name the intended agent.
    pub fn parse_for_agent(agent: Agent, content: &str) -> Result<Self, ManifestError> {
        let manifest = Self::parse(content)?;
        manifest.validate_agent(agent)?;
        Ok(manifest)
    }

    /// Parse a distributable manifest, requiring version and engine metadata.
    ///
    /// Fetching, version selection and persistence remain the caller's policy.
    pub fn parse_versioned(agent: Agent, content: &str) -> Result<Self, ManifestError> {
        let manifest = Self::parse_for_agent(agent, content)?;
        if manifest.version().is_none() {
            return Err(ManifestError::MissingVersion);
        }
        if manifest.min_engine_version().is_none() {
            return Err(ManifestError::MissingEngineVersion);
        }
        Ok(manifest)
    }

    pub fn id(&self) -> &str {
        &self.data.id
    }

    pub fn aliases(&self) -> &[String] {
        &self.data.aliases
    }

    pub fn version(&self) -> Option<&ManifestVersion> {
        self.data.version.as_ref()
    }

    pub fn updated_at(&self) -> Option<&str> {
        self.data.updated_at.as_deref()
    }

    pub fn min_engine_version(&self) -> Option<u32> {
        self.data.min_engine_version
    }

    pub fn rule_count(&self) -> usize {
        self.data.rules.len()
    }

    pub(crate) fn rules(&self) -> &[ManifestRule] {
        &self.data.rules
    }

    pub(crate) fn validate_agent(&self, agent: Agent) -> Result<(), ManifestError> {
        if manifest_matches_agent(&self.data, agent) {
            Ok(())
        } else {
            Err(ManifestError::AgentMismatch {
                id: self.data.id.clone(),
                agent,
            })
        }
    }
}

/// The bundled manifest source for an agent, without loading or compiling it.
pub fn bundled_manifest_text(agent: Agent) -> Option<&'static str> {
    BUNDLED_MANIFESTS
        .iter()
        .find(|(id, _)| *id == agent_label(agent))
        .map(|(_, content)| *content)
}

pub(crate) fn bundled_manifest(agent: Agent) -> Option<Manifest> {
    bundled_manifest_text(agent).map(|content| {
        Manifest::parse_for_agent(agent, content).expect("bundled manifest is valid")
    })
}

fn validate_manifest(manifest: &AgentManifest) -> Result<(), ManifestError> {
    if manifest.rules.is_empty() {
        return Err(ManifestError::EmptyRules);
    }
    if manifest.rules.len() > MAX_RULES_PER_MANIFEST {
        return Err(ManifestError::TooManyRules {
            count: manifest.rules.len(),
            max: MAX_RULES_PER_MANIFEST,
        });
    }
    if let Some(required) = manifest
        .min_engine_version
        .filter(|version| *version > MANIFEST_ENGINE_VERSION)
    {
        return Err(ManifestError::UnsupportedEngine {
            required,
            supported: MANIFEST_ENGINE_VERSION,
        });
    }
    let mut complexity = ManifestComplexity::default();
    for rule in &manifest.rules {
        if rule.id.trim().is_empty() {
            return Err(ManifestError::EmptyRuleId);
        }
        validate_rule(rule, manifest.min_engine_version, &mut complexity).map_err(|source| {
            ManifestError::Rule {
                rule: rule.id.clone(),
                source,
            }
        })?;
    }
    Ok(())
}

fn validate_rule(
    rule: &ManifestRule,
    min_engine_version: Option<u32>,
    complexity: &mut ManifestComplexity,
) -> Result<(), ValidationError> {
    if rule.skip_state_update {
        if rule.state != Some(ManifestState::Unknown) {
            return Err(ValidationError::SkipState);
        }
        if rule.visible_idle || rule.visible_blocker || rule.visible_working {
            return Err(ValidationError::SkipVisibility);
        }
    }
    validate_region_name(&rule.region)?;
    if rule.region.trim().starts_with("top_non_empty_lines(")
        && min_engine_version.is_some_and(|version| version < TOP_NON_EMPTY_LINES_ENGINE_VERSION)
    {
        return Err(ValidationError::RegionEngine {
            required: TOP_NON_EMPTY_LINES_ENGINE_VERSION,
        });
    }
    validate_gate(&manifest_gate_from_rule(rule), "rule", 0, complexity)
}

#[derive(Default)]
struct ManifestComplexity {
    total_gates: usize,
    total_matchers: usize,
}

fn validate_gate(
    gate: &ManifestGate,
    context: &'static str,
    depth: usize,
    complexity: &mut ManifestComplexity,
) -> Result<(), ValidationError> {
    validate_gate_limits(gate, context, depth, complexity)?;
    if !gate_has_positive_matcher(gate) {
        return Err(ValidationError::MissingPositiveMatcher { context });
    }
    for nested in &gate.all {
        validate_gate(nested, "all gate", depth + 1, complexity)?;
    }
    for nested in &gate.any {
        validate_gate(nested, "any gate", depth + 1, complexity)?;
    }
    for nested in &gate.not_gate {
        validate_not_gate(nested, depth + 1, complexity)?;
    }
    Ok(())
}

fn validate_not_gate(
    gate: &ManifestGate,
    depth: usize,
    complexity: &mut ManifestComplexity,
) -> Result<(), ValidationError> {
    validate_gate_limits(gate, "not gate", depth, complexity)?;
    if !gate_has_any_matcher(gate) {
        return Err(ValidationError::EmptyNotGate);
    }
    for nested in &gate.all {
        validate_gate(nested, "not all gate", depth + 1, complexity)?;
    }
    for nested in &gate.any {
        validate_gate(nested, "not any gate", depth + 1, complexity)?;
    }
    for nested in &gate.not_gate {
        validate_not_gate(nested, depth + 1, complexity)?;
    }
    Ok(())
}

fn validate_gate_limits(
    gate: &ManifestGate,
    context: &'static str,
    depth: usize,
    complexity: &mut ManifestComplexity,
) -> Result<(), ValidationError> {
    if depth > MAX_GATE_DEPTH {
        return Err(ValidationError::GateDepth {
            context,
            max: MAX_GATE_DEPTH,
        });
    }
    complexity.total_gates += 1;
    if complexity.total_gates > MAX_TOTAL_GATES {
        return Err(ValidationError::GateCount {
            max: MAX_TOTAL_GATES,
        });
    }
    let count = gate.contains.len() + gate.regex.len() + gate.line_regex.len();
    if count > MAX_MATCHERS_PER_GATE {
        return Err(ValidationError::MatcherCount {
            context,
            count,
            max: MAX_MATCHERS_PER_GATE,
        });
    }
    complexity.total_matchers += count;
    if complexity.total_matchers > MAX_TOTAL_MATCHERS {
        return Err(ValidationError::TotalMatchers {
            max: MAX_TOTAL_MATCHERS,
        });
    }
    for value in gate
        .contains
        .iter()
        .chain(&gate.regex)
        .chain(&gate.line_regex)
    {
        if value.chars().count() > MAX_MATCHER_CHARS {
            return Err(ValidationError::MatcherLength {
                context,
                max: MAX_MATCHER_CHARS,
            });
        }
    }
    Ok(())
}

fn gate_has_positive_matcher(gate: &ManifestGate) -> bool {
    !gate.contains.is_empty()
        || !gate.regex.is_empty()
        || !gate.line_regex.is_empty()
        || !gate.all.is_empty()
        || !gate.any.is_empty()
}

fn gate_has_any_matcher(gate: &ManifestGate) -> bool {
    gate_has_positive_matcher(gate) || !gate.not_gate.is_empty()
}

pub(crate) fn validate_region_name(spec: &str) -> Result<(), ValidationError> {
    let trimmed = spec.trim();
    match trimmed {
        "whole_recent"
        | "after_last_prompt_marker"
        | "before_current_prompt_marker"
        | "whole_recent_without_current_prompt_marker"
        | "current_prompt_block_marker"
        | "after_current_prompt_block_marker"
        | "prompt_box_body"
        | "above_prompt_box"
        | "last_non_empty_above_prompt_box"
        | "after_last_horizontal_rule"
        | "osc_title"
        | "osc_progress" => Ok(()),
        _ if region_count(trimmed, "bottom_lines").is_some()
            || region_count(trimmed, "bottom_non_empty_lines").is_some()
            || top_region_count(trimmed).is_some() =>
        {
            Ok(())
        }
        _ => Err(ValidationError::Region {
            region: trimmed.to_owned(),
        }),
    }
}
