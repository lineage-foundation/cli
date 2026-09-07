use crossterm::event::KeyCode;

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
}

impl App {
    pub fn new(profile_name: String) -> App {
        App {
            profile_name,
            active: Tab::Dashboard,
            should_quit: false,
            status: None,
            refresh_requested: false,
        }
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
}
