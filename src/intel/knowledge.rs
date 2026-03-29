use std::collections::BTreeMap;

use serde::Deserialize;

use crate::project::ProjectKind;

#[derive(Debug, Clone, Default)]
pub struct KnowledgePack {
    rules: BTreeMap<String, RuleKnowledge>,
}

impl KnowledgePack {
    pub fn load(kind: ProjectKind) -> Self {
        let mut common = parse_pack(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/knowledge/common.yml"
        )));
        let ecosystem = match kind {
            ProjectKind::Rust => Some(parse_pack(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/knowledge/rust.yml"
            )))),
            ProjectKind::Node => Some(parse_pack(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/knowledge/node.yml"
            )))),
            ProjectKind::Python => Some(parse_pack(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/knowledge/python.yml"
            )))),
            ProjectKind::Go => Some(parse_pack(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/knowledge/go.yml"
            )))),
            ProjectKind::Unknown => None,
        };

        if let Some(ecosystem) = ecosystem {
            for (rule_id, rule) in ecosystem.rules {
                common
                    .rules
                    .entry(rule_id)
                    .and_modify(|existing| existing.merge(&rule))
                    .or_insert(rule);
            }
        }

        common
    }

    pub fn rule(&self, id: &str) -> RuleKnowledge {
        self.rules.get(id).cloned().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuleKnowledge {
    pub aliases: Vec<String>,
    pub related_paths: Vec<String>,
    pub valid_commands: Vec<String>,
    pub anti_patterns: Vec<String>,
    pub section_aliases: BTreeMap<String, Vec<String>>,
}

impl RuleKnowledge {
    fn merge(&mut self, other: &RuleKnowledge) {
        extend_unique(&mut self.aliases, &other.aliases);
        extend_unique(&mut self.related_paths, &other.related_paths);
        extend_unique(&mut self.valid_commands, &other.valid_commands);
        extend_unique(&mut self.anti_patterns, &other.anti_patterns);

        for (section, aliases) in &other.section_aliases {
            self.section_aliases
                .entry(section.clone())
                .and_modify(|existing| extend_unique(existing, aliases))
                .or_insert_with(|| aliases.clone());
        }
    }

    pub fn all_terms(&self) -> Vec<String> {
        let mut out = self.aliases.clone();
        extend_unique(&mut out, &self.valid_commands);
        extend_unique(&mut out, &self.anti_patterns);
        for values in self.section_aliases.values() {
            extend_unique(&mut out, values);
        }
        out
    }
}

#[derive(Debug, Deserialize)]
struct RawKnowledgePack {
    rules: BTreeMap<String, RawRuleKnowledge>,
}

#[derive(Debug, Deserialize, Default)]
struct RawRuleKnowledge {
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    related_paths: Vec<String>,
    #[serde(default)]
    valid_commands: Vec<String>,
    #[serde(default)]
    anti_patterns: Vec<String>,
    #[serde(default)]
    section_aliases: BTreeMap<String, Vec<String>>,
}

fn parse_pack(contents: &str) -> KnowledgePack {
    let raw = serde_yaml::from_str::<RawKnowledgePack>(contents).unwrap_or(RawKnowledgePack {
        rules: BTreeMap::new(),
    });

    KnowledgePack {
        rules: raw
            .rules
            .into_iter()
            .map(|(id, rule)| {
                (
                    id,
                    RuleKnowledge {
                        aliases: rule.aliases,
                        related_paths: rule.related_paths,
                        valid_commands: rule.valid_commands,
                        anti_patterns: rule.anti_patterns,
                        section_aliases: rule.section_aliases,
                    },
                )
            })
            .collect(),
    }
}

fn extend_unique(target: &mut Vec<String>, incoming: &[String]) {
    for value in incoming {
        if !target.iter().any(|existing| existing == value) {
            target.push(value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecosystem_pack_merges_commands() {
        let pack = KnowledgePack::load(ProjectKind::Rust);
        let ci = pack.rule("ci_workflow");

        assert!(ci.valid_commands.iter().any(|value| value == "cargo test"));
        assert!(ci
            .related_paths
            .iter()
            .any(|value| value == ".github/workflows"));
    }
}
