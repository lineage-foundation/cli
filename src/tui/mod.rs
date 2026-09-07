//! `lineage tui`: terminal setup/teardown and the event loop.
//!
//! This file is intentionally thin and has no automated tests: it owns raw
//! terminal I/O (`crossterm`) and an async `tokio::select!` loop, neither of
//! which lends itself to a unit test. Every decision the loop makes is
//! delegated to the tested `App` state machine (`app::App::on_key` and the
//! `send_*` methods) or the tested `fetch::*` functions; this module only
//! wires crossterm/tick/channel events to those calls, draws each frame,
//! and awaits the submit call inline. It is verified by running
//! `lineage tui` by hand.

pub mod app;
pub mod fetch;
pub mod ui;

use std::io;
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, ExecutableCommand};
use futures_util::StreamExt;
use lineage_sdk::{Client, LocalSigner, NodeSigner, Receipt, Signer, Tokens, Wallet};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::Terminal;
use tokio::sync::mpsc;
use tokio::time::interval;

use app::{App, SendStep, Tab};
use crate::config::{Profile, SignerKind};
use crate::secrets;

/// Results delivered from the spawned background fetches.
enum FetchMsg {
    Dashboard(lineage_sdk::Result<fetch::DashboardData>),
    Wallet(lineage_sdk::Result<lineage_sdk::models::BalancesResponse>),
}

/// Entry point for `lineage tui`: sets up the terminal, runs the event
/// loop, and always restores the terminal on the way out (including on a
/// panic, via a hook installed here).
pub async fn run(profile: &Profile) -> ExitCode {
    if let Err(err) = enable_raw_mode() {
        eprintln!("lineage tui: failed to enable raw mode: {err}");
        return ExitCode::FAILURE;
    }
    if let Err(err) = io::stdout().execute(EnterAlternateScreen) {
        eprintln!("lineage tui: failed to enter alternate screen: {err}");
        let _ = disable_raw_mode();
        return ExitCode::FAILURE;
    }

    install_panic_hook();

    let backend = CrosstermBackend::new(io::stdout());
    let exit_code = match Terminal::new(backend) {
        Ok(mut terminal) => event_loop(profile, &mut terminal).await,
        Err(err) => {
            eprintln!("lineage tui: failed to start terminal: {err}");
            ExitCode::FAILURE
        }
    };

    restore_terminal();
    exit_code
}

/// Disable raw mode and leave the alternate screen. Best-effort: errors are
/// logged to stderr rather than propagated, since this runs on every exit
/// path (including after a panic) and must not itself panic.
fn restore_terminal() {
    if let Err(err) = disable_raw_mode() {
        eprintln!("lineage tui: failed to disable raw mode: {err}");
    }
    if let Err(err) = execute!(io::stdout(), LeaveAlternateScreen) {
        eprintln!("lineage tui: failed to leave alternate screen: {err}");
    }
}

/// Install a panic hook that restores the terminal before running the
/// default hook, so a panic mid-render doesn't leave the user's terminal in
/// raw/alternate-screen mode.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));
}

/// The main render/input/refresh loop, run once the terminal is set up.
async fn event_loop<B: Backend>(profile: &Profile, terminal: &mut Terminal<B>) -> ExitCode {
    let client = match crate::commands::build_client(profile) {
        Ok(client) => client,
        Err(err) => {
            eprintln!("lineage tui: failed to build client: {err}");
            return ExitCode::FAILURE;
        }
    };

    let addresses = watched_addresses(profile);
    let mut app = App::new(profile.name.clone());

    let (tx, mut rx) = mpsc::channel::<FetchMsg>(8);
    spawn_fetches(&client, &addresses, &tx);

    let mut events = EventStream::new();
    let mut ticker = interval(Duration::from_secs(3));
    // The first tick fires immediately; skip it since `spawn_fetches` above
    // already kicked off the initial load.
    ticker.tick().await;

    loop {
        if let Err(err) = terminal.draw(|f| ui::draw(f, &app)) {
            eprintln!("lineage tui: failed to draw frame: {err}");
            return ExitCode::FAILURE;
        }

        if app.should_quit {
            return ExitCode::SUCCESS;
        }

        tokio::select! {
            _ = ticker.tick() => {
                spawn_fetches(&client, &addresses, &tx);
            }
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                            app.should_quit = true;
                        } else {
                            handle_key(&mut app, profile, key.code);
                        }

                        if app.new_address_requested {
                            app.new_address_requested = false;
                            handle_new_address_request(profile, &mut app);
                        }

                        if app.refresh_requested {
                            app.refresh_requested = false;
                            spawn_fetches(&client, &addresses, &tx);
                        }

                        if app.send_step == SendStep::Submitting {
                            // Show the "Submitting…" frame before blocking
                            // on the network call below.
                            if let Err(err) = terminal.draw(|f| ui::draw(f, &app)) {
                                eprintln!("lineage tui: failed to draw frame: {err}");
                                return ExitCode::FAILURE;
                            }
                            submit_send(&client, profile, &mut app).await;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(err)) => {
                        eprintln!("lineage tui: input error: {err}");
                    }
                    None => app.should_quit = true,
                }
            }
            Some(msg) = rx.recv() => {
                apply_fetch(&mut app, msg);
            }
        }
    }
}

