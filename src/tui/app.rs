use crossterm::event::KeyCode;
use lineage_sdk::models::{BalancesResponse, Supply};

use crate::config::Profile;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Tab {
    Dashboard,
    Wallet,
    Send,
}

impl Tab {
    fn next(self) -> Tab {
        match self {
            Tab::Dashboard => Tab::Wallet,
            Tab::Wallet => Tab::Send,
            Tab::Send => Tab::Dashboard,
        }
    }
}

/// Steps of the send flow: fill in the form, review the guardrail-checked
/// amount, confirm with a passphrase, submit, and show the receipt.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SendStep {
    Form,
    Review,
    Confirm,
    Submitting,
    Done,
}

/// Which send-form field currently receives typed input.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SendFocus {
    To,
    Amount,
}

pub struct App {
    pub profile_name: String,
    pub active: Tab,
    pub should_quit: bool,
    pub status: Option<String>,
    pub refresh_requested: bool,
    pub head: Option<u64>,
    pub supply: Option<Supply>,
    pub wallet_total_raw: Option<u64>,
    pub wallet_addresses: Vec<(String, u64)>,
    pub new_address_requested: bool,
    pub last_updated: Option<String>,
    pub send_to: String,
    pub send_amount: String,
    pub send_step: SendStep,
    pub send_focus: SendFocus,
    pub send_passphrase: String,
    pub send_error: Option<String>,
    pub send_receipt: Option<String>,
}

impl App {
    pub fn new(profile_name: String) -> App {
        App {
            profile_name,
            active: Tab::Dashboard,
            should_quit: false,
            status: None,
            refresh_requested: false,
            head: None,
            supply: None,
            wallet_total_raw: None,
            wallet_addresses: Vec::new(),
            new_address_requested: false,
            last_updated: None,
            send_to: String::new(),
            send_amount: String::new(),
            send_step: SendStep::Form,
            send_focus: SendFocus::To,
            send_passphrase: String::new(),
            send_error: None,
            send_receipt: None,
        }
    }

    pub fn apply_head(&mut self, head: u64) {
        self.head = Some(head);
    }

    pub fn apply_supply(&mut self, supply: Supply) {
        self.supply = Some(supply);
    }

    pub fn apply_wallet_total(&mut self, total_raw: u64) {
        self.wallet_total_raw = Some(total_raw);
    }

    /// Fill `wallet_addresses` from a fetched `BalancesResponse`: each
    /// address's raw token balance is the sum of its `Token` UTXOs
    /// (non-token UTXO values, if any, are ignored). Also sets
    /// `wallet_total_raw` from the response's total, so callers no longer
    /// need a separate `apply_wallet_total` call.
    pub fn apply_wallet_balances(&mut self, balances: &BalancesResponse) {
        self.wallet_addresses = balances
            .balance
            .address_list
            .iter()
            .map(|(address, utxos)| {
                let raw: u64 = utxos
                    .iter()
                    .filter_map(|utxo| utxo.value.get("Token").and_then(|v| v.as_u64()))
                    .sum();
                (address.clone(), raw)
            })
            .collect();
        self.wallet_total_raw = Some(balances.balance.total.tokens);
    }

    pub fn set_updated(&mut self, stamp: String) {
        self.last_updated = Some(stamp);
    }

    pub fn supply_pct(&self) -> Option<f64> {
        let supply = self.supply.as_ref()?;
        if supply.total == 0 {
            return None;
        }
        Some((supply.issued as f64 / supply.total as f64) * 100.0)
    }

