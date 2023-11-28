use std::collections::HashMap;

use crossterm::event::KeyCode;
use log::{debug, trace};
use ratatui::{
    layout::{Alignment, Rect},
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
    Tail,
}

pub trait RenderWidget {
    fn render(&mut self, f: &mut Frame, layout: MainLayoutUI);
    fn get_widget(&mut self) -> &mut CliWidget;

    fn get_data(&mut self) -> CliWidgetData {
        self.get_widget().data.clone()
    }

    fn set_thread_started(&mut self, started: bool) {
        self.get_widget().data.thread_started = started
    }

    fn set_data(&mut self, key: String, text: Vec<String>) -> CliWidgetData {
        self.get_widget().data.data.insert(key, Some(text));
        self.get_widget().data.clone()
    }

    fn clear_text_data(&mut self, key: String) -> CliWidgetData {
        self.get_widget().data.clone().data.insert(key, None);
        self.get_widget().data.clone()
    }
}

#[derive(Clone, Debug, Default)]
pub struct HeaderWidget {
    pub widget: CliWidget,
}

#[derive(Clone, Debug, Default)]
pub struct BodyWidget {
    black: bool,
    pub widget: CliWidget,
}

#[derive(Debug, Default, Clone)]
pub struct CliWidget {
    pub id: CliWidgetId,
    pub title: Option<String>,
    pub data: CliWidgetData,
    pub pos: usize,
    is_selected: bool,
}

impl HeaderWidget {
    pub fn new(widget: CliWidget) -> Self {
        HeaderWidget { widget }
    }
}

impl BodyWidget {
    pub fn new(black: bool, widget: CliWidget) -> Self {
        BodyWidget { black, widget }
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
    fn render(&mut self, f: &mut Frame, layout: MainLayoutUI) {
        let rect = layout.get_header_rect(0, f);
        f.render_widget(
            self.header_error(
                self.widget
                    .data
                    .clone()
                    .data
                    .get("error")
                    .and_then(|d| Some(d.clone().unwrap().clone().join("\n"))),
            ),
            rect[0],
        );
        f.render_widget(
            self.header_login_info(
                false,
                self.widget
                    .data
                    .clone()
                    .data
                    .get("login_info")
                    .and_then(|d| Some(d.clone().unwrap().clone().join("\n"))),
            ),
            rect[1],
        );
        let rect = layout.get_header_rect(1, f);
        f.render_widget(
            self.kube_info(
                self.widget
                    .data
                    .clone()
                    .data
                    .get("kube_info")
                    .and_then(|d| Some(d.clone().unwrap().clone().join("\n"))),
            ),
            rect[0],
        );
    }

    fn get_widget(&mut self) -> &mut CliWidget {
        &mut self.widget
    }
}

impl<'a> RenderWidget for BodyWidget {
    fn render(&mut self, f: &mut Frame, layout: MainLayoutUI) {
        trace!("rendering widget with data {:?}", self.widget.data.clone());
        match self.widget.title.clone() {
            Some(title) => {
                if let Some(logs) = self.get_data().data.get("logs") {
                    let rect = layout.get_body_rect(f);
                    if self.black {
                        f.render_widget(
                            self.widget
                                .content_in_black(
                                    title.to_string(),
                                    logs.clone(),
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
                                    self.widget.data.clone().data.get("logs").unwrap().clone(),
                                    rect[self.widget.pos],
                                )
                                .unwrap_or_default(),
                            rect[self.widget.pos],
                        );
                    }
                }
            }
            None => {}
        }
    }

    fn get_widget(&mut self) -> &mut CliWidget {
        &mut self.widget
    }
}