/// Route a key press to the tested `App` methods. Outside the Send tab (or
/// once a send has reached `Done`), keys go through the global
/// `App::on_key` map (`q`/`Tab`/`1-3`/`r`). While filling in or confirming
/// the send form, printable keys are routed to the form/passphrase input
/// instead, so addresses and amounts can contain digits without triggering
/// tab switches.
fn handle_key(app: &mut App, profile: &Profile, key: KeyCode) {
    if app.active != Tab::Send {
        app.on_key(key);
        return;
    }

    match app.send_step {
        SendStep::Form => match key {
            KeyCode::Enter => {
                app.send_advance(profile);
            }
            KeyCode::Tab => app.send_field_next(),
            KeyCode::Backspace => app.send_backspace(),
            KeyCode::Esc => app.active = Tab::Dashboard,
            KeyCode::Char(c) => app.send_input_char(c),
            _ => {}
        },
        SendStep::Review => match key {
            KeyCode::Enter => app.send_step = SendStep::Confirm,
            KeyCode::Esc => app.send_back(),
            _ => {}
        },
        SendStep::Confirm => match key {
            KeyCode::Enter => app.send_step = SendStep::Submitting,
            KeyCode::Esc => app.send_back(),
            KeyCode::Backspace => {
                app.send_passphrase.pop();
            }
            KeyCode::Char(c) => app.send_passphrase.push(c),
            _ => {}
        },
        SendStep::Submitting => {}
        SendStep::Done => match key {
            KeyCode::Enter | KeyCode::Esc => reset_send(app),
            _ => app.on_key(key),
        },
    }
}

/// Clear the send form back to a blank `Form` step after a completed (or
/// failed-and-abandoned) send, without disturbing the rest of `App`.
fn reset_send(app: &mut App) {
    app.send_to.clear();
    app.send_amount.clear();
    app.send_passphrase.clear();
    app.send_error = None;
    app.send_receipt = None;
    app.send_step = SendStep::Form;
}

/// Spawn the dashboard and wallet fetches concurrently, delivering their
/// results over `tx`. Errors are sent along, not swallowed, so the loop can
/// surface them via `App::status`.
fn spawn_fetches(client: &Client, addresses: &[String], tx: &mpsc::Sender<FetchMsg>) {
    let dashboard_client = client.clone();
    let dashboard_addresses = addresses.to_vec();
    let dashboard_tx = tx.clone();
    tokio::spawn(async move {
        let result = fetch::fetch_dashboard(&dashboard_client, &dashboard_addresses).await;
        let _ = dashboard_tx.send(FetchMsg::Dashboard(result)).await;
    });

    let wallet_client = client.clone();
    let wallet_addresses = addresses.to_vec();
    let wallet_tx = tx.clone();
    tokio::spawn(async move {
        let result = fetch::fetch_wallet(&wallet_client, &wallet_addresses).await;
        let _ = wallet_tx.send(FetchMsg::Wallet(result)).await;
    });
}

/// Apply a delivered fetch result to the cached `App` state via its tested
/// `apply_*` methods, or surface the error in the status line.
fn apply_fetch(app: &mut App, msg: FetchMsg) {
    match msg {
        FetchMsg::Dashboard(Ok(data)) => {
            app.apply_head(data.head);
            app.apply_supply(data.supply);
            app.apply_wallet_total(data.wallet_total_raw);
            app.set_updated(wall_clock_stamp());
            app.status = None;
        }
        FetchMsg::Dashboard(Err(err)) => {
            app.status = Some(format!("dashboard refresh failed: {err}"));
        }
        FetchMsg::Wallet(Ok(balances)) => {
            app.apply_wallet_balances(&balances);
            app.set_updated(wall_clock_stamp());
        }
        FetchMsg::Wallet(Err(err)) => {
            app.status = Some(format!("wallet refresh failed: {err}"));
        }
    }
}

/// A genuine wall-clock "last updated" stamp (UTC time of day), computed
/// from `SystemTime::now()` at the moment a refresh lands. Not a fake tick
/// counter: this reflects the real clock, just without pulling in a
/// timezone/date-formatting crate for what's otherwise a one-line label.
fn wall_clock_stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs();
    let secs_of_day = secs % 86_400;
    let (h, m, s) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    format!("{h:02}:{m:02}:{s:02} UTC")
}