    pub fn on_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Tab => self.active = self.active.next(),
            KeyCode::Char('1') => self.active = Tab::Dashboard,
            KeyCode::Char('2') => self.active = Tab::Wallet,
            KeyCode::Char('3') => self.active = Tab::Send,
            KeyCode::Char('r') => self.refresh_requested = true,
            KeyCode::Char('n') => self.new_address_requested = true,
            _ => {}
        }
    }

    /// Append a typed character to whichever send-form field has focus.
    pub fn send_input_char(&mut self, c: char) {
        match self.send_focus {
            SendFocus::To => self.send_to.push(c),
            SendFocus::Amount => self.send_amount.push(c),
        }
    }

    /// Remove the last character from the focused send-form field.
    pub fn send_backspace(&mut self) {
        match self.send_focus {
            SendFocus::To => {
                self.send_to.pop();
            }
            SendFocus::Amount => {
                self.send_amount.pop();
            }
        }
    }

    /// Toggle input focus between the recipient and amount fields.
    pub fn send_field_next(&mut self) {
        self.send_focus = match self.send_focus {
            SendFocus::To => SendFocus::Amount,
            SendFocus::Amount => SendFocus::To,
        };
    }

    /// Advance the send flow from `Form` to `Review`: validates the
    /// recipient and amount, then runs them past the profile's guardrails
    /// (the TUI's review + confirm step stands in for `--yes`). On success
    /// moves to `Review` and returns the parsed LNGX amount; on any
    /// failure sets `send_error`, stays in `Form`, and returns `None`.
    pub fn send_advance(&mut self, profile: &Profile) -> Option<f64> {
        if self.send_step != SendStep::Form {
            return None;
        }

        if self.send_to.trim().is_empty() {
            self.send_error = Some("recipient address is required".to_string());
            return None;
        }

        let amount = match self.send_amount.trim().parse::<f64>() {
            Ok(value) if value > 0.0 => value,
            _ => {
                self.send_error = Some("amount must be a positive number".to_string());
                return None;
            }
        };

        match crate::guard::check(profile, &self.send_to, amount, true) {
            Ok(()) => {
                self.send_error = None;
                self.send_step = SendStep::Review;
                Some(amount)
            }
            Err(denied) => {
                self.send_error = Some(denied.reason);
                None
            }
        }
    }

    /// Step the send flow back toward `Form`, clearing any error.
    pub fn send_back(&mut self) {
        self.send_step = match self.send_step {
            SendStep::Review => SendStep::Form,
            SendStep::Confirm => SendStep::Review,
            other => other,
        };
        self.send_error = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_cycles_and_number_keys_select() {
        let mut app = App::new("testnet".into());
        assert_eq!(app.active, Tab::Dashboard);
        app.on_key(KeyCode::Tab);
        assert_eq!(app.active, Tab::Wallet);
        app.on_key(KeyCode::Char('3'));
        assert_eq!(app.active, Tab::Send);
        app.on_key(KeyCode::Char('1'));
        assert_eq!(app.active, Tab::Dashboard);
    }

    #[test]
    fn q_sets_quit() {
        let mut app = App::new("testnet".into());
        app.on_key(KeyCode::Char('q'));
        assert!(app.should_quit);
    }

    #[test]
    fn apply_updates_cached_fields_and_pct() {
        let mut app = App::new("testnet".into());
        app.apply_head(7141);
        app.apply_supply(Supply {
            total: 200,
            issued: 50,
        });
        app.apply_wallet_total(720_720_000);
        assert_eq!(app.head, Some(7141));
        assert_eq!(app.wallet_total_raw, Some(720_720_000));
        assert_eq!(app.supply_pct(), Some(25.0));
    }

    // `Profile::testnet()` has no `max_amount`/`daily_cap` set, so
    // `guard::check` never touches the on-disk daily tally here.

    #[test]
    fn send_advance_moves_to_review_when_guard_allows() {
        let mut app = App::new("testnet".into());
        app.send_to = "addr-a".into();
        app.send_amount = "10".into();
        let profile = Profile::testnet();

        let amount = app.send_advance(&profile);

        assert_eq!(amount, Some(10.0));
        assert_eq!(app.send_step, SendStep::Review);
        assert_eq!(app.send_error, None);
    }

    #[test]
    fn send_advance_denied_by_guard_stays_in_form_with_reason() {
        let mut app = App::new("testnet".into());
        app.send_to = "addr-a".into();
        app.send_amount = "20".into();
        let mut profile = Profile::testnet();
        profile.max_amount = Some(5.0);

        let amount = app.send_advance(&profile);

        assert_eq!(amount, None);
        assert_eq!(app.send_step, SendStep::Form);
        assert!(app.send_error.is_some());
    }

    #[test]
    fn send_advance_requires_a_recipient() {
        let mut app = App::new("testnet".into());
        app.send_to = "".into();
        app.send_amount = "10".into();
        let profile = Profile::testnet();

        let amount = app.send_advance(&profile);

        assert_eq!(amount, None);
        assert_eq!(app.send_step, SendStep::Form);
        assert!(app.send_error.is_some());
    }

    #[test]
    fn send_advance_requires_a_positive_numeric_amount() {
        let mut app = App::new("testnet".into());
        app.send_to = "addr-a".into();
        app.send_amount = "not-a-number".into();
        let profile = Profile::testnet();

        let amount = app.send_advance(&profile);

        assert_eq!(amount, None);
        assert_eq!(app.send_step, SendStep::Form);
        assert!(app.send_error.is_some());
    }

    #[test]
    fn send_field_next_toggles_focus_and_input_targets_it() {
        let mut app = App::new("testnet".into());
        assert_eq!(app.send_focus, SendFocus::To);

        app.send_input_char('a');
        app.send_field_next();
        assert_eq!(app.send_focus, SendFocus::Amount);
        app.send_input_char('5');
        app.send_backspace();

        assert_eq!(app.send_to, "a");
        assert_eq!(app.send_amount, "");
    }

    #[test]
    fn apply_wallet_balances_sums_token_utxos_per_address() {
        use lineage_sdk::models::{BalanceTotals, Balances, BalancesResponse, OutPointRef, Utxo};
        use std::collections::BTreeMap;

        let mut address_list = BTreeMap::new();
        address_list.insert(
            "addr-a".to_string(),
            vec![Utxo {
                out_point: OutPointRef {
                    n: 0,
                    t_hash: "hash-a".to_string(),
                },
                value: serde_json::json!({"Token": 720_720_000u64}),
            }],
        );
        let balances = BalancesResponse {
            balance: Balances {
                address_list,
                total: BalanceTotals {
                    tokens: 720_720_000,
                    items: serde_json::Value::Null,
                },
            },
        };

        let mut app = App::new("testnet".into());
        app.apply_wallet_balances(&balances);

        assert_eq!(
            app.wallet_addresses,
            vec![("addr-a".to_string(), 720_720_000u64)]
        );
        assert_eq!(app.wallet_total_raw, Some(720_720_000));
    }

    #[test]
    fn n_key_requests_new_address() {
        let mut app = App::new("testnet".into());
        assert!(!app.new_address_requested);
        app.on_key(KeyCode::Char('n'));
        assert!(app.new_address_requested);
    }

    #[test]
    fn send_back_steps_toward_form_and_clears_error() {
        let mut app = App::new("testnet".into());
        app.send_step = SendStep::Review;
        app.send_error = Some("stale".into());

        app.send_back();

        assert_eq!(app.send_step, SendStep::Form);
        assert_eq!(app.send_error, None);
    }
}
