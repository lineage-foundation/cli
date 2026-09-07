use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};
use ratatui::Frame;

use crate::tui::app::{App, SendFocus, SendStep, Tab};

/// Render the whole TUI: a top tab bar, the active tab's view, and a bottom
/// status/help line.
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    draw_tabs(frame, app, chunks[0]);

    match app.active {
        Tab::Dashboard => draw_dashboard(frame, app, chunks[1]),
        Tab::Wallet => draw_wallet(frame, app, chunks[1]),
        Tab::Send => draw_send(frame, app, chunks[1]),
    }

    draw_status(frame, app, chunks[2]);
}

fn draw_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let titles = vec!["1: Dashboard", "2: Wallet", "3: Send"];
    let selected = match app.active {
        Tab::Dashboard => 0,
        Tab::Wallet => 1,
        Tab::Send => 2,
    };
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("lineage tui"))
        .select(selected)
        .highlight_style(Style::default().fg(Color::Yellow));
    frame.render_widget(tabs, area);
}

fn draw_dashboard(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Testnet", Style::default().fg(Color::Green)),
        Span::raw(format!("  profile: {}", app.profile_name)),
    ]));

    match app.head {
        Some(head) => lines.push(Line::from(format!("Head: {head}"))),
        None => lines.push(Line::from("Head: (loading…)")),
    }

    match &app.supply {
        Some(supply) => {
            let issued_lngx = supply.issued as f64 / lineage_sdk::RAW_PER_LNGX as f64;
            let total_lngx = supply.total as f64 / lineage_sdk::RAW_PER_LNGX as f64;
            let pct = app
                .supply_pct()
                .map(|p| format!("{p:.2}%"))
                .unwrap_or_else(|| "n/a".to_string());
            lines.push(Line::from(format!(
                "Supply: {issued_lngx:.2} / {total_lngx:.2} LNGX ({pct})"
            )));
        }
        None => lines.push(Line::from("Supply: (loading…)")),
    }

    match app.wallet_total_raw {
        Some(raw) => {
            let lngx = raw as f64 / lineage_sdk::RAW_PER_LNGX as f64;
            lines.push(Line::from(format!("Wallet total: {lngx:.2} LNGX")));
        }
        None => lines.push(Line::from("Wallet total: (loading…)")),
    }

    lines.push(Line::from(format!(
        "Last updated: {}",
        app.last_updated.as_deref().unwrap_or("never")
    )));

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Dashboard"),
    );
    frame.render_widget(paragraph, area);
}

fn draw_wallet(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = vec![Line::from("Wallet")];

    if app.wallet_addresses.is_empty() {
        lines.push(Line::from(
            "no addresses (press n / configure wallet_path)",
        ));
    } else {
        for (address, raw) in &app.wallet_addresses {
            let lngx = *raw as f64 / lineage_sdk::RAW_PER_LNGX as f64;
            lines.push(Line::from(format!("{address}  {lngx:.2} LNGX")));
        }
    }

    lines.push(Line::from(match app.wallet_total_raw {
        Some(raw) => {
            let lngx = raw as f64 / lineage_sdk::RAW_PER_LNGX as f64;
            format!("Total: {lngx:.2} LNGX")
        }
        None => "Total: (loading…)".to_string(),
    }));

    lines.push(Line::from("n: new address  r: refresh"));

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Wallet"),
    );
    frame.render_widget(paragraph, area);
}

fn draw_send(frame: &mut Frame, app: &App, area: Rect) {
    let lines: Vec<Line> = match app.send_step {
        SendStep::Form => {
            let to_marker = if app.send_focus == SendFocus::To { "> " } else { "  " };
            let amount_marker = if app.send_focus == SendFocus::Amount {
                "> "
            } else {
                "  "
            };
            let mut lines = vec![
                Line::from(format!("{to_marker}To: {}", app.send_to)),
                Line::from(format!("{amount_marker}Amount (LNGX): {}", app.send_amount)),
            ];
            if let Some(err) = &app.send_error {
                lines.push(Line::from(Span::styled(
                    format!("Error: {err}"),
                    Style::default().fg(Color::Red),
                )));
            }
            lines
        }
        SendStep::Review => {
            let amount_lngx: f64 = app.send_amount.trim().parse().unwrap_or(0.0);
            let raw = lineage_sdk::Tokens::from_lngx(amount_lngx).0;
            let mut lines = vec![
                Line::from(format!("To: {}", app.send_to)),
                Line::from(format!("Amount: {amount_lngx} LNGX ({raw} raw)")),
                Line::from("Press enter to confirm, esc to go back."),
            ];
            if let Some(err) = &app.send_error {
                lines.push(Line::from(Span::styled(
                    format!("Error: {err}"),
                    Style::default().fg(Color::Red),
                )));
            }
            lines
        }
        SendStep::Confirm => {
            let masked: String = "*".repeat(app.send_passphrase.len());
            vec![
                Line::from("Enter passphrase to confirm:"),
                Line::from(masked),
            ]
        }
        SendStep::Submitting => vec![Line::from("Submitting…")],
        SendStep::Done => {
            let mut lines = vec![Line::from("Done.")];
            if let Some(receipt) = &app.send_receipt {
                lines.push(Line::from(format!("Receipt: {receipt}")));
            }
            if let Some(err) = &app.send_error {
                lines.push(Line::from(Span::styled(
                    format!("Error: {err}"),
                    Style::default().fg(Color::Red),
                )));
            }
            lines
        }
    };

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Send"),
    );
    frame.render_widget(paragraph, area);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let help = "q: quit  Tab/1-3: switch  r: refresh";
    let mut text = help.to_string();
    if let Some(notice) = &app.notice {
        text.push_str("  |  ");
        text.push_str(notice);
    }
    if let Some(status) = &app.status {
        text.push_str("  |  ");
        text.push_str(status);
    }
    let paragraph = Paragraph::new(Line::from(text));
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::Terminal;

    fn buffer_to_string(buffer: &Buffer) -> String {
        let mut out = String::new();
        for cell in buffer.content.iter() {
            out.push_str(cell.symbol());
        }
        out
    }

    #[test]
    fn dashboard_shows_head_and_testnet() {
        let mut app = App::new("testnet".into());
        app.apply_head(7141);
        app.apply_supply(lineage_sdk::models::Supply {
            total: 360_360_000_000_000_000,
            issued: 90_000_000_000_000_000,
        });
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|f| draw(f, &app)).expect("draw");
        let text = buffer_to_string(terminal.backend().buffer());
        assert!(text.contains("7141"));
        assert!(text.contains("Testnet"));
    }

    #[test]
    fn wallet_tab_shows_seeded_address_and_lngx_balance() {
        let mut app = App::new("testnet".into());
        app.active = Tab::Wallet;
        app.wallet_addresses = vec![("addr-a".to_string(), 720_720_000)];
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|f| draw(f, &app)).expect("draw");
        let text = buffer_to_string(terminal.backend().buffer());
        assert!(text.contains("addr-a"));
        assert!(text.contains("10.00"));
    }

    #[test]
    fn send_review_shows_lngx_and_raw_amount() {
        let mut app = App::new("testnet".into());
        app.active = Tab::Send;
        app.send_step = SendStep::Review;
        app.send_amount = "10".into();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal.draw(|f| draw(f, &app)).expect("draw");
        let text = buffer_to_string(terminal.backend().buffer());
        assert!(text.contains("720720000"));
        assert!(text.contains("10"));
    }
}