/// Addresses to watch for the Dashboard/Wallet views: a local wallet's
/// addresses when the profile signs locally and has a `wallet_path`
/// configured, otherwise empty (the node-signer path has no local
/// addresses to enumerate). Failing to resolve a passphrase or open the
/// wallet just means an empty watch list, not a startup failure — the tabs
/// then show "(loading…)"/zero totals rather than refusing to launch.
fn watched_addresses(profile: &Profile) -> Vec<String> {
    if profile.signer != SignerKind::Local {
        return Vec::new();
    }
    let Some(wallet_path) = profile.resolved_wallet_path() else {
        return Vec::new();
    };
    let passphrase = match secrets::passphrase(profile) {
        Ok(passphrase) => passphrase,
        Err(_) => return Vec::new(),
    };
    match Wallet::open(&wallet_path, &passphrase) {
        Ok(wallet) => wallet.addresses(),
        Err(err) => {
            eprintln!("lineage tui: failed to open wallet: {err}");
            Vec::new()
        }
    }
}

/// Handle a request (the `n` key) to generate a new address: for a
/// locally signed profile with a `wallet_path`, opens the keystore,
/// generates and persists a new address, reports it in the status line,
/// and requests a refresh so the new (zero-balance) address shows up in
/// the Wallet tab. Node-signer profiles (or profiles without a local
/// wallet) have no keystore to add an address to, so they just get a
/// status message explaining why.
fn handle_new_address_request(profile: &Profile, app: &mut App) {
    if profile.signer != SignerKind::Local {
        app.status = Some("new address requires a local wallet".to_string());
        return;
    }
    let Some(wallet_path) = profile.resolved_wallet_path() else {
        app.status = Some("new address requires a local wallet".to_string());
        return;
    };

    let passphrase = match secrets::passphrase(profile) {
        Ok(passphrase) => passphrase,
        Err(err) => {
            app.status = Some(format!("new address failed: {err}"));
            return;
        }
    };

    match Wallet::open(&wallet_path, &passphrase).and_then(|mut wallet| wallet.new_address()) {
        Ok(address) => {
            app.status = Some(format!("new address: {address}"));
            app.refresh_requested = true;
        }
        Err(err) => {
            app.status = Some(format!("new address failed: {err}"));
        }
    }
}

/// Resolve a passphrase, build the profile's signer, and submit the
/// reviewed payment inline, landing on `Done` with a receipt or back on
/// `Review` with `send_error` set. Mirrors `commands::pay::run`'s signer
/// construction.
async fn submit_send(client: &Client, profile: &Profile, app: &mut App) {
    let passphrase = match secrets::passphrase(profile) {
        Ok(passphrase) => passphrase,
        Err(_) if !app.send_passphrase.is_empty() => app.send_passphrase.clone(),
        Err(err) => {
            app.send_error = Some(err.to_string());
            app.send_step = SendStep::Review;
            return;
        }
    };

    let amount_lngx: f64 = app.send_amount.trim().parse().unwrap_or(0.0);
    let amount_raw = Tokens::from_lngx(amount_lngx).0;
    let to = app.send_to.clone();

    let receipt = match profile.signer {
        SignerKind::Local => match profile.resolved_wallet_path() {
            Some(wallet_path) => submit_local(client, &wallet_path, &passphrase, &to, amount_raw).await,
            None => {
                app.send_error = Some("profile has no wallet_path configured".to_string());
                app.send_step = SendStep::Review;
                return;
            }
        },
        SignerKind::Node => {
            let signer = NodeSigner::new(client, passphrase);
            signer.pay(&to, amount_raw).await
        }
    };

    match receipt {
        Ok(receipt) => {
            app.send_receipt = Some(receipt.tx_hash);
            app.send_error = None;
            app.send_step = SendStep::Done;
        }
        Err(err) => {
            app.send_error = Some(err.to_string());
            app.send_step = SendStep::Review;
        }
    }
}

/// The `SignerKind::Local` half of `submit_send`: open the keystore, pick
/// (or create) a change address, and pay.
async fn submit_local(
    client: &Client,
    wallet_path: &std::path::Path,
    passphrase: &str,
    to: &str,
    amount_raw: u64,
) -> lineage_sdk::Result<Receipt> {
    let mut wallet = Wallet::open(wallet_path, passphrase)?;
    let change_address = match wallet.addresses().into_iter().next() {
        Some(address) => address,
        None => wallet.new_address()?,
    };
    let signer = LocalSigner::new(client, &wallet, change_address);
    signer.pay(to, amount_raw).await
}
