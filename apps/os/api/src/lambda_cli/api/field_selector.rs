//! Field selector parser and matcher for K8s API compatibility.
//!
//! Supports filtering resources by field selectors like:
//! - `metadata.name=nginx`
//! - `metadata.namespace=default`
//! - `status.phase=Running`
//! - Combined: `metadata.namespace=default,metadata.name=nginx`

use std::collections::HashMap;
use std::str::FromStr;

/// Field selector requirement with operator.
#[derive(Debug, Clone)]
pub struct FieldRequirement {
    /// Field path (e.g., "metadata.name").
    pub field: String,
    /// Operator (always equals for now).
    pub operator: String,
    /// Target value.
    pub value: String,
}

impl FieldRequirement {
    /// Create a new requirement.
    pub fn new(field: String, value: String) -> Self {
        Self {
            field,
            operator: "=".to_string(),
            value,
        }
    }
}

/// Parsed field selector.
#[derive(Debug, Clone, Default)]
pub struct FieldSelector {
    /// List of requirements (AND-ed together).
    pub requirements: Vec<FieldRequirement>,
}

impl FieldSelector {
    /// Parse a field selector string.
    /// Format: "key1=value1,key2=value2"
    pub fn parse(s: &str) -> Self {
        let mut requirements = Vec::new();

        for part in s.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            if let Some((key, value)) = part.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if !key.is_empty() && !value.is_empty() {
                    requirements.push(FieldRequirement::new(key.to_string(), value.to_string()));
                }
            }
        }

        Self { requirements }
    }

    /// Check if a field selector matches given fields.
    pub fn matches(&self, fields: &HashMap<String, String>) -> bool {
        if self.requirements.is_empty() {
            return true;
        }

        for req in &self.requirements {
            let field_value = fields.get(&req.field);
            match field_value {
                Some(v) if v == &req.value => {}
                _ => return false,
            }
        }

        true
    }

    /// Check if selector has any requirements.
    pub fn is_empty(&self) -> bool {
        self.requirements.is_empty()
    }
}

impl FromStr for FieldSelector {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse(s))
    }
}

/// Label selector requirement.
#[derive(Debug, Clone)]
pub struct LabelRequirement {
    pub key: String,
    pub operator: String,
    pub value: String,
}

/// Parsed label selector.
#[derive(Debug, Clone, Default)]
pub struct LabelSelector {
    pub requirements: Vec<LabelRequirement>,
}

impl LabelSelector {
    /// Parse a label selector string.
    /// Format: "key1=value1,key2!=value2,key3 in (v1,v2)"
    pub fn parse(s: &str) -> Self {
        let mut requirements = Vec::new();

        for part in s.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            // Handle != operator
            if let Some((key, value)) = part.split_once("!=") {
                requirements.push(LabelRequirement {
                    key: key.trim().to_string(),
                    operator: "!=".to_string(),
                    value: value.trim().to_string(),
                });
            }
            // Handle in operator
            else if let Some((key, values)) = part.split_once(" in ") {
                for value in values.trim_matches(|c| c == '(' || c == ')').split(',') {
                    requirements.push(LabelRequirement {
                        key: key.trim().to_string(),
                        operator: "in".to_string(),
                        value: value.trim().to_string(),
                    });
                }
            }
            // Handle = operator (default)
            else if let Some((key, value)) = part.split_once('=') {
                requirements.push(LabelRequirement {
                    key: key.trim().to_string(),
                    operator: "=".to_string(),
                    value: value.trim().to_string(),
                });
            }
        }

        Self { requirements }
    }

    /// Check if a label selector matches given labels.
    pub fn matches(&self, labels: &HashMap<String, String>) -> bool {
        if self.requirements.is_empty() {
            return true;
        }

        for req in &self.requirements {
            match req.operator.as_str() {
                "=" => {
                    if labels.get(&req.key) != Some(&req.value) {
                        return false;
                    }
                }
                "!=" => {
                    if labels.get(&req.key) == Some(&req.value) {
                        return false;
                    }
                }
                "in" => {
                    if !labels.contains_key(&req.key) {
                        return false;
                    }
                }
                _ => {}
            }
        }

        true
    }

    /// Check if selector has any requirements.
    pub fn is_empty(&self) -> bool {
        self.requirements.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_selector_parse() {
        let selector = FieldSelector::parse("metadata.namespace=default,metadata.name=nginx");
        assert_eq!(selector.requirements.len(), 2);
        assert_eq!(selector.requirements[0].field, "metadata.namespace");
        assert_eq!(selector.requirements[0].value, "default");
        assert_eq!(selector.requirements[1].field, "metadata.name");
        assert_eq!(selector.requirements[1].value, "nginx");
    }

    #[test]
    fn test_field_selector_match() {
        let selector = FieldSelector::parse("metadata.namespace=default");
        let mut fields = HashMap::new();
        fields.insert("metadata.namespace".to_string(), "default".to_string());
        assert!(selector.matches(&fields));

        fields.insert("metadata.namespace".to_string(), "kube-system".to_string());
        assert!(!selector.matches(&fields));
    }

    #[test]
    fn test_label_selector_parse() {
        let selector = LabelSelector::parse("app=nginx,env!=prod");
        assert_eq!(selector.requirements.len(), 2);
        assert_eq!(selector.requirements[0].key, "app");
        assert_eq!(selector.requirements[0].operator, "=");
        assert_eq!(selector.requirements[0].value, "nginx");
        assert_eq!(selector.requirements[1].key, "env");
        assert_eq!(selector.requirements[1].operator, "!=");
        assert_eq!(selector.requirements[1].value, "prod");
    }

    #[test]
    fn test_label_selector_match() {
        let selector = LabelSelector::parse("app=nginx");
        let mut labels = HashMap::new();
        labels.insert("app".to_string(), "nginx".to_string());
        assert!(selector.matches(&labels));

        labels.insert("app".to_string(), "apache".to_string());
        assert!(!selector.matches(&labels));
    }
}
