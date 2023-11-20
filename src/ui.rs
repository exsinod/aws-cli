use crate::widgets::{CliWidgetId, RenderWidget};
use std::{cell::RefCell, rc::Rc, sync::mpsc::Sender};

use log::trace;
use ratatui::{
    prelude::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{widgets::CliWidget, Store, TUIAction};

#[derive(Clone)]
pub struct UITransform {
    pub direction: Option<crate::Direction>,
}

impl UITransform {
    pub fn new() -> Self {
        UITransform { direction: None }
    }
}

pub struct UI {
    pub layout: Option<Vec<Rect>>,
    pub widgets: Vec<Rc<RefCell<CliWidget>>>,
    pub ui_transform: UITransform,
    pub store: Store,
    action_tx: Sender<TUIAction>,
}

impl<'a> UI {
    pub fn new(store: Store, action_tx: Sender<TUIAction>) -> Self {
        UI {
            layout: None,
            widgets: Vec::new(),
            ui_transform: UITransform::new(),
            store,
            action_tx,
        }
    }

    pub fn add_layout(&mut self, layout: Vec<Rect>) {
        self.layout = Some(layout)
    }
    pub fn add_widgets(&mut self, mut widgets: Vec<CliWidget>) {
        for (i, widget) in widgets.iter_mut().enumerate() {
            widget.pos = i;
            self.widgets.push(Rc::new(RefCell::new(widget.clone())));
        }
    }

    pub fn update_widgets(&mut self) {
        let mut updated_widgets = vec![];

        for widget in self.widgets.iter_mut() {
            let mut widget_taken = widget.borrow_mut();
            if let Some(data_fn) = widget_taken.data_fn {
                let data = data_fn(self.store.clone());
                widget_taken.data = data.clone();
                updated_widgets.push(Rc::new(RefCell::new(widget_taken.clone())));

                trace!(
                    "update_widgets, widget is {:?}, data is {:?}, store is {:?}",
                    widget,
                    data,
                    self.store
                );
            }
        }
        if !updated_widgets.is_empty() {
            self.widgets = updated_widgets.clone();
            trace!("updated_widgets are {:?}", updated_widgets);
        }
        //
        // if let Some(direction) = &self.ui_transform.direction {
        //     match direction {
        //         crate::Direction::Right => {
        //             self.pos = 1;
        //         }
        //         crate::Direction::Left => {
        //             self.pos = 2;
        //         }
        //         _ => {}
        //     }
        // }
        // if self.pos == 1 {
        //     logs_widget.is_selected(false);
        //     pods_widget.is_selected(true);
        // } else if self.pos == 2 {
        //     logs_widget.is_selected(true);
        //     pods_widget.is_selected(false);
        // }
    }

    pub fn ui(&mut self, f: &mut Frame<'_>) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Max(1), Constraint::Percentage(90)])
            .split(f.size());
        let header_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_layout[0]);

        f.render_widget(Self::header_error(self.store.clone()), header_layout[0]);
        f.render_widget(
            Self::header_login_info(self.store.clone()),
            header_layout[1],
        );
        self.update_widgets();

        if let true = self.store.logged_in {
            let layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(main_layout[1]);
            self.add_layout(layout.to_vec());

            if !self.store.log_thread_started {
                Self::trigger_action(self.action_tx.clone(), TUIAction::GetLogs);
                Self::trigger_action(self.action_tx.clone(), TUIAction::GetPods);
            }

            trace!("widgets after update are {:?}", self.widgets);
            for (i, widget) in self
                .widgets
                .iter()
                .filter(|w| w.borrow().id != CliWidgetId::Login)
                .enumerate()
            {
                let mut borrow_widget = widget.borrow().clone();
                borrow_widget.render(f, layout[i]);
            }
        } else {
            let layout = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![Constraint::Percentage(100)])
                .split(main_layout[1]);
            if self.store.login_request {
                f.render_widget(Paragraph::new("\nWhat do you want to do?\n\n1. retry (I forgot to turn on my VPN)\n2. Login to AWS").block(Block::default().borders(Borders::all()).title("It seems I can't reach your resources...")),
                Self::centered_rect(layout[0], 50, 30))
            }
            let login_widget = self
                .widgets
                .iter()
                .find(|w| w.borrow().id == CliWidgetId::Login)
                .unwrap()
                .borrow();
            trace!("rendering login window {:?}", login_widget);
            login_widget.clone().render(f, layout[0]);
        }
    }

    fn trigger_action(action_tx: Sender<TUIAction>, action: TUIAction) {
        action_tx.send(action).unwrap();
    }

    fn header_error(store: Store) -> Paragraph<'a> {
        Paragraph::new(if let Some(error) = store.error {
            Span::styled(error, Style::default().fg(Color::Red))
        } else {
            Span::styled("All is good", Style::default().fg(Color::LightGreen))
        })
        .block(Block::new().borders(Borders::NONE))
        .alignment(Alignment::Right)
    }
    fn header_login_info(store: Store) -> Paragraph<'a> {
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

    fn centered_rect(r: Rect, percent_x: u16, percent_y: u16) -> Rect {
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
