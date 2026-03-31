use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::project::RepoProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleRequirement {
    Partial,
    Strong,
}

#[derive(Debug, Clone)]
pub struct OssifyConfig {
    source: Option<PathBuf>,
    profile: Option<RepoProfile>,
    minimum_score: u8,
    defaults: ConfigDefaults,
    weights: CategoryWeights,
    rules: BTreeMap<String, RuleOverride>,
}

impl Default for OssifyConfig {
    fn default() -> Self {
        Self {
            source: None,
            profile: None,
            minimum_score: 85,
            defaults: ConfigDefaults::default(),
            weights: CategoryWeights::default(),
            rules: BTreeMap::new(),
        }
    }
}

impl OssifyConfig {
    pub fn load_for_target(target: &Path, explicit: Option<&Path>) -> io::Result<Self> {
        let candidate = match explicit {
            Some(path) => Some(path.to_path_buf()),
            None => {
                let auto = target.join("ossify.toml");
                auto.is_file().then_some(auto)
            }
        };

        let Some(path) = candidate else {
            return Ok(Self::default());
        };

        let contents = fs::read_to_string(&path)?;
        let raw = toml::from_str::<RawConfig>(&contents).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid config {}: {error}", path.display()),
            )
        })?;

        if let Some(version) = raw.version {
            if version != 1 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "unsupported ossify config version {} in {}",
                        version,
                        path.display()
                    ),
                ));
            }
        }

        let minimum_score = raw
            .minimum_score
            .map(|value| value.clamp(1, 100))
            .unwrap_or(85);
        let defaults = raw
            .defaults
            .map(RawDefaults::into_config_defaults)
            .unwrap_or_default();
        let weights = match raw.weights {
            Some(weights) => weights.into_category_weights()?,
            None => CategoryWeights::default(),
        };
        let rules = raw
            .rules
            .into_iter()
            .map(|(key, value)| (key, value.into_rule_override()))
            .collect();

        Ok(Self {
            source: fs::canonicalize(&path).ok().or(Some(path)),
            profile: raw.profile,
            minimum_score,
            defaults,
            weights,
            rules,
        })
    }

    pub fn source(&self) -> Option<&Path> {
        self.source.as_deref()
    }

    pub fn profile_override(&self) -> Option<RepoProfile> {
        self.profile
    }

    pub fn minimum_score(&self) -> u8 {
        self.minimum_score
    }

    pub fn default_owner(&self) -> Option<&str> {
        self.defaults.owner.as_deref()
    }

    pub fn default_license(&self) -> Option<&str> {
        self.defaults.license.as_deref()
    }

    pub fn default_funding(&self) -> Option<&str> {
        self.defaults.funding.as_deref()
    }

    pub fn category_multiplier(&self, category: &str) -> f32 {
        self.weights.for_category(category)
    }

    pub fn rule_override(&self, id: &str) -> Option<&RuleOverride> {
        self.rules.get(id)
    }

    pub fn rule_enabled(&self, id: &str) -> bool {
        self.rule_override(id)
            .and_then(|rule| rule.enabled)
            .unwrap_or(true)
    }

    pub fn rule_weight(&self, id: &str) -> Option<u16> {
        self.rule_override(id).and_then(|rule| rule.weight)
    }

    pub fn rule_requirement(&self, id: &str) -> Option<RuleRequirement> {
        self.rule_override(id).and_then(|rule| rule.required_level)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ConfigDefaults {
    owner: Option<String>,
    license: Option<String>,
    funding: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CategoryWeights {
    identity: f32,
    docs: f32,
    community: f32,
    automation: f32,
    release: f32,
}

impl Default for CategoryWeights {
    fn default() -> Self {
        Self {
            identity: 1.0,
            docs: 1.0,
            community: 1.0,
            automation: 1.0,
            release: 1.0,
        }
    }
}

impl CategoryWeights {
    fn for_category(&self, category: &str) -> f32 {
        match category {
            "identity" => self.identity,
            "docs" => self.docs,
            "community" => self.community,
            "automation" => self.automation,
            "release" => self.release,
            _ => 1.0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuleOverride {
    enabled: Option<bool>,
    weight: Option<u16>,
    required_level: Option<RuleRequirement>,
}

#[derive(Debug, Deserialize, Default)]
struct RawConfig {
    version: Option<u8>,
    profile: Option<RepoProfile>,
    minimum_score: Option<u8>,
    defaults: Option<RawDefaults>,
    weights: Option<RawWeights>,
    #[serde(default)]
    rules: BTreeMap<String, RawRuleOverride>,
}

#[derive(Debug, Deserialize, Default)]
struct RawDefaults {
    owner: Option<String>,
    license: Option<String>,
    funding: Option<String>,
}

impl RawDefaults {
    fn into_config_defaults(self) -> ConfigDefaults {
        ConfigDefaults {
            owner: self
                .owner
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            license: self
                .license
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            funding: self
                .funding
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct RawWeights {
    identity: Option<f32>,
    docs: Option<f32>,
    community: Option<f32>,
    automation: Option<f32>,
    release: Option<f32>,
}

impl RawWeights {
    fn into_category_weights(self) -> io::Result<CategoryWeights> {
        Ok(CategoryWeights {
            identity: normalize_multiplier(self.identity.unwrap_or(1.0), "identity")?,
            docs: normalize_multiplier(self.docs.unwrap_or(1.0), "docs")?,
            community: normalize_multiplier(self.community.unwrap_or(1.0), "community")?,
            automation: normalize_multiplier(self.automation.unwrap_or(1.0), "automation")?,
            release: normalize_multiplier(self.release.unwrap_or(1.0), "release")?,
        })
    }
}

#[derive(Debug, Deserialize, Default)]
struct RawRuleOverride {
    enabled: Option<bool>,
    weight: Option<u16>,
    required_level: Option<RuleRequirement>,
}

impl RawRuleOverride {
    fn into_rule_override(self) -> RuleOverride {
        RuleOverride {
            enabled: self.enabled,
            weight: self.weight.filter(|weight| *weight > 0),
            required_level: self.required_level,
        }
    }
}

fn normalize_multiplier(value: f32, name: &str) -> io::Result<f32> {
    if !value.is_finite() || value <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("weight `{name}` must be a positive number"),
        ));
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_strict_threshold() {
        let config = OssifyConfig::default();
        assert_eq!(config.minimum_score(), 85);
        assert!(config.source().is_none());
    }

    #[test]
    fn config_parses_defaults_and_rules() {
        let root = std::env::temp_dir().join("ossify-config-test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp directory");
        fs::write(
            root.join("ossify.toml"),
            r#"
version = 1
profile = "cli"
minimum_score = 92

[defaults]
owner = "@acme"
license = "apache-2.0"
funding = "github:acme"

[weights]
docs = 1.5

[rules.readme]
required_level = "strong"
weight = 18
"#,
        )
        .expect("write config");

        let config = OssifyConfig::load_for_target(&root, None).expect("load config");
        assert_eq!(config.minimum_score(), 92);
        assert_eq!(config.profile_override(), Some(RepoProfile::Cli));
        assert_eq!(config.default_owner(), Some("@acme"));
        assert_eq!(config.default_license(), Some("apache-2.0"));
        assert_eq!(config.default_funding(), Some("github:acme"));
        assert_eq!(config.category_multiplier("docs"), 1.5);
        assert_eq!(config.rule_weight("readme"), Some(18));
        assert_eq!(
            config.rule_requirement("readme"),
            Some(RuleRequirement::Strong)
        );

        let _ = fs::remove_dir_all(&root);
    }
}
