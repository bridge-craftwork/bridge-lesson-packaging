//! Topic baselines — the per-lesson difficulty prior (see topics.example.toml).
//!
//! Editorial, like the profile's editorial sidecar: the tool can't derive these
//! numbers, you author them. Loaded by `profile --topics` and used to attribute
//! each deal to a topic and combine the topic baseline with the observed,
//! objective per-deal difficulty.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Topics {
    #[serde(default)]
    pub defaults: Baseline,
    #[serde(default, rename = "topic")]
    pub topics: Vec<TopicRule>,
}

/// Baseline difficulty on the two axes (ordinal; higher = harder).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Baseline {
    pub bidding: u8,
    pub cardplay: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TopicRule {
    pub name: String,
    /// Case-insensitive substrings matched against the deal's lesson path.
    #[serde(rename = "match")]
    pub matches: Vec<String>,
    pub bidding: u8,
    pub cardplay: u8,
}

impl Topics {
    pub fn load(path: &Path) -> Result<Topics> {
        let txt = std::fs::read_to_string(path)
            .with_context(|| format!("reading topics {}", path.display()))?;
        toml::from_str(&txt).with_context(|| format!("parsing topics TOML {}", path.display()))
    }

    /// Resolve a deal's topic from its lesson path (record `source.file`).
    /// Returns the first matching topic, or the `(unmatched)` default.
    pub fn resolve(&self, lesson_path: &str) -> (String, Baseline) {
        let hay = lesson_path.to_lowercase();
        for t in &self.topics {
            if t.matches.iter().any(|m| hay.contains(&m.to_lowercase())) {
                return (
                    t.name.clone(),
                    Baseline {
                        bidding: t.bidding,
                        cardplay: t.cardplay,
                    },
                );
            }
        }
        ("(unmatched)".to_string(), self.defaults)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_first_match_case_insensitively() {
        let t: Topics = toml::from_str(
            r#"
            [defaults]
            bidding = 1
            cardplay = 1
            [[topic]]
            name = "Finesse"
            match = ["Finesse"]
            bidding = 1
            cardplay = 2
            [[topic]]
            name = "Squeeze"
            match = ["Squeeze"]
            bidding = 1
            cardplay = 4
        "#,
        )
        .unwrap();

        let (name, base) = t.resolve("Baker/finesse.pbn");
        assert_eq!(name, "Finesse");
        assert_eq!(base.cardplay, 2);

        let (name, base) = t.resolve("Advanced/Squeeze practice deals.pbn");
        assert_eq!(name, "Squeeze");
        assert_eq!(base.cardplay, 4);

        let (name, base) = t.resolve("Stayman.pbn");
        assert_eq!(name, "(unmatched)");
        assert_eq!(base.cardplay, 1); // default
    }
}
