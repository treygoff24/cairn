//! `cairn-config` — deterministic config loading, config hash, and feature/mode flags.
//!
//! # Forward compatibility policy
//!
//! Unknown top-level keys in the TOML file are silently stripped before
//! deserialization. This lets a newer config file (with fields this version
//! doesn't understand) load without error. The loader logs nothing about
//! stripping — it is a silent forward-compatibility concession, not a
//! diagnostic surface. Nested unknown keys within struct fields (e.g. inside
//! `[features]` or `[ablation]`) produce a deserialization error in Phase 1;
//! the policy may deepen in later phases.
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

use cairn_types::ConfigHash;

// ── Mode ────────────────────────────────────────────────────────────────────

/// Operating mode governing how aggressively the Cairn daemon enforces rules.
///
/// Phase 1 represents and loads the mode but does not act on it.
/// Enforcement semantics arrive in Phase 5 (hooks and MCP clients).
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

// ── Feature flags ──────────────────────────────────────────────────────────

/// Optional capabilities the daemon may activate at startup.
///
/// All flags are `true` by default except `multi_agent_coordination`, which is
/// opt-in during Phase 1 while the coordination substrate is still being built.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct FeatureFlags {
    /// Enable diagnostics watch loop and delta reporting.
    pub diagnostics: bool,
    /// Enable tree-sitter / language extractors.
    pub extract: bool,
    /// Enable context-frame injection into adapter sessions.
    pub context_injection: bool,
    /// Enable pre-edit and post-edit interception hooks.
    pub edit_interception: bool,
    /// Enable cross-session staleness arbitration.
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

// ── Ablation flags ─────────────────────────────────────────────────────────

/// Flags for selectively disabling subsystems (experiments, diagnostics).
///
/// All flags default to `false` — no ablation in normal operation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct AblationFlags {
    /// Disable the observation and edit ledgers.
    pub disable_ledger: bool,
    /// Disable content-hash freshness checks.
    pub disable_freshness_check: bool,
    /// Disable the dependency graph (if built).
    pub disable_graph: bool,
    /// Disable all adapter hooks.
    pub disable_hooks: bool,
    /// Disable the context-frame scheduler.
    pub disable_scheduler: bool,
}

// ── Top-level config ───────────────────────────────────────────────────────

/// The complete Cairn configuration, deserialized from TOML.
///
/// # Fields
///
/// * `protocol_version` — pinned wire-protocol version (mandatory). Mapped to
///   [`cairn_types::ProtocolVersion`] at load time. No default — a version
///   must be present in the config file.
/// * `mode` — enforcement mode (defaults to `Default`).
/// * `features` — optional capability flags (all-on defaults, see [`FeatureFlags`]).
/// * `ablation` — subsystem disable flags (all-off defaults, see [`AblationFlags`]).
/// * `project_name` — optional human-readable project label (unused in Phase 1,
///   reserved for diagnostics and the web UI).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CairnConfig {
    /// Pinned wire-protocol version. Must match the daemon and all adapters.
    pub protocol_version: u32,

    /// Enforcement mode.
    #[serde(default)]
    pub mode: CairnMode,

    /// Optional project name (reserved for diagnostics / web UI).
    ///
    /// Declared before the table-valued fields so the canonical TOML serialization
    /// used by [`CairnConfig::config_hash`] always emits scalars before tables —
    /// TOML requires this, and a trailing scalar would otherwise fail to serialize.
    #[serde(default)]
    pub project_name: Option<String>,

    /// Optional capability flags.
    #[serde(default)]
    pub features: FeatureFlags,

    /// Subsystem ablation flags.
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

// ── Known-key filter ───────────────────────────────────────────────────────

/// Known top-level config keys. Used to strip unknown keys for forward compat.
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

// ── Loading ────────────────────────────────────────────────────────────────

/// Errors produced during config loading and processing.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The TOML source could not be parsed.
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// The struct could not be serialized to canonical form.
    #[error("canonical serialization error: {0}")]
    CanonicalSerialize(String),

    /// Config file could not be read.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The deserialized config failed semantic validation.
    #[error("config validation error: {0}")]
    Validation(String),
}

