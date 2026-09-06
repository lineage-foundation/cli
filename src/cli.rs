use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "lineage", version, about = "Lineage CLI")]
pub struct Cli {
    /// Emit machine-readable JSON output
    #[arg(long, global = true)]
    pub json: bool,

    /// Confirm write operations without an interactive prompt
    #[arg(long, global = true)]
    pub yes: bool,

    /// Print the intended action without performing any network write
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Profile name to use from the config file
    #[arg(long, global = true)]
    pub profile: Option<String>,

    /// Network to target: "testnet" or a custom URL
    #[arg(long, global = true)]
    pub network: Option<String>,

    /// Suppress non-essential output
    #[arg(long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Query the current token supply
    Supply,

    /// Look up balances for one or more addresses
    Balance {
        /// Addresses to look up
        #[arg(required = true, num_args = 1..)]
        addresses: Vec<String>,
    },

    /// Fetch blocks: the latest block, a single block by number, or a batch
    Blocks {
        /// "latest" or a specific block number
        target: Option<String>,

        /// Comma-separated block numbers to fetch in a batch
        #[arg(long, value_delimiter = ',', num_args = 1..)]
        nums: Vec<u64>,
    },

    /// Fetch a blockchain entry by key
    Entries {
        /// Blockchain entry key
        key: String,
    },

    /// Transaction-related commands
    Tx {
        #[command(subcommand)]
        command: TxCommand,
    },

    /// Show the block currently being mined
    Mining,

    /// Fetch debug information from a node
    Debug {
        /// Which node to query
        node: NodeArg,
    },

    /// Local keystore and node-wallet management
    Wallet {
        #[command(subcommand)]
        command: WalletCommand,
    },

    /// Pay an address using the profile's signer backend
    Pay {
        /// Recipient address
        address: String,

        /// Amount in LNGX (decimals accepted)
        amount: f64,
    },
}

#[derive(Subcommand, Debug)]
pub enum WalletCommand {
    /// Create a new local wallet keystore at the profile's `wallet_path`
    New,

    /// Generate and persist a new address in the local wallet
    Address,

    /// List addresses held in the local wallet
    List,

    /// Import keypairs into a node's wallet
    Import {
        /// Which node's wallet to import into
        node: NodeArg,

        /// Path to a JSON file containing the keypairs payload
        file: PathBuf,
    },

    /// Change the passphrase of a node's wallet
    Passphrase {
        /// Which node's wallet to update
        node: NodeArg,

        /// The new passphrase
        new: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum TxCommand {
    /// Look up the status of a transaction by hash
    Status {
        /// Transaction hash
        hash: String,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum NodeArg {
    Mempool,
    Storage,
    Miner,
}

impl From<NodeArg> for lineage_sdk::NodeClass {
    fn from(node: NodeArg) -> Self {
        match node {
            NodeArg::Mempool => lineage_sdk::NodeClass::Mempool,
            NodeArg::Storage => lineage_sdk::NodeClass::Storage,
            NodeArg::Miner => lineage_sdk::NodeClass::Miner,
        }
    }
}
