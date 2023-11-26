use std::collections::HashMap;

use log::{debug, trace};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{
    structs::{CliWidgetData, TUIAction},
    ui::MainLayoutUI,
};

#[derive(Debug, Default, Clone, Hash, Eq, PartialEq)]
pub enum CliWidgetId {
    #[default]
    Default,
    Header,
    Login,
    GetLogs,
    GetLoginLogs,
    GetPods,
}

pub trait RenderWithWidgetData {
    fn render(&mut self, f: &mut Frame, layout: MainLayoutUI);
}

pub trait RenderWidget {
    fn render(&mut self, f: &mut Frame, layout: MainLayoutUI);
    fn get_data(&mut self) -> CliWidgetData;
    fn set_thread_started(&mut self, started: bool);
    fn set_text_data(&mut self, key: String, text: String) -> CliWidgetData;
    fn clear_text_data(&mut self, key: String) -> CliWidgetData;
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
                    .and_then(|d| d.clone()),
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
                    .and_then(|d| d.clone()),
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
                    .and_then(|d| d.clone()),
            ),
            rect[0],
        );
    }

    fn get_data(&mut self) -> CliWidgetData {
        self.widget.data.clone()
    }

    fn set_thread_started(&mut self, started: bool) {
        self.widget.data.thread_started = started
    }

    fn set_text_data(&mut self, key: String, text: String) -> CliWidgetData {
        self.widget.data.data.insert(key, Some(text));
        self.widget.data.clone()
    }

    fn clear_text_data(&mut self, key: String) -> CliWidgetData {
        self.widget.data.clone().data.insert(key, None);
        self.widget.data.clone()
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

    fn get_data(&mut self) -> CliWidgetData {
        self.widget.data.clone()
    }

    fn set_thread_started(&mut self, started: bool) {
        self.widget.data.thread_started = started
    }

    fn set_text_data(&mut self, key: String, text: String) -> CliWidgetData {
        self.widget.data.data.insert(key, Some(text.clone()));
        self.widget.data.clone()
    }

    fn clear_text_data(&mut self, key: String) -> CliWidgetData {
        self.widget.data.clone().data.insert(key, None);
        self.widget.data.clone()
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
        logs: Option<String>,
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
                Paragraph::new(log.to_string())
                    .scroll((Self::calculate_scroll(log, rect), 0))
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
        logs: Option<String>,
        rect: Rect,
    ) -> Option<Paragraph<'a>> {
        if let Some(log) = logs {
            Some(
                Paragraph::new(log.to_string())
                    .scroll((Self::calculate_scroll(log, rect), 0))
                    .block(Block::new().title(title).borders(Borders::ALL))
                    .style(Style::new().bg(Color::White).fg(Color::Black))
                    .alignment(Alignment::Left)
                    .wrap(Wrap { trim: false }),
            )
        } else {
            None
        }
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
}

pub fn create_header_widget_data<'a>() -> HeaderWidget {
    let login_info_data = CliWidgetData::new(CliWidgetId::Header);
    HeaderWidget::new(CliWidget::unbordered(CliWidgetId::Header, login_info_data))
}

pub fn create_login_widget_data<'a>() -> BodyWidget {
    let cli_widget_data = CliWidgetData {
        id: CliWidgetId::GetLoginLogs,
        thread_started: false,
        initiate_thread: None,
        data: HashMap::default(),
    };
    BodyWidget::new(
        false,
        CliWidget::bordered(
            CliWidgetId::GetLoginLogs,
            "Logging in...".to_string(),
            0,
            cli_widget_data,
        ),
    )
}

pub fn create_logs_widget_data<'a>() -> BodyWidget {
    let cli_widget_data = CliWidgetData {
        id: CliWidgetId::GetLogs,
        thread_started: false,
        initiate_thread: Some(|a| {
            a.send(TUIAction::GetLogs).unwrap();
        }),
        data: HashMap::default(),
    };
    BodyWidget::new(
        true,
        CliWidget::bordered(
            CliWidgetId::GetLogs,
            "Salespoint Logs".to_string(),
            0,
            cli_widget_data,
        ),
    )
}

pub fn create_pods_widget_data<'a>() -> BodyWidget {
    let cli_widget_data = CliWidgetData {
        id: CliWidgetId::GetPods,
        thread_started: false,
        initiate_thread: Some(|a| {
            a.send(TUIAction::GetPods).unwrap();
        }),
        data: HashMap::default(),
    };
    BodyWidget::new(
        true,
        CliWidget::bordered(
            CliWidgetId::GetPods,
            "Salespoint pods".to_string(),
            1,
            cli_widget_data,
        ),
    )
}
