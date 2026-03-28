//! Full-screen dashboard (Ratatui + crossterm).

use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval};

use crate::dashboard::{ControlState, DashboardStats, format_bytes};

pub async fn run_dashboard(stats: std::sync::Arc<DashboardStats>) -> Result<()> {
    let mut terminal = ratatui::try_init().context("terminal init")?;

    let (quit_tx, mut quit_rx) = mpsc::channel::<()>(4);
    let tx = quit_tx.clone();
    std::thread::spawn(move || {
        loop {
            if event::poll(Duration::from_millis(250)).unwrap_or(false)
                && let Ok(Event::Key(key)) = event::read()
                && key.kind == KeyEventKind::Press
            {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                        let _ = tx.blocking_send(());
                        break;
                    }
                    _ => {}
                }
            }
        }
    });

    let mut tick = interval(Duration::from_millis(100));
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut quit = false;
    while !quit {
        terminal
            .draw(|frame| render(frame, &stats))
            .context("terminal draw")?;

        tokio::select! {
            _ = tick.tick() => {}
            Some(()) = quit_rx.recv() => { quit = true; }
            r = tokio::signal::ctrl_c() => {
                r.context("ctrl_c")?;
                quit = true;
            }
        }
    }

    ratatui::restore();
    Ok(())
}

fn render(frame: &mut Frame, stats: &DashboardStats) {
    let area = frame.area();
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(" tunnelion ").cyan().bold());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(inner);

    let ctrl = stats.control_state();
    frame.render_widget(
        Paragraph::new(control_line(&stats.relay_display, ctrl)),
        chunks[0],
    );

    let header = Row::new(vec![
        Cell::from("Name"),
        Cell::from("Local addr"),
        Cell::from("Public domain"),
        Cell::from("Status"),
        Cell::from("In"),
        Cell::from("Out"),
    ])
    .style(Style::new().bold().fg(Color::Yellow))
    .bottom_margin(1);

    let widths = [
        Constraint::Percentage(14),
        Constraint::Percentage(18),
        Constraint::Percentage(26),
        Constraint::Percentage(14),
        Constraint::Percentage(14),
        Constraint::Percentage(14),
    ];

    let mut rows: Vec<Row> = Vec::new();
    for t in &stats.tunnels {
        let active = t.active_streams.load(std::sync::atomic::Ordering::Relaxed);
        let status = if ctrl != ControlState::Live {
            "…".to_string()
        } else if active > 0 {
            format!("Active ({active})")
        } else {
            "Idle".to_string()
        };
        let style = if active > 0 && ctrl == ControlState::Live {
            Style::new().fg(Color::Green)
        } else {
            Style::new()
        };
        rows.push(
            Row::new(vec![
                Cell::from(t.name.clone()),
                Cell::from(t.local_addr_display.clone()),
                Cell::from(t.domain.clone()),
                Cell::from(status),
                Cell::from(format_bytes(
                    t.bytes_in.load(std::sync::atomic::Ordering::Relaxed),
                )),
                Cell::from(format_bytes(
                    t.bytes_out.load(std::sync::atomic::Ordering::Relaxed),
                )),
            ])
            .style(style),
        );
    }

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::new().title(" tunnels ").borders(Borders::ALL));

    frame.render_widget(table, chunks[1]);

    // Light fg on dark grey: black-on-grey often disappears in terminals (looks like empty boxes).
    let key = Style::new()
        .fg(Color::Gray)
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD);
    let help = Line::from(vec![
        Span::raw("Quit: "),
        Span::styled("q", key),
        Span::raw(" · "),
        Span::styled("Esc", key),
        Span::raw(" · "),
        Span::styled("Ctrl+C", key),
    ]);
    frame.render_widget(Paragraph::new(help), chunks[2]);
}

fn control_line(relay: &str, s: ControlState) -> Line<'static> {
    let (label, color): (&str, Color) = match s {
        ControlState::Starting => ("starting", Color::DarkGray),
        ControlState::Connecting => ("connecting…", Color::Yellow),
        ControlState::Live => ("connected", Color::Green),
        ControlState::Reconnecting => ("reconnecting…", Color::Yellow),
        ControlState::Down => ("disconnected", Color::Red),
    };
    Line::from(vec![
        Span::styled("relay ".to_string(), Style::new().fg(Color::DarkGray)),
        Span::styled(relay.to_string(), Style::new().add_modifier(Modifier::BOLD)),
        Span::raw(" · "),
        Span::styled(label.to_string(), Style::new().fg(color)),
    ])
}
