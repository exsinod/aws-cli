use std::sync::mpsc::Sender;

use ratatui::{
    prelude::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{Store, TUIAction};

pub fn ui(f: &mut Frame<'_>, store: Store, action_tx: Sender<TUIAction>) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Max(1), Constraint::Percentage(90)])
        .split(f.size());
    let header_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_layout[0]);

    f.render_widget(header_error(store.clone()), header_layout[0]);
    f.render_widget(header_login_info(store.clone()), header_layout[1]);

    if let true = store.logged_in {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_layout[1]);
        render_widget_and_call(f, store.clone(), store.clone().logs, action_tx.clone(), get_logs_action, layout[0]);
        render_widget_and_call(f, store.clone(), store.pods, action_tx.clone(), get_pods_action, layout[1]);
    } else {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_layout[1]);
        f.render_widget(
            content_in_white(
                "Login script".to_string(),
                store.clone().login_log,
                layout[1],
            )
            .unwrap_or_default(),
            layout[1],
        );
    }
}

fn render_widget_and_call(
    f: &mut Frame,
    store: Store,
    data: Option<String>,
    action_tx: Sender<TUIAction>,
    do_action: fn(store: Store, action_thread: Sender<TUIAction>),
    rect: Rect,
) {
        do_action(store.clone(), action_tx);
        f.render_widget(
            content_in_black(
                "Salespoint V2 logs".to_string(),
                data,
                rect,
            )
            .unwrap_or_default(),
            rect,
        );
}

fn get_logs_action(store: Store, action_tx: Sender<TUIAction>) {
    if let false = store.log_thread_started {
        action_tx.send(TUIAction::GetLogs).unwrap();
    }
}

fn get_pods_action(store: Store, action_tx: Sender<TUIAction>) {
        action_tx.send(TUIAction::GetPods).unwrap();
}

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

fn header_error<'a>(store: Store) -> Paragraph<'a> {
    Paragraph::new(if let Some(error) = store.error {
        Span::styled(error, Style::default().fg(Color::Red))
    } else {
        Span::styled("All is good", Style::default().fg(Color::LightGreen))
    })
    .block(Block::new().borders(Borders::NONE))
    .alignment(Alignment::Right)
}
fn header_login_info<'a>(store: Store) -> Paragraph<'a> {
    Paragraph::new(if store.logged_in {
        Span::styled(
            "LOGGED IN".to_string(),
            Style::default().fg(Color::LightGreen),
        )
    } else if let Some(code) = store.login_code {
        Span::styled(code, Style::default().fg(Color::Yellow))
    } else {
        Span::styled("busy".to_string(), Style::default().fg(Color::Red))
    })
    .block(Block::new().borders(Borders::NONE))
    .alignment(Alignment::Right)
}

fn content_in_black<'a>(title: String, logs: Option<String>, rect: Rect) -> Option<Paragraph<'a>> {
    if let Some(log) = logs {
        Some(
            Paragraph::new(log.to_string())
                .scroll((calculate_scroll(log, rect), 0))
                .block(Block::new().title(title).borders(Borders::ALL))
                .style(Style::new().fg(Color::White).bg(Color::Black))
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: false }),
        )
    } else {
        None
    }
}

fn content_in_white<'a>(title: String, logs: Option<String>, rect: Rect) -> Option<Paragraph<'a>> {
    if let Some(log) = logs {
        Some(
            Paragraph::new(log.to_string())
                .scroll((calculate_scroll(log, rect), 0))
                .block(Block::new().title(title).borders(Borders::ALL))
                .style(Style::new().bg(Color::White).fg(Color::Black))
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: false }),
        )
    } else {
        None
    }
}
