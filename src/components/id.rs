use std::fmt;
use std::str::FromStr;

use anyhow::bail;
use serde::{Deserialize, Serialize};

/// Stable lowercase component identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ComponentId(String);

impl ComponentId {
    pub fn parse(value: impl Into<String>) -> anyhow::Result<Self> {
        let value = value.into();
        validate(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_child_of(&self, parent: &ComponentId) -> bool {
        self.0
            .strip_prefix(parent.as_str())
            .is_some_and(|suffix| suffix.starts_with('/'))
    }

    pub fn relative_to<'a>(&'a self, parent: &ComponentId) -> Option<&'a str> {
        self.0.strip_prefix(parent.as_str())?.strip_prefix('/')
    }
}

impl fmt::Display for ComponentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for ComponentId {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

fn validate(value: &str) -> anyhow::Result<()> {
    if value.is_empty() {
        bail!("component ID cannot be empty");
    }
    for segment in value.split('/') {
        let mut characters = segment.chars();
        if !matches!(characters.next(), Some(first) if first.is_ascii_lowercase())
            || !characters.all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
            })
        {
            bail!(
                "invalid component ID '{}'; use lowercase path segments",
                value
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_ids_and_parent_relationships() {
        let use_id = ComponentId::parse("use").unwrap();
        let browser = ComponentId::parse("use/browser").unwrap();
        assert!(browser.is_child_of(&use_id));
        assert_eq!(browser.relative_to(&use_id), Some("browser"));
        assert!(!use_id.is_child_of(&browser));
    }

    #[test]
    fn rejects_unsafe_or_ambiguous_ids() {
        for value in ["", "Use", "use//browser", "use/../box", "-use", "use_thing"] {
            assert!(ComponentId::parse(value).is_err(), "{value} should fail");
        }
    }
}
