// SPDX-License-Identifier: Apache-2.0
// Derived from Herdr; see the workspace NOTICE for provenance.

//! Metadata token patches, expiration, sequence validation, and normalization.
//!
//! Tokens are display strings supplied by hooks, not model token usage counts.
//! Expiry changes state only when the caller explicitly sweeps a timestamp.

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
struct MetadataToken {
    value: String,
    expires_at: Option<Instant>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetadataTokens {
    entries: HashMap<String, MetadataToken>,
}

pub const MAX_SEQUENCE_SOURCES: usize = 32;

pub fn sequence_is_fresh(sequences: &HashMap<String, u64>, source: &str, seq: Option<u64>) -> bool {
    seq.is_none_or(|seq| sequences.get(source).is_none_or(|last| seq > *last))
}

pub fn accept_sequence(
    sequences: &mut HashMap<String, u64>,
    source: &str,
    seq: Option<u64>,
) -> Result<bool, MetadataError> {
    let Some(seq) = seq else {
        return Ok(true);
    };
    if !sequence_is_fresh(sequences, source, Some(seq)) {
        return Ok(false);
    }
    if !sequences.contains_key(source) && sequences.len() >= MAX_SEQUENCE_SOURCES {
        return Err(MetadataError::TooManySequenceSources {
            limit: MAX_SEQUENCE_SOURCES,
        });
    }
    sequences.insert(source.to_string(), seq);
    Ok(true)
}

impl MetadataTokens {
    pub fn patch(
        &mut self,
        patch: HashMap<String, Option<String>>,
        ttl: Option<Duration>,
        now: Instant,
    ) -> bool {
        let expires_at = ttl.and_then(|ttl| now.checked_add(ttl));
        let mut changed = false;
        for (key, value) in patch {
            match value {
                Some(value) => {
                    let token = MetadataToken { value, expires_at };
                    if self.entries.get(&key) != Some(&token) {
                        self.entries.insert(key, token);
                        changed = true;
                    }
                }
                None => {
                    changed |= self.entries.remove(&key).is_some();
                }
            }
        }
        changed
    }

    pub fn key_count_after_patch(&self, patch: &HashMap<String, Option<String>>) -> usize {
        let mut keys = self
            .values()
            .into_keys()
            .collect::<std::collections::HashSet<_>>();
        for (key, value) in patch {
            if value.is_some() {
                keys.insert(key.clone());
            } else {
                keys.remove(key);
            }
        }
        keys.len()
    }

    pub fn values(&self) -> HashMap<String, String> {
        self.entries
            .iter()
            .map(|(key, token)| (key.clone(), token.value.clone()))
            .collect()
    }

    pub fn next_expiry(&self) -> Option<Instant> {
        self.entries
            .values()
            .filter_map(|token| token.expires_at)
            .min()
    }

    pub fn expire_at(&mut self, now: Instant) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|_, token| token.expires_at.is_none_or(|deadline| deadline > now));
        self.entries.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patch(items: &[(&str, Option<&str>)]) -> HashMap<String, Option<String>> {
        items
            .iter()
            .map(|(key, value)| ((*key).into(), value.map(str::to_string)))
            .collect()
    }

    #[test]
    fn sequence_sources_are_bounded() {
        let mut sequences = HashMap::new();
        for index in 0..MAX_SEQUENCE_SOURCES {
            assert_eq!(
                accept_sequence(&mut sequences, &format!("source-{index}"), Some(1)),
                Ok(true)
            );
        }
        assert_eq!(
            accept_sequence(&mut sequences, "one-too-many", Some(1)),
            Err(MetadataError::TooManySequenceSources {
                limit: MAX_SEQUENCE_SOURCES
            })
        );
        assert_eq!(
            accept_sequence(&mut sequences, "source-0", Some(1)),
            Ok(false)
        );
        assert_eq!(
            accept_sequence(&mut sequences, "source-0", Some(2)),
            Ok(true)
        );
    }

    #[test]
    fn patches_and_clears_individual_keys() {
        let now = Instant::now();
        let mut tokens = MetadataTokens::default();
        tokens.patch(
            patch(&[("summary", Some("one")), ("model", Some("opus"))]),
            None,
            now,
        );
        tokens.patch(
            patch(&[("summary", Some("two")), ("model", None)]),
            None,
            now,
        );

        assert_eq!(
            tokens.values(),
            HashMap::from([("summary".into(), "two".into())])
        );
    }

