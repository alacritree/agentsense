// SPDX-License-Identifier: Apache-2.0
// Adapted from Herdr for agentsense; see the package NOTICE.

//! Instance-owned manifest selection and evaluation.
//! Highest priority wins; equal priorities retain the first matching rule.

use serde::Serialize;

use crate::manifest::{bundled_manifest, compiled_rule_matches, rule_evidence};
use crate::regions::region;
use crate::{
    agent_label, parse_agent_label, Agent, AgentDetection, AgentState, Manifest, ManifestError,
    ManifestVersion,
};

/// Explanation reason when a known non-Codex agent has no matching rule.
pub const DEFAULT_KNOWN_AGENT_IDLE_FALLBACK: &str = "default_known_agent_idle_fallback";

/// A terminal snapshot and optional OSC-derived strings.
///
/// `screen` is already-rendered terminal text. Feed raw escape sequences through
/// the terminal emulator first. Missing OSC inputs should be empty strings.
#[derive(Debug, Clone, Copy, Default)]
pub struct DetectionInput<'a> {
    pub screen: &'a str,
    pub osc_title: &'a str,
    pub osc_progress: &'a str,
}

impl<'a> DetectionInput<'a> {
    pub const fn screen(screen: &'a str) -> Self {
        Self {
            screen,
            osc_title: "",
            osc_progress: "",
        }
    }
}

impl<'a> From<&'a str> for DetectionInput<'a> {
    fn from(screen: &'a str) -> Self {
        Self::screen(screen)
    }
}

/// Origin of the active in-memory manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestSource {
    Bundled,
    Supplied,
}

/// Active manifest metadata; no remote state or filesystem location is inferred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestSummary {
    pub agent: Agent,
    pub source: ManifestSource,
    pub version: Option<ManifestVersion>,
    pub rule_count: usize,
}

/// Evidence for every rule, including the rule selected by priority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DetectionExplain {
    pub agent: Option<String>,
    pub state: AgentState,
    pub source: Option<ManifestSource>,
    pub matched_rule: Option<MatchedRule>,
    pub visible_idle: bool,
    pub visible_blocker: bool,
    pub visible_working: bool,
    pub skip_state_update: bool,
    pub skipped_update_reason: Option<String>,
    pub fallback_reason: Option<String>,
    pub evaluated_rules: Vec<EvaluatedRule>,
    pub manifest_version: Option<ManifestVersion>,
}

