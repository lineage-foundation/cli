use crossterm::event::KeyCode;
use lineage_sdk::models::Supply;

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

pub struct App {
    pub profile_name: String,
    pub active: Tab,
    pub should_quit: bool,
    pub status: Option<String>,
    pub refresh_requested: bool,
    pub head: Option<u64>,
    pub supply: Option<Supply>,
    pub wallet_total_raw: Option<u64>,
    pub last_updated: Option<String>,
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
            last_updated: None,
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
            _ => {}
        }
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
}