    #[test]
    fn ttl_only_changes_keys_in_the_patch() {
        let now = Instant::now();
        let deadline = now + Duration::from_secs(1);
        let mut tokens = MetadataTokens::default();
        tokens.patch(
            patch(&[("short", Some("one"))]),
            Some(Duration::from_secs(1)),
            now,
        );
        tokens.patch(patch(&[("persistent", Some("two"))]), None, now);

        assert!(tokens.expire_at(deadline));
        assert_eq!(
            tokens.values(),
            HashMap::from([("persistent".into(), "two".into())])
        );
    }

    #[test]
    fn values_remain_stable_until_expiry_mutates_state() {
        let now = Instant::now();
        let deadline = now + Duration::from_secs(1);
        let mut tokens = MetadataTokens::default();
        tokens.patch(
            patch(&[("summary", Some("temporary"))]),
            Some(Duration::from_secs(1)),
            now,
        );

        assert_eq!(
            tokens.values(),
            HashMap::from([("summary".into(), "temporary".into())])
        );
        assert!(tokens.expire_at(deadline));
        assert!(tokens.values().is_empty());
    }

    #[test]
    fn delayed_expiry_sweep_removes_every_token_due_at_now() {
        let now = Instant::now();
        let mut tokens = MetadataTokens::default();
        tokens.patch(
            patch(&[("first", Some("one"))]),
            Some(Duration::from_secs(1)),
            now,
        );
        tokens.patch(
            patch(&[("second", Some("two"))]),
            Some(Duration::from_secs(2)),
            now,
        );

        assert!(tokens.expire_at(now + Duration::from_secs(10)));
        assert!(tokens.values().is_empty());
    }

    #[test]
    fn stale_expiry_does_not_clear_replacement() {
        let now = Instant::now();
        let first_deadline = now + Duration::from_secs(1);
        let mut tokens = MetadataTokens::default();
        tokens.patch(
            patch(&[("summary", Some("old"))]),
            Some(Duration::from_secs(1)),
            now,
        );
        tokens.patch(
            patch(&[("summary", Some("new"))]),
            Some(Duration::from_secs(5)),
            now,
        );

        assert!(!tokens.expire_at(first_deadline));
        assert_eq!(
            tokens.values(),
            HashMap::from([("summary".into(), "new".into())])
        );
    }

    #[test]
    fn update_without_ttl_cancels_previous_expiry() {
        let now = Instant::now();
        let deadline = now + Duration::from_secs(1);
        let mut tokens = MetadataTokens::default();
        tokens.patch(
            patch(&[("summary", Some("temporary"))]),
            Some(Duration::from_secs(1)),
            now,
        );
        tokens.patch(patch(&[("summary", Some("persistent"))]), None, now);

        assert!(!tokens.expire_at(deadline));
        assert_eq!(
            tokens.values(),
            HashMap::from([("summary".into(), "persistent".into())])
        );
    }
}

/// A rejected metadata source, value, or sequence.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MetadataError {
    #[error("metadata source must not be empty")]
    EmptySource,
    #[error("metadata source must be {limit} characters or fewer")]
    SourceTooLong { limit: usize },
    #[error("metadata source may contain only ASCII letters, digits, colon, dot, underscore, and hyphen")]
    InvalidSource,
    #[error("metadata ttl_ms must be at least {minimum_ms}")]
    TtlTooShort { minimum_ms: u64 },
    #[error("metadata ttl_ms must be {maximum_ms} or less")]
    TtlTooLong { maximum_ms: u64 },
    #[error("missing token to set or clear")]
    EmptyTokenPatch,
    #[error("a metadata report may update at most {limit} tokens")]
    TooManyTokens { limit: usize },
    #[error("invalid metadata token key: {key}")]
    InvalidTokenKey { key: String },
    #[error("metadata sequence history allows at most {limit} sources")]
    TooManySequenceSources { limit: usize },
}

pub fn normalize_reported_agent_label(agent: &str) -> Option<String> {
    let trimmed = agent.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(agent) = agentsense_core::parse_agent_label(trimmed) {
        return Some(agentsense_core::agent_label(agent).to_string());
    }
    Some(trimmed.to_string())
}

