pub mod app;

/// Entry point for `lineage tui`. This is a stub for now: terminal setup,
/// the event loop, and teardown are implemented once the `App` state
/// machine, data fetches, and rendering are in place.
pub async fn run(_profile: &crate::config::Profile) -> std::process::ExitCode {
    std::process::ExitCode::SUCCESS
}
