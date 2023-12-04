use std::{collections::HashMap, sync::mpsc::Sender};

use crossterm::event::KeyCode;
use log::trace;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{
    structs::{CliWidgetData, Store, TUIAction, TUIEvent},
    ui::MainLayoutUI,
};

#[derive(Debug, Default, Clone, Hash, Eq, PartialEq)]
pub enum CliWidgetId {
    #[default]
    Default,
    Header,
    GetLogs,
    GetLoginLogs,
    GetPods,
    RequestLogin,
    LoginRequest,
}

pub trait RenderWidget {
    fn render(&self, f: &mut Frame, layout: &MainLayoutUI);
    fn get_widget(&self) -> &CliWidget;
    fn get_widget_mut(&mut self) -> &mut CliWidget;

    fn get_title(&self) -> Option<String> {
        self.get_widget().title.clone()
    }

    fn set_title(&mut self, title: &str) {
        self.get_widget_mut().title = Some(title.to_string());
    }

    fn get_data(&self) -> CliWidgetData {
        self.get_widget().data.clone()
    }

    fn set_data(&mut self, key: String, text: Vec<String>) {
        self.get_widget_mut().data.data.insert(key, Some(text));
    }

    fn clear_text_data(&mut self, key: String) {
        self.get_widget_mut().data.data.insert(key, None);
    }
}

#[derive(Clone, Debug, Default)]
pub struct HeaderWidget {
    pub widget: CliWidget,
}

#[derive(Clone, Debug, Default)]
pub struct ErrorActionWidget {
    black: bool,
    pub widget: CliWidget,
}

#[derive(Clone, Debug, Default)]
pub struct BodyWidget {
    black: bool,
    full_screen: bool,
    pub widget: CliWidget,
}

#[derive(Debug, Default, Clone)]
pub struct CliWidget {
    pub id: CliWidgetId,
    pub title: Option<String>,
    pub data: CliWidgetData,
    pub pos: usize,
    pub logged_in: bool,
    is_selected: bool,
}

impl HeaderWidget {
    pub fn new(widget: CliWidget) -> Self {
        HeaderWidget { widget }
    }
}

impl ErrorActionWidget {
    pub fn new(black: bool, widget: CliWidget) -> Self {
        ErrorActionWidget { black, widget }
    }

    fn centered_rect(&self, r: Rect, percent_x: u16, percent_y: u16) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
}

impl BodyWidget {
    pub fn new(black: bool, full_screen: bool, widget: CliWidget) -> Self {
        BodyWidget {
            black,
            full_screen,
            widget,
        }
    }
}

impl<'a> HeaderWidget {
    fn kube_info(&self, text: Option<String>) -> Paragraph<'a> {
        let mut span = Span::default();
        if let Some(text) = text {
            if text == "Dev" {
                span = Span::styled(text, Style::default().fg(Color::Green));
            } else if text == "Prod" {
                span = Span::styled(text, Style::default().fg(Color::Red));
            }
            Paragraph::new(span)
                .block(Block::new().borders(Borders::NONE))
                .alignment(Alignment::Right)
        } else {
            Paragraph::new(Span::raw("")).block(Block::new().borders(Borders::NONE))
        }
    }
    fn header_error(&self, text: Option<String>) -> Paragraph<'a> {
        Paragraph::new(if let Some(error) = text {
            Span::styled(error, Style::default().fg(Color::Red))
        } else {
            Span::styled("All is good", Style::default().fg(Color::LightGreen))
        })
        .block(Block::new().borders(Borders::NONE))
        .alignment(Alignment::Right)
    }

    fn header_login_info(&self, is_logged_in: bool, text: Option<String>) -> Paragraph<'a> {
        Paragraph::new(if is_logged_in {
            Span::styled(
                "LOGGED IN".to_string(),
                Style::default().fg(Color::LightGreen),
            )
        } else if let Some(code) = text {
            Span::styled(code, Style::default().fg(Color::Yellow))
        } else {
            Span::styled("busy".to_string(), Style::default().fg(Color::Red))
        })
        .block(Block::new().borders(Borders::NONE))
        .alignment(Alignment::Right)
    }
}

