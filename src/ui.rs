use crate::widgets::RenderWidget;
use std::rc::Rc;

use ratatui::{
    prelude::{Constraint, Direction, Layout, Rect},
    Frame,
};

#[derive(Clone)]
pub struct UITransform {
    pub direction: Option<crate::Direction>,
}

impl UITransform {
    pub fn new() -> Self {
        UITransform { direction: None }
    }
}

pub trait ShowUI<'a> {
    fn ui(&mut self, f: &mut Frame<'_>);
}

#[derive(Clone)]
pub struct SingleLayoutUI {}

impl SingleLayoutUI {
    pub fn new() -> Self {
        SingleLayoutUI {}
    }

    pub fn get_body_rect(&self, f: &mut Frame<'_>) -> Rect {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(100)])
            .split(f.size())[0]
    }
}

#[derive(Clone)]
pub struct MainLayoutUI<'a> {
    pub draw_frame: Option<fn() -> &'a mut Frame<'a>>,
}

impl<'a> MainLayoutUI<'a> {
    pub fn new() -> Self {
        MainLayoutUI { draw_frame: None }
    }

    pub fn get_full_rect(&self, f: &mut Frame<'_>) -> Rc<[Rect]> {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Max(1),
                Constraint::Max(1),
                Constraint::Percentage(90),
            ])
            .split(f.size());
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(100)])
            .split(main_layout[2])
    }

    pub fn get_body_rect(&self, f: &mut Frame<'_>) -> Rc<[Rect]> {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Max(1),
                Constraint::Max(1),
                Constraint::Percentage(90),
            ])
            .split(f.size());
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_layout[2])
    }

    pub fn get_header_rect(&self, line: usize, f: &mut Frame<'_>) -> Rc<[Rect]> {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Max(1),
                Constraint::Max(1),
                Constraint::Percentage(90),
            ])
            .split(f.size());
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_layout[line])
    }
}

pub struct UI<'a> {
    main_layout: Option<&'a MainLayoutUI<'a>>,
    single_layout: Option<&'a SingleLayoutUI>,
    widgets: Option<Vec<Box<&'a dyn RenderWidget>>>,
    pub widget_fn: Option<fn(f: &mut Frame<'_>, layout: Rect)>,
    pub ui_transform: UITransform,
}

impl<'a> UI<'a> {
    pub fn main(main_layout: &'a MainLayoutUI<'a>) -> Self {
        UI {
            main_layout: Some(main_layout),
            single_layout: None,
            widgets: None,
            widget_fn: None,
            ui_transform: UITransform::new(),
        }
    }
    pub fn single(main_layout: &'a SingleLayoutUI) -> Self {
        UI {
            main_layout: None,
            single_layout: Some(main_layout),
            widgets: None,
            widget_fn: None,
            ui_transform: UITransform::new(),
        }
    }

    pub fn ui(&mut self, f: &mut Frame<'_>) {
        if let Some(main_layout) = &self.main_layout {
            if let Some(widgets) = &self.widgets {
                for widget in widgets.iter() {
                    widget.render(f, main_layout);
                }
            }
        }
        if let Some(single_layout) = &self.single_layout {
            let rect = single_layout.get_body_rect(f);
            (self.widget_fn.unwrap())(f, rect);
        }
    }

    pub fn add_to_widgets(&mut self, widgets: Vec<Box<&'a dyn RenderWidget>>) {
        self.widgets = Some(widgets);
    }
}