impl DetectionExplain {
    pub fn detection(&self) -> AgentDetection {
        AgentDetection {
            state: self.state,
            skip_state_update: self.skip_state_update,
            visible_idle: self.visible_idle,
            visible_blocker: self.visible_blocker,
            visible_working: self.visible_working,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatchedRule {
    pub id: String,
    pub priority: i32,
    pub region: String,
    pub state: AgentState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvaluatedRule {
    pub id: String,
    pub priority: i32,
    pub region: String,
    pub evidence: RuleEvidence,
    pub state: AgentState,
    pub matched: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleEvidence {
    pub contains: Vec<String>,
    pub regex: Vec<String>,
    pub line_regex: Vec<String>,
    pub all_count: usize,
    pub any_count: usize,
    pub not_count: usize,
    pub region_bytes: usize,
    /// At most 240 characters plus an ellipsis.
    pub region_preview: String,
}

#[derive(Debug, Clone)]
struct LoadedManifest {
    agent: Agent,
    manifest: Manifest,
    source: ManifestSource,
}

/// Reusable detector with its own compiled rules and explicit replacement policy.
///
/// Clones are independent; changing one detector never affects another. Shared
/// read-only use across threads needs no application-level lock.
#[derive(Debug, Clone)]
pub struct Detector {
    manifests: Vec<LoadedManifest>,
}

impl Default for Detector {
    fn default() -> Self {
        Self::bundled()
    }
}

impl Detector {
    /// Construct a detector with the bundled manifest set.
    pub fn new() -> Self {
        Self::bundled()
    }

    /// Compile all bundled manifests once for this detector.
    pub fn bundled() -> Self {
        Self {
            manifests: Agent::SCREEN_MANIFEST_AGENTS
                .into_iter()
                .map(|agent| LoadedManifest {
                    agent,
                    manifest: bundled_manifest(agent).expect("agent has a bundled manifest"),
                    source: ManifestSource::Bundled,
                })
                .collect(),
        }
    }

    /// Construct an empty detector. Known-agent fallback semantics still apply.
    pub const fn empty() -> Self {
        Self {
            manifests: Vec::new(),
        }
    }

    /// Validate and replace one manifest. A failure leaves the current one intact.
    pub fn set_manifest(&mut self, agent: Agent, content: &str) -> Result<(), ManifestError> {
        self.install_manifest(agent, Manifest::parse_for_agent(agent, content)?)
    }

    /// Install a previously parsed manifest after verifying its agent identity.
    pub fn install_manifest(
        &mut self,
        agent: Agent,
        manifest: Manifest,
    ) -> Result<(), ManifestError> {
        manifest.validate_agent(agent)?;
        let loaded = LoadedManifest {
            agent,
            manifest,
            source: ManifestSource::Supplied,
        };
        if let Some(previous) = self.manifests.iter_mut().find(|entry| entry.agent == agent) {
            *previous = loaded;
        } else {
            self.manifests.push(loaded);
        }
        Ok(())
    }

    /// Restore the bundled manifest, or remove a supplied one when none is bundled.
    pub fn reset_manifest(&mut self, agent: Agent) {
        self.manifests.retain(|entry| entry.agent != agent);
        if let Some(manifest) = bundled_manifest(agent) {
            self.manifests.push(LoadedManifest {
                agent,
                manifest,
                source: ManifestSource::Bundled,
            });
        }
    }

    pub fn manifest(&self, agent: Agent) -> Option<&Manifest> {
        self.loaded_manifest(agent).map(|loaded| &loaded.manifest)
    }

    pub fn manifest_summaries(&self) -> Vec<ManifestSummary> {
        self.manifests
            .iter()
            .map(|loaded| ManifestSummary {
                agent: loaded.agent,
                source: loaded.source,
                version: loaded.manifest.version().cloned(),
                rule_count: loaded.manifest.rule_count(),
            })
            .collect()
    }

    /// Evaluate the snapshot without allocating diagnostic rule evidence.
    pub fn detect(&self, agent: Option<Agent>, input: DetectionInput<'_>) -> AgentDetection {
        let Some(loaded) = agent.and_then(|agent| self.loaded_manifest(agent)) else {
            return fallback_detection(agent);
        };
        let mut matched = None;
        for (rule, compiled) in loaded
            .manifest
            .rules()
            .iter()
            .zip(&loaded.manifest.compiled_rules)
        {
            if compiled_rule_matches(compiled, region(input, &rule.region))
                && matched.is_none_or(|previous: &crate::manifest::ManifestRule| {
                    previous.priority < rule.priority
                })
            {
                matched = Some(rule);
            }
        }
        matched.map_or_else(|| fallback_detection(agent), rule_detection)
    }

    /// Evaluate all rules and retain bounded diagnostic evidence for each one.
    pub fn explain(&self, agent: Option<Agent>, input: DetectionInput<'_>) -> DetectionExplain {
        let loaded = agent.and_then(|agent| self.loaded_manifest(agent));
        let mut result = fallback_explain(agent, loaded);
        let Some(loaded) = loaded else {
            return result;
        };
        let mut matched = None;
        for (rule, compiled) in loaded
            .manifest
            .rules()
            .iter()
            .zip(&loaded.manifest.compiled_rules)
        {
            let text = region(input, &rule.region);
            let matches = compiled_rule_matches(compiled, text);
            result.evaluated_rules.push(EvaluatedRule {
                id: rule.id.clone(),
                priority: rule.priority,
                region: rule.region.clone(),
                evidence: rule_evidence(rule, text),
                state: rule
                    .state
                    .map(AgentState::from)
                    .unwrap_or(AgentState::Unknown),
                matched: matches,
            });
            if matches
                && matched.is_none_or(|previous: &crate::manifest::ManifestRule| {
                    previous.priority < rule.priority
                })
            {
                matched = Some(rule);
            }
        }
        if let Some(rule) = matched {
            let detection = rule_detection(rule);
            result.state = detection.state;
            result.visible_idle = detection.visible_idle;
            result.visible_blocker = detection.visible_blocker;
            result.visible_working = detection.visible_working;
            result.skip_state_update = detection.skip_state_update;
            result.skipped_update_reason = rule
                .skip_state_update
                .then(|| format!("matched_rule:{}", rule.id));
            result.matched_rule = Some(MatchedRule {
                id: rule.id.clone(),
                priority: rule.priority,
                region: rule.region.clone(),
                state: detection.state,
            });
            result.fallback_reason = None;
        }
        result
    }

    /// Resolve a known label or alias and report unknown labels explicitly.
    pub fn explain_for_label(&self, label: &str, input: DetectionInput<'_>) -> DetectionExplain {
        if let Some(agent) = parse_agent_label(label) {
            self.explain(Some(agent), input)
        } else {
            let mut result = fallback_explain(None, None);
            result.agent = Some(label.to_owned());
            result.fallback_reason = Some("unknown_agent".to_owned());
            result
        }
    }

    fn loaded_manifest(&self, agent: Agent) -> Option<&LoadedManifest> {
        self.manifests.iter().find(|loaded| loaded.agent == agent)
    }
}

fn rule_detection(rule: &crate::manifest::ManifestRule) -> AgentDetection {
    let state = rule
        .state
        .map(AgentState::from)
        .unwrap_or(AgentState::Unknown);
    AgentDetection {
        state,
        visible_idle: rule.visible_idle && state == AgentState::Idle,
        visible_blocker: rule.visible_blocker && state == AgentState::Blocked,
        visible_working: rule.visible_working && state == AgentState::Working,
        skip_state_update: rule.skip_state_update,
    }
}

fn fallback_detection(agent: Option<Agent>) -> AgentDetection {
    AgentDetection {
        state: if agent.is_some_and(|agent| agent != Agent::Codex) {
            AgentState::Idle
        } else {
            AgentState::Unknown
        },
        skip_state_update: false,
        visible_idle: false,
        visible_blocker: false,
        visible_working: false,
    }
}

fn fallback_explain(agent: Option<Agent>, loaded: Option<&LoadedManifest>) -> DetectionExplain {
    DetectionExplain {
        agent: agent.map(|agent| agent_label(agent).to_owned()),
        state: fallback_detection(agent).state,
        source: loaded.map(|loaded| loaded.source),
        matched_rule: None,
        visible_idle: false,
        visible_blocker: false,
        visible_working: false,
        skip_state_update: false,
        skipped_update_reason: None,
        fallback_reason: match agent {
            Some(Agent::Codex) => Some("codex_state_ambiguous".to_owned()),
            Some(_) => Some(DEFAULT_KNOWN_AGENT_IDLE_FALLBACK.to_owned()),
            None => None,
        },
        evaluated_rules: Vec::new(),
        manifest_version: loaded.and_then(|loaded| loaded.manifest.version().cloned()),
    }
}