impl<'a> RenderWidget for HeaderWidget {
    fn render(&self, f: &mut Frame, layout: &MainLayoutUI) {
        let rect = layout.get_header_rect(0, f);
        if let Some(error) = self.widget.data.data.get("error") {
            f.render_widget(
                self.header_error(error.as_ref().and_then(|e| Some(e.join("\n")))),
                rect[0],
            );
        }
        if let Some(login_info) = self.widget.data.data.get("login_info") {
            if let Some(Some(logged_in)) = self.get_data().data.get("logged in") {
                f.render_widget(
                    self.header_login_info(
                        logged_in[0].eq(true.to_string().as_str()),
                        login_info.as_ref().and_then(|e| Some(e.join("\n"))),
                    ),
                    rect[1],
                );
            }
        }
        let rect = layout.get_header_rect(1, f);
        if let Some(kube_info) = self.widget.data.data.get("kube_info") {
            f.render_widget(
                self.kube_info(kube_info.as_ref().and_then(|e| Some(e.join("\n")))),
                rect[0],
            );
        }
    }

    fn get_widget(&self) -> &CliWidget {
        &self.widget
    }

    fn get_widget_mut(&mut self) -> &mut CliWidget {
        &mut self.widget
    }
}

impl<'a> RenderWidget for ErrorActionWidget {
    fn render(&self, f: &mut Frame, layout: &MainLayoutUI) {
        if let Some(title) = self.widget.title.clone() {
            if let Some(Some(logs)) = self.get_data().data.get("logs") {
                let rect = layout.get_full_rect(f);
                f.render_widget(
                    self.widget
                        .content_in_black(title, logs, rect[0])
                        .unwrap_or_default(),
                    self.centered_rect(rect[0], 50, 30),
                );
            }
        }
    }

    fn get_widget(&self) -> &CliWidget {
        &self.widget
    }

    fn get_widget_mut(&mut self) -> &mut CliWidget {
        &mut self.widget
    }
}

impl<'a> RenderWidget for BodyWidget {
    fn render(&self, f: &mut Frame, layout: &MainLayoutUI) {
        trace!("rendering widget with data {:?}", self.widget.data.clone());
        match self.widget.title.clone() {
            Some(title) => {
                if let Some(Some(logs)) = self.get_data().data.get("logs") {
                    if self.full_screen {
                        let rect = layout.get_full_rect(f);
                        if self.black {
                            f.render_widget(
                                self.widget
                                    .content_in_black(
                                        title.to_string(),
                                        logs,
                                        rect[self.widget.pos],
                                    )
                                    .unwrap_or_default(),
                                rect[self.widget.pos],
                            );
                        } else {
                            f.render_widget(
                                self.widget
                                    .content_in_white(
                                        title.to_string(),
                                        logs,
                                        rect[self.widget.pos],
                                    )
                                    .unwrap_or_default(),
                                rect[self.widget.pos],
                            );
                        }
                    } else {
                        let rect = layout.get_body_rect(f);
                        if self.black {
                            f.render_widget(
                                self.widget
                                    .content_in_black(
                                        title.to_string(),
                                        logs,
                                        rect[self.widget.pos],
                                    )
                                    .unwrap_or_default(),
                                rect[self.widget.pos],
                            );
                        } else {
                            f.render_widget(
                                self.widget
                                    .content_in_white(
                                        title.to_string(),
                                        logs,
                                        rect[self.widget.pos],
                                    )
                                    .unwrap_or_default(),
                                rect[self.widget.pos],
                            );
                        }
                    }
                }
            }
            None => {}
        }
    }

    fn get_widget(&self) -> &CliWidget {
        &self.widget
    }

    fn get_widget_mut(&mut self) -> &mut CliWidget {
        &mut self.widget
    }
}

