//! `cairn-config` — deterministic config loading, config hash, and feature/mode flags.
//!
//! # Forward compatibility policy
//!
//! Unknown top-level keys in the TOML file are silently stripped before
//! deserialization. This lets a newer config file (with fields this version
//! doesn't understand) load without error. The loader logs nothing about
//! stripping — it is a silent forward-compatibility concession, not a
//! diagnostic surface. Nested unknown keys within struct fields (e.g. inside
//! `[features]` or `[ablation]`) produce a deserialization error.
//!
//! # Config hash stability (key-order independence)
//!
//! The config hash is computed by serializing the *deserialized struct* to a
//! canonical TOML string (field order = struct declaration order) and hashing
//! that with BLAKE3. Two config files that differ only in key ordering produce
//! identical structs and therefore identical hashes.

use std::path::Path;

use blake3::Hash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use cairn_types::{ConfigHash, ProtocolVersion};

/// Operating mode governing how aggressively the Cairn daemon enforces rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CairnMode {
    /// Deny edits that violate staleness or safety checks (most aggressive).
    Strict,
    /// Warn on violations but allow the edit to proceed (sensible default).
    #[default]
    Default,
    /// Log violations silently; never block or surface warnings (quietest).
    Advisory,
}

/// Optional capabilities the daemon may activate at startup.
///
/// All flags are `true` by default except `multi_agent_coordination`, which is
/// opt-in.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct FeatureFlags {
    pub diagnostics: bool,
    pub extract: bool,
    pub context_injection: bool,
    pub edit_interception: bool,
    pub multi_agent_coordination: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            diagnostics: true,
            extract: true,
            context_injection: true,
            edit_interception: true,
            multi_agent_coordination: false,
        }
    }
}

/// Flags for selectively disabling subsystems (experiments, diagnostics).
///
/// All flags default to `false` — no ablation in normal operation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct AblationFlags {
    pub disable_ledger: bool,
    pub disable_freshness_check: bool,
    pub disable_graph: bool,
    pub disable_hooks: bool,
    pub disable_scheduler: bool,
}

/// The complete Cairn configuration, deserialized from TOML.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CairnConfig {
    /// Pinned wire-protocol version. Must match the daemon and all adapters.
    pub protocol_version: ProtocolVersion,

    #[serde(default)]
    pub mode: CairnMode,

    /// Optional human-readable project name.
    ///
    /// Declared before the table-valued fields so the canonical TOML serialization
    /// used by [`CairnConfig::config_hash`] always emits scalars before tables —
    /// TOML requires this, and a trailing scalar would otherwise fail to serialize.
    #[serde(default)]
    pub project_name: Option<String>,

    #[serde(default)]
    pub features: FeatureFlags,

    #[serde(default)]
    pub ablation: AblationFlags,
}

impl CairnConfig {
    /// Compute the deterministic [`ConfigHash`] for this configuration.
    ///
    /// Serializes the struct to a canonical TOML string (field order follows
    /// struct declaration order), hashes with BLAKE3, and wraps the lowercase
    /// hex digest in [`ConfigHash::from_hex`].
    ///
    /// Two `CairnConfig` values with the same fields always produce the same
    /// hash, regardless of how the original TOML keys were ordered on disk.
    pub fn config_hash(&self) -> Result<ConfigHash, ConfigError> {
        let canonical =
            toml::to_string(self).map_err(|e| ConfigError::CanonicalSerialize(e.to_string()))?;
        let hash: Hash = blake3::hash(canonical.as_bytes());
        Ok(ConfigHash::from_hex(hash.to_hex().to_string()))
    }
}

const KNOWN_TOP_LEVEL_KEYS: &[&str] = &[
    "protocol_version",
    "mode",
    "features",
    "ablation",
    "project_name",
];

/// Remove unknown top-level keys from a `toml::map::Map`.
///
/// This is a silent forward-compatibility measure: a newer config file with
/// keys this version does not understand will load without error, with the
/// unknown keys stripped before serde deserialization.
fn strip_unknown_keys(table: &mut toml::map::Map<String, toml::Value>) {
    let known: &[&str] = KNOWN_TOP_LEVEL_KEYS;
    table.retain(|k, _| {
        let s: &str = k;
        known.contains(&s)
    });
}

/// Errors produced during config loading and processing.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("canonical serialization error: {0}")]
    CanonicalSerialize(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("config validation error: {0}")]
    Validation(String),
}