impl<'a> CliWidget {
    pub fn bordered(id: CliWidgetId, title: String, pos: usize, data: CliWidgetData) -> Self {
        CliWidget {
            id,
            title: Some(title),
            data,
            pos,
            is_selected: false,
        }
    }

    pub fn unbordered(id: CliWidgetId, data: CliWidgetData) -> Self {
        CliWidget {
            id,
            title: None,
            data,
            pos: 0,
            is_selected: false,
        }
    }

    fn content_in_black(
        &self,
        title: String,
        logs: Option<Vec<String>>,
        rect: Rect,
    ) -> Option<Paragraph<'a>> {
        let bg_color = Color::Black;
        let fg_color = Color::White;
        let border_color = match self.is_selected {
            true => Color::Red,
            false => fg_color,
        };
        if let Some(log) = logs {
            Some(
                Paragraph::new(log.join(""))
                    .scroll((Self::calculate_scroll(log, rect), 50))
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
        } else {
            None
        }
    }

    fn content_in_white(
        &self,
        title: String,
        logs: Option<Vec<String>>,
        rect: Rect,
    ) -> Option<Paragraph<'a>> {
        if let Some(log) = logs {
            Some(
                Paragraph::new(log.join("\n"))
                    .scroll((Self::calculate_scroll(log, rect), 0))
                    .block(Block::new().title(title).borders(Borders::ALL))
                    .style(Style::new().bg(Color::White).fg(Color::Black))
                    .alignment(Alignment::Left)
                    .wrap(Wrap { trim: true }),
            )
        } else {
            None
        }
    }

    fn calculate_scroll(lines: Vec<String>, estate: Rect) -> u16 {
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
    if let Some(Some(existing_test)) = &mut widget.get_data().data.get_mut("logs") {
        existing_test.push(text);
        widget.set_data("logs".to_string(), existing_test.to_vec());
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
        event_handler: |_, _| {},
        keymap: |_| {},
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
        CliWidget::bordered(
            CliWidgetId::GetLoginLogs,
            "Logging in...".to_string(),
            0,
            login_widget_data,
        ),
    );
    let login_event_handler = |event: &TUIEvent, store: &mut Store| match event {
        TUIEvent::AddLoginLog(log_part) => {
            add_to_widget_data(store.login_widget.as_mut().unwrap(), log_part.to_string());
        }
        _ => {}
    };
    WidgetDescription {
        widget: login_widget,
        event_handler: login_event_handler,
        keymap: |_| {},
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
        CliWidget::bordered(
            CliWidgetId::GetLogs,
            "Salespoint Logs".to_string(),
            0,
            logs_widget_data,
        ),
    );
    let logs_event_handler = |event: &TUIEvent, store: &mut Store| match event {
        TUIEvent::AddLog(log_part) => {
            add_to_widget_data(store.logs_widget.as_mut().unwrap(), log_part.to_string());
        }
        _ => {}
    };
    WidgetDescription {
        widget: logs_widget,
        event_handler: logs_event_handler,
        keymap: |_| {},
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
        CliWidget::bordered(
            CliWidgetId::GetPods,
            "Salespoint pods".to_string(),
            1,
            pods_widget_data,
        ),
    );
    let pods_event_handler = |event: &TUIEvent, store: &mut Store| match event {
        TUIEvent::AddPods(pods) => {
            add_to_widget_data(store.pods_widget.as_mut().unwrap(), pods.to_string());
        }
        _ => {}
    };
    WidgetDescription {
        widget: pods_widget,
        event_handler: pods_event_handler,
        keymap: |_| {},
    }
}

pub fn create_tail_widget_data<'a>() -> WidgetDescription<BodyWidget> {
    let tail_widget_data = CliWidgetData {
        id: CliWidgetId::Tail,
        thread_started: false,
        initiate_thread: Some(|a| {
            a.send(TUIAction::GetTail).unwrap();
        }),
        data: HashMap::default(),
    };
    let tail_widget = BodyWidget::new(
        true,
        CliWidget::bordered(
            CliWidgetId::Tail,
            "cli logs".to_string(),
            1,
            tail_widget_data,
        ),
    );
    let tail_event_handler = |event: &TUIEvent, store: &mut Store| match event {
        TUIEvent::AddTailLog(tail_log) => {
            add_to_widget_data(store.tail_widget.as_mut().unwrap(), tail_log.to_string());
        }
        _ => {}
    };
    WidgetDescription {
        widget: tail_widget,
        event_handler: tail_event_handler,
        keymap: |_| {},
    }
}

#[derive(Clone)]
pub struct WidgetDescription<T: RenderWidget + Clone> {
    widget: T,
    event_handler: fn(&TUIEvent, &mut Store),
    keymap: fn(KeyCode),
}

impl<T: RenderWidget + Clone> WidgetDescription<T> {
    pub fn get_widget(&self) -> T {
        self.widget.clone()
    }

    pub fn get_event_handler(&self) -> fn(&TUIEvent, &mut Store) {
        self.event_handler
    }

    pub fn get_keymap(&self) -> fn(KeyCode) {
        self.keymap
    }
}
