use log::trace;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::Store;

#[derive(Debug, Clone, PartialEq)]
pub enum CliWidgetId {
    Login,
    GetLogs,
    GetPods,
}

pub trait RenderWidget {
    fn render(&mut self, f: &mut Frame, rect: Rect);
}

pub trait UpdateVisitor {
    fn is_selected(&mut self, is_selected: bool);
}

#[derive(Debug, Clone)]
pub struct CliWidget {
    pub id: CliWidgetId,
    pub title: String,
    pub data: Option<String>,
    pub data_fn: Option<fn(store: Store) -> Option<String>>,
    pub layout: Vec<Rect>,
    pub pos: usize,
    is_selected: bool,
}

impl UpdateVisitor for CliWidget {
    fn is_selected(&mut self, is_selected: bool) {
        self.is_selected = is_selected;
    }
}

impl RenderWidget for CliWidget {
    fn render(&mut self, f: &mut Frame, rect: Rect) {
        trace!("rendering widget with data {:?}", self.data.clone());
        f.render_widget(
            self.content_in_black(self.title.to_string(), self.data.clone(), rect)
                .unwrap_or_default(),
            rect,
        );
    }
}

impl CliWidget {
    pub fn new(id: CliWidgetId, title: String) -> Self {
        CliWidget {
            id,
            title,
            data: None,
            data_fn: None,
            layout: vec![],
            pos: 0,
            is_selected: false,
        }
    }

    fn content_in_black<'a>(
        &mut self,
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

    // fn content_in_white<'a>(
    //     title: String,
    //     logs: Option<String>,
    //     rect: Rect,
    // ) -> Option<Paragraph<'a>> {
    //     if let Some(log) = logs {
    //         Some(
    //             Paragraph::new(log.to_string())
    //                 .scroll((Self::calculate_scroll(log, rect), 0))
    //                 .block(Block::new().title(title).borders(Borders::ALL))
    //                 .style(Style::new().bg(Color::White).fg(Color::Black))
    //                 .alignment(Alignment::Left)
    //                 .wrap(Wrap { trim: false }),
    //         )
    //     } else {
    //         None
    //     }
    // }

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