impl<'a> CliWidget {
    pub fn bordered(id: CliWidgetId, title: &str, pos: usize, data: CliWidgetData) -> Self {
        CliWidget {
            id,
            title: Some(title.to_string()),
            data,
            pos,
            logged_in: false,
            is_selected: false,
        }
    }

    pub fn unbordered(id: CliWidgetId, data: CliWidgetData) -> Self {
        CliWidget {
            id,
            title: None,
            data,
            pos: 0,
            logged_in: false,
            is_selected: false,
        }
    }

    fn content_in_black(
        &self,
        title: String,
        logs: &Vec<String>,
        rect: Rect,
    ) -> Option<Paragraph<'a>> {
        let bg_color = Color::Black;
        let fg_color = Color::White;
        let border_color = match self.is_selected {
            true => Color::Red,
            false => fg_color,
        };
        Some(
            Paragraph::new(logs.join(""))
                .scroll((Self::calculate_scroll(&logs, &rect), 50))
                .block(
                    Block::new()
                        .title(title)
                        .borders(Borders::ALL)
                        .style(Style::new().fg(border_color)),
                )
                .style(Style::new().fg(fg_color).bg(bg_color))
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: false }),
        )
    }

    fn content_in_white(
        &self,
        title: String,
        logs: &Vec<String>,
        rect: Rect,
    ) -> Option<Paragraph<'a>> {
        Some(
            Paragraph::new(logs.join("\n"))
                .scroll((Self::calculate_scroll(&logs, &rect), 0))
                .block(Block::new().title(title).borders(Borders::ALL))
                .style(Style::new().bg(Color::White).fg(Color::Black))
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: true }),
        )
    }

    fn calculate_scroll(lines: &Vec<String>, estate: &Rect) -> u16 {
        let mut scroll_to: u16 = 0;
        for line in lines {
            let new_lines = line.chars().filter(|c| c.eq(&'\n')).count();
            let estate_space = line.len() as u16 / estate.width;
            if new_lines as u16 > estate_space {
                scroll_to += new_lines as u16 + 1;
            } else {
                scroll_to += estate_space + 1;
            }
        }
        let height = estate.height - 4;
        if height > scroll_to {
            scroll_to = 0;
        } else {
            scroll_to = scroll_to - height;
        }
        scroll_to
    }
}

fn add_to_widget_data<'a>(widget: &mut BodyWidget, text: String) -> &mut BodyWidget {
    if let Some(Some(existing_text)) = &mut widget.get_data().data.get_mut("logs") {
        existing_text.push(text);
        widget.set_data("logs".to_string(), existing_text.to_vec());
    } else {
        widget.set_data("logs".to_string(), vec![text]);
    }
    widget
}

pub fn create_header_widget_data<'a>() -> WidgetDescription<HeaderWidget> {
    let header_data = CliWidgetData::new(CliWidgetId::Header);
    let header_widget = HeaderWidget::new(CliWidget::unbordered(CliWidgetId::Header, header_data));
    WidgetDescription {
        widget: header_widget,
        event_handler: |_, _| None,
        keymap: |_, _, _| {},
    }
}

pub fn create_login_widget_data<'a>() -> WidgetDescription<BodyWidget> {
    let login_widget_data = CliWidgetData {
        id: CliWidgetId::GetLoginLogs,
        thread_started: false,
        initiate_thread: None,
        data: HashMap::default(),
    };
    let login_widget = BodyWidget::new(
        false,
        true,
        CliWidget::bordered(
            CliWidgetId::GetLoginLogs,
            "Logging in...",
            0,
            login_widget_data,
        ),
    );
    let login_event_handler = |event: &TUIEvent, store: &mut Store| match event {
        TUIEvent::AddLoginLog(log_part) => {
            add_to_widget_data(store.login_widget.as_mut().unwrap(), log_part.to_string());
            None
        }
        _ => Some(()),
    };
    WidgetDescription {
        widget: login_widget,
        event_handler: login_event_handler,
        keymap: |_, _, _| {},
    }
}

