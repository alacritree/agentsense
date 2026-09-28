// SPDX-License-Identifier: Apache-2.0
// Derived from Herdr; see the workspace NOTICE for provenance.

//! Normalize hook reports independently of their delivery transport.
//!
//! Hosts decide freshness and authority after parsing. The historic `herdr:*`
//! protocol source identifiers remain valid, including session-only hooks.

use std::collections::HashMap;
use std::time::Duration;

use agentsense_core::AgentState;
use serde::{Deserialize, Serialize};

use crate::metadata::{
    normalize_metadata_source, normalize_metadata_tokens, normalize_metadata_ttl,
    normalize_reported_agent_label, MetadataError,
};
use crate::session::{
    normalize_session_start_source, session_ref_from_report, validate_resume_argv, AgentSessionRef,
    ResumeArgvError,
};

/// State spelling used by agent hook JSON reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookState {
    Idle,
    Working,
    Blocked,
    Unknown,
}

impl From<HookState> for AgentState {
    fn from(value: HookState) -> Self {
        match value {
            HookState::Idle => Self::Idle,
            HookState::Working => Self::Working,
            HookState::Blocked => Self::Blocked,
            HookState::Unknown => Self::Unknown,
        }
    }
}

/// Invalid report contents; transport and deserialization errors remain with callers.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HookReportError {
    #[error("reported agent must not be empty")]
    EmptyAgent,
    #[error(transparent)]
    Resume(#[from] ResumeArgvError),
    #[error(transparent)]
    Metadata(#[from] MetadataError),
    #[error("unknown state label: {status}")]
    InvalidStateLabel { status: String },
    #[error("cannot set and clear the same metadata field")]
    SetAndClear,
    #[error("missing metadata field to set or clear")]
    EmptyMetadata,
}

/// A hook state report without the host's pane or terminal identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookReport {
    pub source: String,
    pub agent: String,
    pub state: HookState,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub seq: Option<u64>,
    #[serde(default)]
    pub agent_session_id: Option<String>,
    #[serde(default)]
    pub agent_session_path: Option<String>,
    #[serde(default)]
    pub resume_argv: Option<Vec<String>>,
}

/// A normalized state report, ready for caller-owned authority arbitration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedHookReport {
    pub source: String,
    pub agent: String,
    pub state: AgentState,
    pub message: Option<String>,
    pub seq: Option<u64>,
    pub session_ref: Option<AgentSessionRef>,
    pub resume_argv: Option<Vec<String>>,
}

impl HookReport {
    pub fn normalize(self) -> Result<ParsedHookReport, HookReportError> {
        let agent =
            normalize_reported_agent_label(&self.agent).ok_or(HookReportError::EmptyAgent)?;
        if let Some(argv) = &self.resume_argv {
            validate_resume_argv(argv)?;
        }
        let session_ref = session_ref_from_report(
            &self.source,
            &agent,
            self.agent_session_id,
            self.agent_session_path,
        );
        Ok(ParsedHookReport {
            source: self.source,
            agent,
            state: self.state.into(),
            message: self.message,
            seq: self.seq,
            session_ref,
            resume_argv: self.resume_argv,
        })
    }
}

/// A hook that reports session identity without asserting activity state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionReport {
    pub source: String,
    pub agent: String,
    #[serde(default)]
    pub seq: Option<u64>,
    #[serde(default)]
    pub agent_session_id: Option<String>,
    #[serde(default)]
    pub agent_session_path: Option<String>,
    #[serde(default)]
    pub session_start_source: Option<String>,
    #[serde(default)]
    pub resume_argv: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSessionReport {
    pub source: String,
    pub agent: String,
    pub seq: Option<u64>,
    pub session_ref: Option<AgentSessionRef>,
    pub session_start_source: Option<String>,
    pub resume_argv: Option<Vec<String>>,
}

impl SessionReport {
    pub fn normalize(self) -> Result<ParsedSessionReport, HookReportError> {
        let agent =
            normalize_reported_agent_label(&self.agent).ok_or(HookReportError::EmptyAgent)?;
        if let Some(argv) = &self.resume_argv {
            validate_resume_argv(argv)?;
        }
        let session_ref = session_ref_from_report(
            &self.source,
            &agent,
            self.agent_session_id,
            self.agent_session_path,
        );
        Ok(ParsedSessionReport {
            source: self.source,
            agent,
            seq: self.seq,
            session_ref,
            session_start_source: normalize_session_start_source(self.session_start_source),
            resume_argv: self.resume_argv,
        })
    }
}

/// A metadata patch without the host's resource identifier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataReport {
    pub source: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub applies_to_source: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub display_agent: Option<String>,
    #[serde(default)]
    pub state_labels: HashMap<String, String>,
    #[serde(default)]
    pub tokens: HashMap<String, Option<String>>,
    #[serde(default)]
    pub clear_title: bool,
    #[serde(default)]
    pub clear_display_agent: bool,
    #[serde(default)]
    pub clear_state_labels: bool,
    #[serde(default)]
    pub seq: Option<u64>,
    #[serde(default)]
    pub ttl_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMetadataReport {
    pub source: String,
    pub agent: Option<String>,
    pub applies_to_source: Option<String>,
    pub title: Option<String>,
    pub display_agent: Option<String>,
    pub state_labels: HashMap<String, String>,
    pub tokens: Option<HashMap<String, Option<String>>>,
    pub clear_title: bool,
    pub clear_display_agent: bool,
    pub clear_state_labels: bool,
    pub seq: Option<u64>,
    pub ttl: Option<Duration>,
}

