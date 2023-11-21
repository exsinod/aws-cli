use std::collections::HashMap;

use log::trace;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{structs::CliWidgetData, ui::MainLayoutUI};

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
    fn get_data(&mut self, key: String) -> CliWidgetData;
    fn set_thread_started(&mut self, key: String, started: bool);
    fn set_text_data(
        &mut self,
        key: String,
        text: Option<String>,
    ) -> HashMap<String, CliWidgetData>;
}

#[derive(Clone, Debug, Default)]
pub struct HeaderWidget {
    widget: CliWidget,
}

#[derive(Clone, Debug, Default)]
pub struct BodyWidget {
    black: bool,
    widget: CliWidget,
}

impl HeaderWidget {
    pub fn new(widget: CliWidget) -> Self {
        HeaderWidget { widget }
    }
}

impl<'a> RenderWidget for HeaderWidget {
    fn render(&mut self, f: &mut Frame, layout: MainLayoutUI) {
        let rect = layout.get_header_rect(0, f);
        f.render_widget(
            self.header_error(
                self.widget
                    .data
                    .get("error")
                    .and_then(|d| d.text.clone()),
            ),
            rect[0],
        );
        f.render_widget(
            self.header_login_info(
                false,
                self.widget
                    .data
                    .get("login_info")
                    .and_then(|d| d.text.clone()),
            ),
            rect[1],
        );
        let rect = layout.get_header_rect(1, f);
        f.render_widget(
            self.kube_info(
                self.widget
                    .data
                    .get("kube_info")
                    .and_then(|d| d.text.clone()),
            ),
            rect[0],
        );
    }

    fn get_data(&mut self, key: String) -> CliWidgetData {
        self.widget.data.get(&key).unwrap().clone()
    }

    fn set_thread_started(&mut self, key: String, started: bool) {
        self.widget.data.get_mut(&key).unwrap().thread_started = started
    }

    fn set_text_data(
        &mut self,
        key: String,
        text: Option<String>,
    ) -> HashMap<String, CliWidgetData> {
        self.widget.data.get_mut(&key).unwrap().text = text;
        self.widget.data.clone()
    }
}

impl<'a> HeaderWidget {
    fn kube_info(&self, text: Option<String>) -> Paragraph<'a> {
        Paragraph::new(Span::styled(text.unwrap_or("".to_string()), Style::default().fg(Color::Red)))
        .block(Block::new().borders(Borders::NONE))
        .alignment(Alignment::Right)
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

impl<'a> RenderWidget for BodyWidget {
    fn render(&mut self, f: &mut Frame, layout: MainLayoutUI) {
        trace!("rendering widget with data {:?}", self.widget.data.clone());
        match &self.widget.title {
            Some(title) => {
                let rect = layout.get_body_rect(f);
                if self.black {
                    f.render_widget(
                        self.widget
                            .content_in_black(
                                title.to_string(),
                                self.widget.data.get("logs").unwrap().text.clone(),
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
                                self.widget.data.get("logs").unwrap().text.clone(),
                                rect[self.widget.pos],
                            )
                            .unwrap_or_default(),
                        rect[self.widget.pos],
                    );
                }
            }
            None => {}
        }
    }

    fn get_data(&mut self, key: String) -> CliWidgetData {
        self.widget.data.get(&key).unwrap().clone()
    }

    fn set_thread_started(&mut self, key: String, started: bool) {
        self.widget.data.get_mut(&key).unwrap().thread_started = started
    }

    fn set_text_data(
        &mut self,
        key: String,
        text: Option<String>,
    ) -> HashMap<String, CliWidgetData> {
        self.widget.data.get_mut(&key).unwrap().text = text;
        self.widget.data.clone()
    }
}

impl BodyWidget {
    pub fn new(black: bool, widget: CliWidget) -> Self {
        BodyWidget { black, widget }
    }
}

#[derive(Debug, Default, Clone)]
pub struct CliWidget {
    pub id: CliWidgetId,
    pub title: Option<String>,
    pub data: HashMap<String, CliWidgetData>,
    pub pos: usize,
    is_selected: bool,
}

impl<'a> CliWidget {
    pub fn bordered(
        id: CliWidgetId,
        title: String,
        pos: usize,
        data: HashMap<String, CliWidgetData>,
    ) -> Self {
        CliWidget {
            id,
            title: Some(title),
            data,
            pos,
            is_selected: false,
        }
    }

    pub fn unbordered(id: CliWidgetId, data: HashMap<String, CliWidgetData>) -> Self {
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
