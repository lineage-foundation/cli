//! Config file and profiles for the `lineage` CLI.
//!
//! Profiles are loaded from `~/.config/lineage/config.toml` (via
//! `dirs::config_dir`). When the file is absent, a built-in `testnet`
//! profile is used as the default. `Config::resolve` picks a profile by
//! name and applies an optional `--network` override.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Name of the built-in default profile.
pub const DEFAULT_PROFILE: &str = "testnet";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignerKind {
    /// Sign locally with a keystore at `Profile::wallet_path`.
    Local,
    /// Delegate signing to the node (the deployed miner holds a wallet).
    Node,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confirm {
    /// Writes require `--yes` on the command line.
    Manual,
    /// Writes proceed without `--yes`.
    Auto,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Profile {
    /// The profile's name (its key in `Config::profiles`), used e.g. as the
    /// OS keyring account. Not read from the config file itself; set by
    /// `Config::resolve`.
    #[serde(default)]
    pub name: String,
    pub mempool: String,
    pub storage: String,
    pub miner: String,
    #[serde(default)]
    pub api_key: Option<String>,
    pub signer: SignerKind,
    #[serde(default)]
    pub wallet_path: Option<PathBuf>,
    #[serde(default)]
    pub max_amount: Option<f64>,
    #[serde(default)]
    pub daily_cap: Option<f64>,
    #[serde(default)]
    pub allowlist: Vec<String>,
    pub confirm: Confirm,
}

impl Profile {
    /// The built-in `testnet` profile used when no config file exists.
    pub fn testnet() -> Self {
        Self {
            name: DEFAULT_PROFILE.to_string(),
            mempool: "https://mempool.lineage.to".to_string(),
            storage: "https://storage.lineage.to".to_string(),
            miner: "https://miner.lineage.to".to_string(),
            api_key: None,
            signer: SignerKind::Node,
            wallet_path: None,
            max_amount: None,
            daily_cap: None,
            allowlist: Vec::new(),
            confirm: Confirm::Manual,
        }
    }

    /// Point all three hosts at a single base URL.
    fn set_network(&mut self, url: &str) {
        self.mempool = url.to_string();
        self.storage = url.to_string();
        self.miner = url.to_string();
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub default_profile: String,
    pub profiles: HashMap<String, Profile>,
}

impl Default for Config {
    fn default() -> Self {
        let mut profiles = HashMap::new();
        profiles.insert(DEFAULT_PROFILE.to_string(), Profile::testnet());
        Self {
            default_profile: DEFAULT_PROFILE.to_string(),
            profiles,
        }
    }
}

/// Errors loading or resolving the config.
#[derive(Debug)]
pub enum ConfigError {
    Read(String),
    Parse(String),
    UnknownProfile(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Read(msg) => write!(f, "could not read config file: {msg}"),
            ConfigError::Parse(msg) => write!(f, "could not parse config file: {msg}"),
            ConfigError::UnknownProfile(name) => write!(f, "no such profile: {name}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// Load the config from `~/.config/lineage/config.toml`, falling back
    /// to the built-in default when the directory can't be resolved.
    pub fn load() -> Result<Config, ConfigError> {
        match dirs::config_dir() {
            Some(dir) => Config::load_from(&dir.join("lineage").join("config.toml")),
            None => Ok(Config::default()),
        }
    }

    /// Load the config from an explicit path, returning the built-in
    /// default when the file does not exist. Exposed separately so tests
    /// can point at a tempdir file instead of the real home directory.
    pub fn load_from(path: &Path) -> Result<Config, ConfigError> {
        if !path.exists() {
            return Ok(Config::default());
        }
        let contents = fs::read_to_string(path).map_err(|err| ConfigError::Read(err.to_string()))?;
        toml::from_str(&contents).map_err(|err| ConfigError::Parse(err.to_string()))
    }

    /// Pick a profile by name (or the configured default) and apply an
    /// optional `--network` override: `"testnet"` selects the built-in
    /// testnet hosts, anything else is treated as a single base URL
    /// applied to all three hosts.
    pub fn resolve(
        &self,
        name: Option<&str>,
        network_override: Option<&str>,
    ) -> Result<Profile, ConfigError> {
        let key = name.unwrap_or(&self.default_profile);
        let mut profile = self
            .profiles
            .get(key)
            .cloned()
            .ok_or_else(|| ConfigError::UnknownProfile(key.to_string()))?;
        profile.name = key.to_string();

        if let Some(network) = network_override {
            if network == DEFAULT_PROFILE {
                let testnet = Profile::testnet();
                profile.mempool = testnet.mempool;
                profile.storage = testnet.storage;
                profile.miner = testnet.miner;
            } else {
                profile.set_network(network);
            }
        }

        Ok(profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_config(contents: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.toml");
        let mut file = fs::File::create(&path).expect("create config file");
        file.write_all(contents.as_bytes()).expect("write config file");
        (dir, path)
    }

    #[test]
    fn load_from_missing_file_returns_builtin_testnet_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("does-not-exist.toml");

        let config = Config::load_from(&path).expect("load default config");
        let profile = config.resolve(None, None).expect("resolve default profile");

        assert_eq!(config.default_profile, "testnet");
        assert_eq!(profile, Profile::testnet());
    }

    #[test]
    fn named_profile_loads_from_file() {
        let (_dir, path) = write_config(
            r#"
            default_profile = "dev"

            [profiles.dev]
            mempool = "http://localhost:8081"
            storage = "http://localhost:8082"
            miner = "http://localhost:8083"
            signer = "local"
            confirm = "manual"
            wallet_path = "/tmp/dev-wallet"

            [profiles.testnet]
            mempool = "https://mempool.lineage.to"
            storage = "https://storage.lineage.to"
            miner = "https://miner.lineage.to"
            signer = "node"
            confirm = "manual"
            "#,
        );

        let config = Config::load_from(&path).expect("load config");
        let profile = config
            .resolve(Some("dev"), None)
            .expect("resolve named profile");

        assert_eq!(profile.mempool, "http://localhost:8081");
        assert_eq!(profile.storage, "http://localhost:8082");
        assert_eq!(profile.miner, "http://localhost:8083");
        assert_eq!(profile.signer, SignerKind::Local);
        assert_eq!(profile.wallet_path, Some(PathBuf::from("/tmp/dev-wallet")));
    }

    #[test]
    fn network_override_with_url_sets_all_three_hosts() {
        let (_dir, path) = write_config(
            r#"
            default_profile = "dev"

            [profiles.dev]
            mempool = "http://localhost:8081"
            storage = "http://localhost:8082"
            miner = "http://localhost:8083"
            signer = "local"
            confirm = "manual"
            "#,
        );

        let config = Config::load_from(&path).expect("load config");
        let profile = config
            .resolve(Some("dev"), Some("http://mock:9999"))
            .expect("resolve with override");

        assert_eq!(profile.mempool, "http://mock:9999");
        assert_eq!(profile.storage, "http://mock:9999");
        assert_eq!(profile.miner, "http://mock:9999");
    }

    #[test]
    fn network_override_testnet_selects_builtin_testnet_hosts() {
        let (_dir, path) = write_config(
            r#"
            default_profile = "dev"

            [profiles.dev]
            mempool = "http://localhost:8081"
            storage = "http://localhost:8082"
            miner = "http://localhost:8083"
            signer = "local"
            confirm = "manual"
            "#,
        );

        let config = Config::load_from(&path).expect("load config");
        let profile = config
            .resolve(Some("dev"), Some("testnet"))
            .expect("resolve testnet override");

        assert_eq!(profile.mempool, "https://mempool.lineage.to");
        assert_eq!(profile.storage, "https://storage.lineage.to");
        assert_eq!(profile.miner, "https://miner.lineage.to");
    }

    #[test]
    fn unknown_profile_name_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("does-not-exist.toml");
        let config = Config::load_from(&path).expect("load default config");

        let result = config.resolve(Some("nope"), None);
        assert!(matches!(result, Err(ConfigError::UnknownProfile(_))));
    }
}