/// Load configuration from a TOML string.
///
/// Unknown top-level keys are silently stripped (forward compatibility).
pub fn load_str(toml_str: &str) -> Result<CairnConfig, ConfigError> {
    // Parse into a raw table so we can strip unknown keys before
    // deserializing into the typed struct.
    let raw: toml::Value = toml::from_str(toml_str)?;
    let mut table = match raw {
        toml::Value::Table(t) => t,
        _ => return Err(ConfigError::Validation("top-level must be a table".into())),
    };
    strip_unknown_keys(&mut table);

    // Deserialize directly from the pruned TOML value — no string round-trip.
    // Unknown top-level keys were stripped above (forward compatibility); unknown
    // keys nested inside a known section are rejected by `deny_unknown_fields`.
    let config: CairnConfig = toml::Value::Table(table).try_into()?;

    if config.protocol_version == 0 {
        return Err(ConfigError::Validation(
            "protocol_version must be non-zero".into(),
        ));
    }

    Ok(config)
}

/// Load configuration from a TOML file path.
pub fn load_path<P: AsRef<Path>>(path: P) -> Result<CairnConfig, ConfigError> {
    let toml_str = std::fs::read_to_string(path)?;
    load_str(&toml_str)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_types::ConfigHash;

    // ═══════════════════════════════════════════════════════════════════════
    // Fixture helpers
    // ═══════════════════════════════════════════════════════════════════════

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

    // ═══════════════════════════════════════════════════════════════════════
    // Deterministic load
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn load_full_config() {
        let config = load_str(valid_toml()).unwrap();
        assert_eq!(config.protocol_version, 1);
        assert_eq!(config.mode, CairnMode::Default);
        assert!(config.features.diagnostics);
        assert!(!config.features.multi_agent_coordination);
        assert!(!config.ablation.disable_ledger);
        assert_eq!(config.project_name.as_deref(), Some("test-project"));
    }

    #[test]
    fn load_minimal_config() {
        let config = load_str(minimal_toml()).unwrap();
        assert_eq!(config.protocol_version, 1);
        assert_eq!(config.mode, CairnMode::Default); // default
        assert!(config.features.diagnostics); // default
        assert!(!config.ablation.disable_ledger); // default
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

    // ═══════════════════════════════════════════════════════════════════════
    // Config hash stability (key-order independence)
    // ═══════════════════════════════════════════════════════════════════════

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

        // Same logical config with keys in different order
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
        // The hex string should be non-empty lowercase hex
        let hex = hash.as_hex();
        assert!(!hex.is_empty(), "hex digest must not be empty");
        assert_eq!(hex.len(), 64, "BLAKE3 hex digest must be 64 chars");
        assert!(
            hex.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
            "hex digest must be lowercase"
        );
        // Round-trip through from_hex
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

    // ═══════════════════════════════════════════════════════════════════════
    // Forward compatibility — unknown keys
    // ═══════════════════════════════════════════════════════════════════════

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
        assert_eq!(config.protocol_version, 1);
        assert_eq!(config.mode, CairnMode::Default);
    }

    #[test]
    fn unknown_nested_keys_in_known_section_still_error() {
        // In Phase 1, unknown keys inside known sections (like [features])
        // still produce an error. This test documents the current boundary.
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
        assert_eq!(config.protocol_version, 2);
        assert_eq!(config.mode, CairnMode::Default);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Validation
    // ═══════════════════════════════════════════════════════════════════════

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

    // ═══════════════════════════════════════════════════════════════════════
    // File loading
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn load_path_round_trip() {
        // Write a temporary config file and load it via load_path.
        let dir = std::env::temp_dir().join(format!("cairn-config-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("test_config.toml");
        std::fs::write(&file_path, valid_toml()).unwrap();

        let config = load_path(&file_path).unwrap();
        assert_eq!(config.protocol_version, 1);

        // Cleanup
        let _ = std::fs::remove_file(&file_path);
        let _ = std::fs::remove_dir(&dir);
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
            protocol_version: 1,
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