impl MetadataReport {
    pub fn normalize(self) -> Result<ParsedMetadataReport, HookReportError> {
        let agent = self
            .agent
            .as_deref()
            .map(|agent| normalize_reported_agent_label(agent).ok_or(HookReportError::EmptyAgent))
            .transpose()?;
        let source = normalize_metadata_source(self.source)?;
        let raw_title_set = self.title.is_some();
        let raw_display_agent_set = self.display_agent.is_some();
        let raw_state_labels_set = !self.state_labels.is_empty();
        let tokens = if self.tokens.is_empty() {
            None
        } else {
            Some(normalize_metadata_tokens(self.tokens)?)
        };
        let ttl = normalize_metadata_ttl(self.ttl_ms)?;
        let title = normalize_presentation_text(self.title);
        let display_agent = normalize_presentation_text(self.display_agent);
        let applies_to_source = self
            .applies_to_source
            .map(normalize_metadata_source)
            .transpose()?;
        let state_labels = normalize_state_labels(self.state_labels)?;
        if raw_title_set && self.clear_title
            || raw_display_agent_set && self.clear_display_agent
            || raw_state_labels_set && self.clear_state_labels
        {
            return Err(HookReportError::SetAndClear);
        }
        if title.is_none()
            && display_agent.is_none()
            && state_labels.is_empty()
            && tokens.is_none()
            && !self.clear_title
            && !self.clear_display_agent
            && !self.clear_state_labels
        {
            return Err(HookReportError::EmptyMetadata);
        }
        Ok(ParsedMetadataReport {
            source,
            agent,
            applies_to_source,
            title,
            display_agent,
            state_labels,
            tokens,
            clear_title: self.clear_title,
            clear_display_agent: self.clear_display_agent,
            clear_state_labels: self.clear_state_labels,
            seq: self.seq,
            ttl,
        })
    }
}

pub fn normalize_presentation_text(value: Option<String>) -> Option<String> {
    let trimmed = value?.trim().to_string();
    let normalized: String = trimmed
        .chars()
        .filter(|ch| !ch.is_control())
        .take(80)
        .collect();
    (!normalized.trim().is_empty()).then(|| normalized.trim().to_string())
}

pub fn normalize_state_labels(
    labels: HashMap<String, String>,
) -> Result<HashMap<String, String>, HookReportError> {
    labels
        .into_iter()
        .map(|(status, label)| {
            let status = status.trim().to_ascii_lowercase();
            if !matches!(
                status.as_str(),
                "idle" | "working" | "blocked" | "done" | "unknown"
            ) {
                return Err(HookReportError::InvalidStateLabel { status });
            }
            Ok(normalize_presentation_text(Some(label)).map(|label| (status, label)))
        })
        .filter_map(Result::transpose)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_preserves_custom_agent_and_rejects_invalid_resume_command() {
        let report = HookReport {
            source: "custom".into(),
            agent: " My Agent ".into(),
            state: HookState::Working,
            message: None,
            seq: Some(4),
            agent_session_id: Some("session".into()),
            agent_session_path: None,
            resume_argv: None,
        };
        let normalized = report.clone().normalize().unwrap();
        assert_eq!(normalized.agent, "My Agent");
        assert_eq!(normalized.session_ref, None);
        assert_eq!(normalized.state, AgentState::Working);
        assert_eq!(
            HookReport {
                resume_argv: Some(vec!["/bin/agent".into()]),
                ..report
            }
            .normalize(),
            Err(HookReportError::Resume(ResumeArgvError::InvalidCommand))
        );
    }

    #[test]
    fn metadata_conflicts_are_checked_before_empty_values_are_discarded() {
        let report = MetadataReport {
            source: "custom".into(),
            title: Some("\n".into()),
            clear_title: true,
            ..MetadataReport::default()
        };
        assert_eq!(report.normalize(), Err(HookReportError::SetAndClear));
    }

    #[test]
    fn metadata_normalization_preserves_clear_only_requests() {
        let report = MetadataReport {
            source: " custom ".into(),
            clear_title: true,
            ttl_ms: Some(1),
            ..MetadataReport::default()
        }
        .normalize()
        .unwrap();
        assert_eq!(report.source, "custom");
        assert_eq!(report.ttl, Some(Duration::from_millis(1)));
        assert!(report.clear_title);
        assert!(report.tokens.is_none());
    }

    #[test]
    fn state_labels_keep_done_and_reject_unknown_names() {
        assert_eq!(
            normalize_state_labels(HashMap::from([(" DONE ".into(), " done\x01 ".into())]))
                .unwrap(),
            HashMap::from([("done".into(), "done".into())])
        );
        assert_eq!(
            normalize_state_labels(HashMap::from([("surprised".into(), "!".into())])),
            Err(HookReportError::InvalidStateLabel {
                status: "surprised".into()
            })
        );
    }
}