pub fn create_logs_widget_data<'a>() -> WidgetDescription<BodyWidget> {
    let logs_widget_data = CliWidgetData {
        id: CliWidgetId::GetLogs,
        thread_started: false,
        initiate_thread: Some(|a| {
            a.send(TUIAction::GetLogs).unwrap();
        }),
        data: HashMap::default(),
    };
    let logs_widget = BodyWidget::new(
        true,
        false,
        CliWidget::bordered(CliWidgetId::GetLogs, "Salespoint Logs", 0, logs_widget_data),
    );
    let logs_event_handler = |event: &TUIEvent, store: &mut Store| match event {
        TUIEvent::AddLog(log_part) => {
            add_to_widget_data(store.logs_widget.as_mut().unwrap(), log_part.to_string());
            None
        }
        _ => Some(()),
    };
    WidgetDescription {
        widget: logs_widget,
        event_handler: logs_event_handler,
        keymap: |_, _, _| {},
    }
}

pub fn create_pods_widget_data<'a>() -> WidgetDescription<BodyWidget> {
    let pods_widget_data = CliWidgetData {
        id: CliWidgetId::GetPods,
        thread_started: false,
        initiate_thread: Some(|a| {
            a.send(TUIAction::GetPods).unwrap();
        }),
        data: HashMap::default(),
    };
    let pods_widget = BodyWidget::new(
        true,
        false,
        CliWidget::bordered(CliWidgetId::GetPods, "Salespoint pods", 1, pods_widget_data),
    );
    let pods_event_handler = |event: &TUIEvent, store: &mut Store| match event {
        TUIEvent::AddPods(pods) => {
            add_to_widget_data(store.pods_widget.as_mut().unwrap(), pods.to_string());
            None
        }
        _ => Some(()),
    };
    WidgetDescription {
        widget: pods_widget,
        event_handler: pods_event_handler,
        keymap: |_, _, _| {},
    }
}

pub fn create_request_login_widget_data<'a>() -> WidgetDescription<ErrorActionWidget> {
    let login_request_widget_data = CliWidgetData {
        id: CliWidgetId::RequestLogin,
        thread_started: false,
        initiate_thread: Some(|_| {}),
        data: HashMap::default(),
    };
    let login_request_widget = ErrorActionWidget::new(
        true,
        CliWidget::bordered(CliWidgetId::LoginRequest, "", 1, login_request_widget_data),
    );
    let login_request_event_handler = |event: &TUIEvent, store: &mut Store| match event {
        TUIEvent::RequestLoginStart => {
            store.request_login = true;
            store
                .request_login_widget
                .as_mut()
                .unwrap()
                .set_title("It seems I can't reach your resources...");
            store.request_login_widget.as_mut().unwrap().set_data(
                "logs".to_string(),
                vec![
                    "\nWhat do you want to do?\n\n".to_string(),
                    "1. retry (I forgot to turn on my VPN)\n".to_string(),
                    "2. Login to AWS".to_string(),
                ],
            );
            None
        }
        TUIEvent::RequestLoginStop => {
            store.request_login = false;
            None
        }
        _ => Some(()),
    };
    WidgetDescription {
        widget: login_request_widget,
        event_handler: login_request_event_handler,
        keymap: |keycode: KeyCode, store: &Store, event_tx: &Sender<TUIEvent>| {
            if store.request_login {
                match keycode {
                    KeyCode::Char('1') => {
                        event_tx.send(TUIEvent::RequestLoginStop).unwrap();
                        event_tx.send(TUIEvent::ClearError).unwrap();
                        event_tx.send(TUIEvent::CheckConnectivity).unwrap();
                    }
                    KeyCode::Char('2') => {
                        event_tx.send(TUIEvent::RequestLoginStop).unwrap();
                        event_tx.send(TUIEvent::NeedsLogin).unwrap()
                    }
                    _ => {}
                }
            }
        },
    }
}

#[derive(Clone)]
pub struct WidgetDescription<T: RenderWidget + Clone> {
    widget: T,
    event_handler: fn(&TUIEvent, &mut Store) -> Option<()>,
    keymap: fn(KeyCode, &Store, &Sender<TUIEvent>),
}

impl<T: RenderWidget + Clone> WidgetDescription<T> {
    pub fn get_widget(&self) -> T {
        self.widget.clone()
    }

    pub fn get_event_handler(&self) -> fn(&TUIEvent, &mut Store) -> Option<()> {
        self.event_handler
    }

    pub fn get_keymap(&self) -> fn(KeyCode, &Store, &Sender<TUIEvent>) {
        self.keymap
    }
}