pub const METADATA_TTL_MAX_MS: u64 = 86_400_000;
pub const METADATA_SOURCE_MAX_CHARS: usize = 80;
const METADATA_TTL_MIN_MS: u64 = 1;
const MAX_METADATA_TOKEN_KEYS_PER_REQUEST: usize = 16;
pub const MAX_METADATA_TOKEN_KEYS_PER_RESOURCE: usize = 32;
const MAX_METADATA_TOKEN_KEY_LEN: usize = 32;
const MAX_METADATA_TOKEN_VALUE_LEN: usize = 80;

pub fn normalize_metadata_source(value: String) -> Result<String, MetadataError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(MetadataError::EmptySource);
    }
    if value.chars().count() > METADATA_SOURCE_MAX_CHARS {
        return Err(MetadataError::SourceTooLong {
            limit: METADATA_SOURCE_MAX_CHARS,
        });
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ':' | '.' | '_' | '-'))
    {
        return Err(MetadataError::InvalidSource);
    }
    Ok(value.to_string())
}

pub fn normalize_metadata_ttl(
    ttl_ms: Option<u64>,
) -> Result<Option<std::time::Duration>, MetadataError> {
    let Some(ttl_ms) = ttl_ms else {
        return Ok(None);
    };
    if ttl_ms < METADATA_TTL_MIN_MS {
        return Err(MetadataError::TtlTooShort {
            minimum_ms: METADATA_TTL_MIN_MS,
        });
    }
    if ttl_ms > METADATA_TTL_MAX_MS {
        return Err(MetadataError::TtlTooLong {
            maximum_ms: METADATA_TTL_MAX_MS,
        });
    }
    Ok(Some(std::time::Duration::from_millis(ttl_ms)))
}

pub fn normalize_metadata_tokens(
    tokens: std::collections::HashMap<String, Option<String>>,
) -> Result<std::collections::HashMap<String, Option<String>>, MetadataError> {
    if tokens.is_empty() {
        return Err(MetadataError::EmptyTokenPatch);
    }
    if tokens.len() > MAX_METADATA_TOKEN_KEYS_PER_REQUEST {
        return Err(MetadataError::TooManyTokens {
            limit: MAX_METADATA_TOKEN_KEYS_PER_REQUEST,
        });
    }

    tokens
        .into_iter()
        .map(|(key, value)| {
            if key.is_empty()
                || key.len() > MAX_METADATA_TOKEN_KEY_LEN
                || !key
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
            {
                return Err(MetadataError::InvalidTokenKey { key });
            }
            let value = value.and_then(|value| {
                let normalized = value
                    .trim()
                    .chars()
                    .filter(|ch| !ch.is_control())
                    .take(MAX_METADATA_TOKEN_VALUE_LEN)
                    .collect::<String>();
                (!normalized.trim().is_empty()).then(|| normalized.trim().to_string())
            });
            Ok((key, value))
        })
        .collect()
}

#[cfg(test)]
mod metadata_token_tests {
    use super::*;

    #[test]
    fn token_normalization_sanitizes_values_and_turns_empty_into_clear() {
        let tokens = normalize_metadata_tokens(std::collections::HashMap::from([
            ("summary".into(), Some("  review\nready  ".into())),
            ("empty".into(), Some(" \n ".into())),
            ("clear".into(), None),
        ]))
        .unwrap();

        assert_eq!(tokens["summary"].as_deref(), Some("reviewready"));
        assert_eq!(tokens["empty"], None);
        assert_eq!(tokens["clear"], None);
    }

    #[test]
    fn token_normalization_rejects_invalid_or_unbounded_keys() {
        for key in [
            "bad.name".to_string(),
            "x".repeat(MAX_METADATA_TOKEN_KEY_LEN + 1),
        ] {
            assert!(normalize_metadata_tokens(std::collections::HashMap::from([(
                key,
                Some("value".into()),
            )]))
            .is_err());
        }
        let too_many = (0..=MAX_METADATA_TOKEN_KEYS_PER_REQUEST)
            .map(|index| (format!("key{index}"), Some("value".into())))
            .collect();
        assert!(normalize_metadata_tokens(too_many).is_err());
    }
}
