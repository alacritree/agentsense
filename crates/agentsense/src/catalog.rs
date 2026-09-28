// SPDX-License-Identifier: Apache-2.0
// Adapted from Herdr for agentsense; see the package NOTICE.

//! Parse manifest catalog metadata supplied by callers.
//! No URLs are fetched and no filesystem paths are opened.

use std::collections::BTreeSet;

use serde::Deserialize;
use thiserror::Error;

use crate::{agent_label, parse_agent_label, Agent};

/// Recognized manifest entry with a relative catalog path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogAgent {
    pub agent: Agent,
    pub path: String,
}

/// Validated entries and unknown IDs retained for caller diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestCatalog {
    pub agents: Vec<CatalogAgent>,
    pub unknown_agents: Vec<String>,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CatalogError {
    #[error("failed to parse catalog TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("unsupported catalog schema_version {version}")]
    SchemaVersion { version: u32 },
    #[error("catalog entry {id} has an empty path")]
    EmptyPath { id: String },
    #[error("catalog entry {id} has an unsafe path {path:?}")]
    UnsafePath { id: String, path: String },
    #[error("catalog contains duplicate agent {id}")]
    DuplicateAgent { id: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCatalog {
    schema_version: u32,
    #[serde(default)]
    agents: Vec<RawCatalogAgent>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCatalogAgent {
    id: String,
    path: String,
}

impl ManifestCatalog {
    /// Parse catalog schema version 1. Unknown agents are reported and skipped.
    pub fn parse(content: &str) -> Result<Self, CatalogError> {
        let catalog: RawCatalog = toml::from_str(content)?;
        if catalog.schema_version != 1 {
            return Err(CatalogError::SchemaVersion {
                version: catalog.schema_version,
            });
        }
        let mut seen = BTreeSet::new();
        let mut agents = Vec::new();
        let mut unknown_agents = Vec::new();
        for entry in catalog.agents {
            let Some(agent) = parse_agent_label(&entry.id) else {
                unknown_agents.push(entry.id);
                continue;
            };
            if entry.path.trim().is_empty() {
                return Err(CatalogError::EmptyPath { id: entry.id });
            }
            // Backslashes and drive prefixes are unsafe on Windows too, even when
            // the catalog is being inspected on a Unix host.
            if entry.path.contains(':')
                || entry.path.contains('\\')
                || entry.path.starts_with('/')
                || entry.path.split('/').any(|part| part == "..")
            {
                return Err(CatalogError::UnsafePath {
                    id: entry.id,
                    path: entry.path,
                });
            }
            if !seen.insert(agent_label(agent)) {
                return Err(CatalogError::DuplicateAgent { id: entry.id });
            }
            agents.push(CatalogAgent {
                agent,
                path: entry.path,
            });
        }
        Ok(Self {
            agents,
            unknown_agents,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_agents_and_reports_unknown_ids() {
        let catalog = ManifestCatalog::parse(
            r#"
schema_version = 1
[[agents]]
id = "codex"
path = "manifests/codex.toml"
[[agents]]
id = "future-agent"
path = "future.toml"
"#,
        )
        .unwrap();
        assert_eq!(
            catalog.agents,
            vec![CatalogAgent {
                agent: Agent::Codex,
                path: "manifests/codex.toml".to_owned()
            }]
        );
        assert_eq!(catalog.unknown_agents, ["future-agent"]);
    }

    #[test]
    fn rejects_duplicates_including_aliases() {
        let error = ManifestCatalog::parse(
            r#"
schema_version = 1
[[agents]]
id = "copilot"
path = "a.toml"
[[agents]]
id = "github-copilot"
path = "b.toml"
"#,
        )
        .unwrap_err();
        assert!(matches!(error, CatalogError::DuplicateAgent { .. }));
    }

    #[test]
    fn rejects_unsafe_paths() {
        for path in [
            "../codex.toml",
            "sub/../codex.toml",
            "/codex.toml",
            "https://example.test/codex.toml",
            r"..\codex.toml",
            r"C:\codex.toml",
        ] {
            let text = format!("schema_version = 1\n[[agents]]\nid = 'codex'\npath = '{path}'\n");
            assert!(
                matches!(
                    ManifestCatalog::parse(&text),
                    Err(CatalogError::UnsafePath { .. })
                ),
                "accepted {path:?}"
            );
        }
    }
}
