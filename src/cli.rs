use clap::{Parser, Subcommand};

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
}
