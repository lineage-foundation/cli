//! Command implementations shared infrastructure: building an SDK client
//! from a resolved profile, and mapping SDK errors through the `Reporter`.

use std::process::ExitCode;

use crate::config::Profile;
use crate::exit::Code;
use crate::output::{render_error, Reporter};

pub mod pay;
pub mod read;
pub mod wallet;
pub mod write;

/// Build an SDK client from a resolved profile's hosts and (optional)
/// API key.
pub fn build_client(profile: &Profile) -> lineage_sdk::Result<lineage_sdk::Client> {
    let hosts = lineage_sdk::Hosts {
        mempool: profile.mempool.clone(),
        storage: profile.storage.clone(),
        miner: profile.miner.clone(),
    };
    let client = lineage_sdk::Client::new(hosts)?;
    Ok(match &profile.api_key {
        Some(key) => client.with_api_key(key.clone()),
        None => client,
    })
}

/// Report an SDK error through the `Reporter` and return the mapped exit
/// code.
pub fn report_sdk_error(reporter: &Reporter, err: &lineage_sdk::Error) -> ExitCode {
    let (code, detail) = render_error(err);
    reporter.fail(code, &err.to_string(), Some(detail));
    code.into()
}

/// Shorthand for the success exit code.
pub fn ok_code() -> ExitCode {
    Code::Ok.into()
}
