use clap::Parser;

mod cli;
mod config;
mod exit;
mod output;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = cli::Cli::parse();
    match run(cli).await {
        Ok(code) => code,
        Err(code) => code,
    }
}

async fn run(_cli: cli::Cli) -> Result<std::process::ExitCode, std::process::ExitCode> {
    Ok(std::process::ExitCode::SUCCESS)
}
