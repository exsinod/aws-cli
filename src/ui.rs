use std::sync::mpsc::Receiver;

use ratatui::{
    prelude::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::Store;

fn calculate_scroll(lines: String, estate: Rect) -> u16 {
    let mut scroll_to: u16 = 0;

    let lines = lines.matches("\n").count();
    scroll_to = scroll_to + lines as u16;
    let height = estate.height - 4;
    if height > scroll_to {
        scroll_to = 0;
    } else {
        scroll_to = scroll_to - height;
    }
    scroll_to
}

pub fn ui(f: &mut Frame<'_>, store_rx: &Receiver<Store>) {
    if let Ok(store) = store_rx.try_recv() {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Max(1), Constraint::Percentage(90)])
            .split(f.size());
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_layout[1]);
        f.render_widget(
            Paragraph::new(if store.logged_in {
                Span::styled(
                    "LOGGED IN".to_string(),
                    Style::default().fg(Color::LightGreen),
                )
            } else if let Some(code) = store.login_code {
                Span::styled(code, Style::default().fg(Color::Yellow))
            } else {
                Span::styled(
                    "busy".to_string(),
                    Style::default().fg(Color::Red),
                )
            })
            .block(Block::new().borders(Borders::NONE))
            .alignment(Alignment::Right),
            main_layout[0],
        );
        if let Some(log) = store.logs {
            f.render_widget(
                Paragraph::new(log.to_string())
                    .scroll((calculate_scroll(log, layout[0]), 0))
                    .block(Block::new().title("Paragraph").borders(Borders::ALL))
                    .style(Style::new().fg(Color::White).bg(Color::Black))
                    .alignment(Alignment::Left)
                    .wrap(Wrap { trim: false }),
                layout[0],
            );
        }
        if let Some(log) = store.login_log {
            f.render_widget(
                Paragraph::new(log.to_string())
                    .scroll((calculate_scroll(log, layout[1]), 0))
                    .block(Block::new().title("Paragraph").borders(Borders::ALL))
                    .style(Style::new().bg(Color::White).fg(Color::Black))
                    .alignment(Alignment::Left)
                    .wrap(Wrap { trim: false }),
                layout[1],
            );
        }
    };
}
