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

/// Contents written to `config.toml` on first run, when no config file
/// exists yet at the resolved config path.
pub const DEFAULT_CONFIG_TOML: &str = r#"default_profile = "testnet"

[profiles.testnet]
mempool = "https://mempool.lineage.to"
storage = "https://storage.lineage.to"
miner   = "https://miner.lineage.to"
signer  = "node"
confirm = "manual"

[profiles.local]
mempool = "https://mempool.lineage.to"
storage = "https://storage.lineage.to"
miner   = "https://miner.lineage.to"
signer  = "local"
confirm = "manual"
wallet_path = "~/.lineage/wallet.json"
"#;

/// Expand a leading `~` component to the user's home directory. Only a
/// *leading* `~` is treated specially (`~` alone, or `~` as the first path
/// component, e.g. `~/foo`); a `~` anywhere else in the path is left as
/// literal text. Returns `path` unchanged if the home directory can't be
/// resolved.
pub fn expand_tilde(path: &Path) -> PathBuf {
    let mut components = path.components();
    match components.next() {
        Some(std::path::Component::Normal(first)) if first == "~" => match dirs::home_dir() {
            Some(home) => home.join(components.as_path()),
            None => path.to_path_buf(),
        },
        _ => path.to_path_buf(),
    }
}

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

    /// `wallet_path` with a leading `~` expanded to the home directory, so
    /// consumers never have to deal with an unexpanded path.
    pub fn resolved_wallet_path(&self) -> Option<PathBuf> {
        self.wallet_path.as_ref().map(|path| expand_tilde(path))
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
    /// Load the config from `~/.config/lineage/config.toml`. On first run
    /// (the file doesn't exist yet) a default config is scaffolded there;
    /// falls back to the built-in default when the config directory can't
    /// be resolved.
    pub fn load() -> Result<Config, ConfigError> {
        match dirs::config_dir() {
            Some(dir) => Config::load_or_init(&dir.join("lineage").join("config.toml")),
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

    /// Load the config from an explicit path, scaffolding it with
    /// [`DEFAULT_CONFIG_TOML`] first if it doesn't exist yet. If the
    /// parent directory or file can't be written (e.g. permissions), falls
    /// back to the built-in default rather than erroring. Exposed
    /// separately so tests can point at a tempdir path.
    pub fn load_or_init(path: &Path) -> Result<Config, ConfigError> {
        if path.exists() {
            return Config::load_from(path);
        }
        Ok(Config::scaffold_default_at(path))
    }

    /// Write [`DEFAULT_CONFIG_TOML`] to `path` (creating its parent
    /// directory first) and parse it back. Any failure along the way — the
    /// directory can't be created, the file can't be written, or the
    /// written content somehow fails to parse — falls back to
    /// `Config::default()` rather than erroring.
    fn scaffold_default_at(path: &Path) -> Config {
        let parent_ok = match path.parent() {
            Some(parent) => fs::create_dir_all(parent).is_ok(),
            None => true,
        };
        if parent_ok && fs::write(path, DEFAULT_CONFIG_TOML).is_ok() {
            if let Ok(config) = toml::from_str(DEFAULT_CONFIG_TOML) {
                return config;
            }
        }
        Config::default()
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

    #[test]
    fn expand_tilde_expands_leading_tilde_to_home_dir() {
        let home = dirs::home_dir().expect("home dir available in test env");
        let expanded = expand_tilde(Path::new("~"));
        assert_eq!(expanded, home);
    }

    #[test]
    fn expand_tilde_expands_leading_tilde_with_subpath_to_home_dir() {
        let home = dirs::home_dir().expect("home dir available in test env");
        let expanded = expand_tilde(Path::new("~/.lineage/wallet.json"));
        assert_eq!(expanded, home.join(".lineage").join("wallet.json"));
    }

    #[test]
    fn expand_tilde_leaves_absolute_path_unchanged() {
        let path = Path::new("/tmp/dev-wallet");
        assert_eq!(expand_tilde(path), path);
    }

    #[test]
    fn expand_tilde_leaves_relative_non_tilde_path_unchanged() {
        let path = Path::new("some/relative/path");
        assert_eq!(expand_tilde(path), path);
    }

    #[test]
    fn resolved_wallet_path_expands_tilde() {
        let home = dirs::home_dir().expect("home dir available in test env");
        let mut profile = Profile::testnet();
        profile.wallet_path = Some(PathBuf::from("~/.lineage/wallet.json"));

        assert_eq!(
            profile.resolved_wallet_path(),
            Some(home.join(".lineage").join("wallet.json"))
        );
    }

    #[test]
    fn resolved_wallet_path_is_none_when_unset() {
        let profile = Profile::testnet();
        assert_eq!(profile.resolved_wallet_path(), None);
    }

    #[test]
    fn load_or_init_scaffolds_default_config_when_file_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("lineage").join("config.toml");
        assert!(!path.exists());

        let config = Config::load_or_init(&path).expect("load_or_init");

        assert!(path.exists(), "load_or_init should write the config file");
        let written = fs::read_to_string(&path).expect("read scaffolded config");
        assert_eq!(written, DEFAULT_CONFIG_TOML);

        assert_eq!(config.default_profile, "testnet");
        let testnet = config.profiles.get("testnet").expect("testnet profile");
        assert_eq!(testnet.signer, SignerKind::Node);

        let local = config.profiles.get("local").expect("local profile");
        assert_eq!(local.signer, SignerKind::Local);
        assert_eq!(
            local.wallet_path,
            Some(PathBuf::from("~/.lineage/wallet.json"))
        );
    }

    #[test]
    fn load_or_init_loads_existing_file_without_overwriting() {
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

        let config = Config::load_or_init(&path).expect("load_or_init existing file");

        assert_eq!(config.default_profile, "dev");
        assert!(config.profiles.contains_key("dev"));
    }
}
