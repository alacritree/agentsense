// SPDX-License-Identifier: Apache-2.0
// Adapted from Herdr for agentsense; see the package NOTICE.

//! Dotted numeric manifest versions, independent of SemVer and release dates.
//! Trailing zero components compare equal; original spelling is retained.

use std::{cmp::Ordering, fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A validated dotted numeric version (for example `2026.06.10.1`).
#[derive(Debug, Clone)]
pub struct ManifestVersion(String);

/// A version must contain dot-separated unsigned 64-bit numeric components.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum VersionError {
    #[error("version must not be empty")]
    Empty,
    #[error("version {version:?} contains an empty segment")]
    EmptySegment { version: String },
    #[error("version {version:?} must be dotted numeric")]
    NonNumeric { version: String },
    #[error("version {version:?} contains an oversized segment")]
    OversizedSegment { version: String },
}

impl ManifestVersion {
    pub fn parse(value: &str) -> Result<Self, VersionError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(VersionError::Empty);
        }
        for segment in value.split('.') {
            if segment.is_empty() {
                return Err(VersionError::EmptySegment {
                    version: value.to_owned(),
                });
            }
            if !segment.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(VersionError::NonNumeric {
                    version: value.to_owned(),
                });
            }
            segment
                .parse::<u64>()
                .map_err(|_| VersionError::OversizedSegment {
                    version: value.to_owned(),
                })?;
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ManifestVersion {
    type Err = VersionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl fmt::Display for ManifestVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ManifestVersion {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

impl Serialize for ManifestVersion {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl Ord for ManifestVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut left = self
            .0
            .split('.')
            .map(|part| part.parse::<u64>().expect("validated version"));
        let mut right = other
            .0
            .split('.')
            .map(|part| part.parse::<u64>().expect("validated version"));
        loop {
            let pair = (left.next(), right.next());
            if pair == (None, None) {
                return Ordering::Equal;
            }
            let order = pair.0.unwrap_or(0).cmp(&pair.1.unwrap_or(0));
            if order != Ordering::Equal {
                return order;
            }
        }
    }
}

impl PartialOrd for ManifestVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ManifestVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for ManifestVersion {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_dotted_numeric_segments() {
        for (left, right, order) in [
            ("2026.6.10.1", "2026.6.9.9", Ordering::Greater),
            ("1.2.0", "1.2", Ordering::Equal),
            ("1.2.1", "1.2", Ordering::Greater),
            ("01.2.000", "1.2", Ordering::Equal),
            ("1", "1.0.0.1", Ordering::Less),
        ] {
            assert_eq!(
                ManifestVersion::parse(left)
                    .unwrap()
                    .cmp(&ManifestVersion::parse(right).unwrap()),
                order
            );
        }
    }

    #[test]
    fn rejects_invalid_versions() {
        for value in [
            "",
            "2026.06.alpha",
            "2026..06",
            "2026.999999999999999999999999999999",
            "1.+2",
            "1.２",
        ] {
            assert!(ManifestVersion::parse(value).is_err(), "accepted {value:?}");
        }
    }

    #[test]
    fn retains_trimmed_spelling_in_serde() {
        let version = ManifestVersion::parse("  01.02.0  ").unwrap();
        let json = serde_json::to_string(&version).unwrap();
        assert_eq!(json, "\"01.02.0\"");
        assert_eq!(
            serde_json::from_str::<ManifestVersion>(&json).unwrap(),
            version
        );
    }
}