/// Load configuration from a TOML string.
///
/// Unknown top-level keys are silently stripped (forward compatibility).
pub fn load_str(toml_str: &str) -> Result<CairnConfig, ConfigError> {
    let mut table: toml::Table = toml::from_str(toml_str)?;
    strip_unknown_keys(&mut table);

    // Nested keys still pass through each section's `deny_unknown_fields` check.
    let config: CairnConfig = toml::Value::Table(table).try_into()?;

    if config.protocol_version.0 == 0 {
        return Err(ConfigError::Validation(
            "protocol_version must be non-zero".into(),
        ));
    }

    Ok(config)
}

pub fn load_path<P: AsRef<Path>>(path: P) -> Result<CairnConfig, ConfigError> {
    let toml_str = std::fs::read_to_string(path)?;
    load_str(&toml_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_types::ConfigHash;

    fn valid_toml() -> &'static str {
        r#"
protocol_version = 1
mode = "default"
project_name = "test-project"

[features]
diagnostics = true
extract = true
context_injection = true
edit_interception = true
multi_agent_coordination = false

[ablation]
disable_ledger = false
disable_freshness_check = false
disable_graph = false
disable_hooks = false
disable_scheduler = false
"#
    }

    fn minimal_toml() -> &'static str {
        r#"protocol_version = 1"#
    }

    #[test]
    fn load_full_config() {
        let config = load_str(valid_toml()).unwrap();
        assert_eq!(config.protocol_version, ProtocolVersion(1));
        assert_eq!(config.mode, CairnMode::Default);
        assert!(config.features.diagnostics);
        assert!(!config.features.multi_agent_coordination);
        assert!(!config.ablation.disable_ledger);
        assert_eq!(config.project_name.as_deref(), Some("test-project"));
    }

    #[test]
    fn load_minimal_config() {
        let config = load_str(minimal_toml()).unwrap();
        assert_eq!(config.protocol_version, ProtocolVersion(1));
        assert_eq!(config.mode, CairnMode::Default);
        assert!(config.features.diagnostics);
        assert!(!config.ablation.disable_ledger);
        assert!(config.project_name.is_none());
    }

    #[test]
    fn all_modes_round_trip() {
        for (toml_val, expected) in [
            ("\"strict\"", CairnMode::Strict),
            ("\"default\"", CairnMode::Default),
            ("\"advisory\"", CairnMode::Advisory),
        ] {
            let toml_str = format!("protocol_version = 1\nmode = {toml_val}");
            let config = load_str(&toml_str).unwrap();
            assert_eq!(config.mode, expected, "mode round-trip for {toml_val}");
        }
    }

    #[test]
    fn default_mode_when_omitted() {
        let config = load_str(minimal_toml()).unwrap();
        assert_eq!(config.mode, CairnMode::Default);
    }

    #[test]
    fn config_hash_equal_for_reordered_keys() {
        let toml_a = r#"
protocol_version = 1
mode = "strict"

[features]
diagnostics = true
extract = true
context_injection = false
edit_interception = true
multi_agent_coordination = false

[ablation]
disable_ledger = false
disable_freshness_check = true
disable_graph = false
disable_hooks = false
disable_scheduler = false
"#;

        let toml_b = r#"
mode = "strict"
protocol_version = 1

[ablation]
disable_ledger = false
disable_freshness_check = true
disable_scheduler = false
disable_graph = false
disable_hooks = false

[features]
multi_agent_coordination = false
extract = true
diagnostics = true
edit_interception = true
context_injection = false
"#;

        let config_a = load_str(toml_a).unwrap();
        let config_b = load_str(toml_b).unwrap();

        assert_eq!(config_a, config_b, "configs must be semantically equal");
        assert_eq!(
            config_a.config_hash().unwrap(),
            config_b.config_hash().unwrap(),
            "config hash must be stable across key ordering"
        );
    }

    #[test]
    fn config_hash_different_for_different_values() {
        let config_default = load_str("protocol_version = 1\nmode = \"default\"").unwrap();
        let config_strict = load_str("protocol_version = 1\nmode = \"strict\"").unwrap();
        assert_ne!(
            config_default.config_hash().unwrap(),
            config_strict.config_hash().unwrap(),
            "different configs must produce different hashes"
        );
    }

    #[test]
    fn config_hash_reproducible() {
        let config = load_str(valid_toml()).unwrap();
        let hash_a = config.config_hash().unwrap();
        let hash_b = config.config_hash().unwrap();
        assert_eq!(hash_a, hash_b, "hash must be reproducible");
    }

    #[test]
    fn config_hash_from_hex_round_trip() {
        let config = load_str(valid_toml()).unwrap();
        let hash = config.config_hash().unwrap();
        let hex = hash.as_hex();
        assert!(!hex.is_empty(), "hex digest must not be empty");
        assert_eq!(hex.len(), 64, "BLAKE3 hex digest must be 64 chars");
        assert!(
            hex.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
            "hex digest must be lowercase"
        );
        let hash2 = ConfigHash::from_hex(hex);
        assert_eq!(hash, hash2);
    }

    #[test]
    fn config_hash_works_with_project_name() {
        // Regression: a config with a project_name set must still hash. The scalar
        // must serialize before the table fields, or canonical TOML serialization
        // fails. `valid_toml()` carries a project_name.
        let config = load_str(valid_toml()).unwrap();
        assert_eq!(config.project_name.as_deref(), Some("test-project"));
        let hash = config
            .config_hash()
            .expect("config_hash must succeed when project_name is set");
        assert_eq!(hash.as_hex().len(), 64);
    }

    #[test]
    fn unknown_top_level_keys_dont_crash() {
        let toml = r#"
protocol_version = 1
unknown_key = "some_value"
another_unknown = 42

[unknown_section]
foo = "bar"
"#;
        let config = load_str(toml).unwrap();
        assert_eq!(config.protocol_version, ProtocolVersion(1));
        assert_eq!(config.mode, CairnMode::Default);
    }

    #[test]
    fn unknown_nested_keys_in_known_section_still_error() {
        let toml = r#"
protocol_version = 1

[features]
diagnostics = true
unknown_nested = "boom"
"#;
        let result = load_str(toml);
        assert!(
            result.is_err(),
            "nested unknown keys should error in Phase 1"
        );
    }

    #[test]
    fn all_unknown_keys_ignored() {
        let toml = r#"
protocol_version = 2
foobar = 1
baz = "qux"

[unknown_table]
x = 1
y = 2
"#;
        let config = load_str(toml).unwrap();
        assert_eq!(config.protocol_version, ProtocolVersion(2));
        assert_eq!(config.mode, CairnMode::Default);
    }

    #[test]
    fn zero_protocol_version_rejected() {
        let result = load_str("protocol_version = 0");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("protocol_version must be non-zero"),
            "error message must mention the zero-version rejection"
        );
    }

    #[test]
    fn negative_protocol_version_rejected() {
        let result = load_str("protocol_version = -1");
        assert!(result.is_err());
    }

    #[test]
    fn invalid_toml_rejected() {
        let result = load_str("this is not valid toml {{{");
        assert!(result.is_err());
    }

    #[test]
    fn load_path_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!("cairn-config-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        let file_path = dir.join("test_config.toml");
        std::fs::write(&file_path, valid_toml())?;

        let config = load_path(&file_path)?;
        assert_eq!(config.protocol_version, ProtocolVersion(1));

        std::fs::remove_file(&file_path)?;
        std::fs::remove_dir(&dir)?;
        Ok(())
    }

    #[test]
    fn load_nonexistent_path_errors() {
        let result = load_path("/tmp/cairn-nonexistent-test-file.toml");
        assert!(result.is_err());
    }

    #[test]
    fn known_keys_cover_all_config_fields() {
        // Guard against `KNOWN_TOP_LEVEL_KEYS` drifting from the struct: a new field
        // not in the allowlist would be silently stripped on load (data loss).
        let cfg = CairnConfig {
            protocol_version: ProtocolVersion(1),
            mode: CairnMode::Default,
            project_name: Some("x".into()),
            features: FeatureFlags::default(),
            ablation: AblationFlags::default(),
        };
        let toml::Value::Table(table) = toml::Value::try_from(&cfg).unwrap() else {
            panic!("CairnConfig must serialize to a table");
        };
        for key in table.keys() {
            assert!(
                KNOWN_TOP_LEVEL_KEYS.contains(&key.as_str()),
                "config field `{key}` is missing from KNOWN_TOP_LEVEL_KEYS"
            );
        }
    }
}
