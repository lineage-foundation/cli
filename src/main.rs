use std::process::ExitCode;

use clap::Parser;

mod cli;
mod commands;
mod config;
mod exit;
mod guard;
mod output;
mod secrets;

use cli::{Cli, Command, TxCommand, WalletCommand};
use config::{Config, ConfigError, Profile};
use exit::Code;
use output::Reporter;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    run(cli).await
}

async fn run(cli: Cli) -> ExitCode {
    let reporter = Reporter::new(cli.json, cli.quiet);

    let profile = match resolve_profile(&cli) {
        Ok(profile) => profile,
        Err(err) => {
            reporter.fail(Code::Usage, &err.to_string(), None);
            return Code::Usage.into();
        }
    };

    let (yes, dry_run) = (cli.yes, cli.dry_run);

    match cli.command {
        Command::Supply => commands::read::supply(&profile, &reporter).await,
        Command::Balance { addresses } => commands::read::balance(&profile, &reporter, &addresses).await,
        Command::Blocks { target, nums } => {
            commands::read::blocks(&profile, &reporter, target.as_deref(), &nums).await
        }
        Command::Entries { key } => commands::read::entries(&profile, &reporter, &key).await,
        Command::Tx { command } => match command {
            TxCommand::Status { hash } => commands::read::tx_status(&profile, &reporter, &hash).await,
            TxCommand::Submit { file } => commands::write::tx_submit(&profile, &reporter, &file, yes).await,
            TxCommand::Serialize { file } => commands::write::tx_serialize(&profile, &reporter, &file).await,
            TxCommand::Deserialize { file } => commands::write::tx_deserialize(&profile, &reporter, &file).await,
        },
        Command::Mining => commands::read::mining(&profile, &reporter).await,
        Command::Debug { node } => commands::read::debug(&profile, &reporter, node).await,
        Command::Wallet { command } => match command {
            WalletCommand::New { force } => commands::wallet::new(&profile, &reporter, force).await,
            WalletCommand::Address => commands::wallet::address(&profile, &reporter).await,
            WalletCommand::List => commands::wallet::list(&profile, &reporter).await,
            WalletCommand::Import { node, file } => {
                commands::wallet::import(&profile, &reporter, node, &file).await
            }
            WalletCommand::Passphrase { node, new } => {
                commands::wallet::passphrase(&profile, &reporter, node, &new).await
            }
            WalletCommand::Refresh { node, addresses } => {
                commands::wallet::refresh(&profile, &reporter, node, &addresses).await
            }
        },
        Command::Pay { address, amount } => {
            commands::pay::run(&profile, &reporter, &address, amount, yes, dry_run).await
        }
        Command::Items { file } => commands::write::items(&profile, &reporter, &file, yes).await,
        Command::Donate { target } => commands::write::donate(&profile, &reporter, &target, yes).await,
    }
}

/// Load the config file and resolve the profile named by `--profile`,
/// applying the `--network` override.
fn resolve_profile(cli: &Cli) -> Result<Profile, ConfigError> {
    let config = Config::load()?;
    config.resolve(cli.profile.as_deref(), cli.network.as_deref())
}
