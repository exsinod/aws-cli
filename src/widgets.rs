use std::{
    collections::HashMap,
    rc::Rc,
    sync::{mpsc::Sender, Arc, Mutex},
};

use crossterm::event::KeyCode;
use log::{debug, trace};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::{
    app::{DataStream, StreamType},
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
    fn get_widget(&self) -> &Arc<Mutex<CliWidget>>;
    fn get_widget_mut(&mut self) -> &mut Arc<Mutex<CliWidget>>;

    fn get_title(&self) -> Option<String> {
        return match self.get_widget().try_lock() {
            Ok(ref mut widget) => {
                debug!("yeah {:?}", widget);
                widget.title.clone()
            }
            Err(error) => {
                debug!("no {:?}", error);
                Some("".to_string())
            }
        }
    }

    fn set_title(&mut self, title: &str) {
        match self.get_widget_mut().try_lock() {
            Ok(ref mut widget) => {
                debug!("yeah {:?}", widget);
                widget.title = Some(title.to_string());
            }
            Err(error) => {
                debug!("no {:?}", error);
            }
        }
    }

    fn get_data(&self) -> CliWidgetData {
        return match self.get_widget().try_lock() {
            Ok(ref mut widget) => {
                debug!("yeah {:?}", widget);
                widget.data.clone()
            }
            Err(error) => {
                debug!("no {:?}", error);
                CliWidgetData::default()
            }
        }
    }

    fn set_data(&mut self, key: String, text: Vec<String>) {
        match self.get_widget_mut().try_lock() {
            Ok(ref mut widget) => {
                debug!("yeah {:?}", widget);
                widget.data.data.insert(key, Some(text));
            }
            Err(error) => {
                debug!("no {:?}", error);
            }
        }
    }

    fn clear_text_data(&mut self, key: &str) {
        match self.get_widget_mut().try_lock() {
            Ok(ref mut widget) => {
                debug!("yeah {:?}", widget);
                widget.data.data.insert(key.to_string(), None);
            }
            Err(error) => {
                debug!("no {:?}", error);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct HeaderWidget {
    i think this is not possible because it is locked as long as &self
        so best to let main own the arc mutex
    pub widget: Arc<Mutex<CliWidget>>,
}

#[derive(Clone, Debug)]
pub struct ErrorActionWidget {
    pub widget: Arc<Mutex<CliWidget>>,
}

#[derive(Clone, Debug)]
pub struct BodyWidget {
    full_screen: bool,
    pub widget: Arc<Mutex<CliWidget>>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub enum ColorScheme {
    #[default]
    Black,
    White,
}

#[derive(Debug, Clone)]
pub struct CliWidget {
    pub id: CliWidgetId,
    pub title: Option<String>,
    pub data: CliWidgetData,
    pub pos: usize,
    pub color_scheme: ColorScheme,
    is_selected: bool,
}

impl HeaderWidget {
    pub fn new(widget: Arc<Mutex<CliWidget>>) -> Self {
        HeaderWidget { widget }
    }
}

impl ErrorActionWidget {
    pub fn new(widget: Arc<Mutex<CliWidget>>) -> Self {
        ErrorActionWidget { widget }
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
    pub fn new(full_screen: bool, widget: Arc<Mutex<CliWidget>>) -> Self {
        BodyWidget {
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
        if let Some(error) = self.widget.lock().unwrap().data.data.get("error") {
            f.render_widget(
                self.header_error(error.as_ref().and_then(|e| Some(e.join("\n")))),
                rect[0],
            );
        }
        if let Some(login_info) = self.widget.lock().unwrap().data.data.get("login_info") {
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
        if let Some(kube_info) = self.widget.lock().unwrap().data.data.get("kube_info") {
            f.render_widget(
                self.kube_info(kube_info.as_ref().and_then(|e| Some(e.join("\n")))),
                rect[0],
            );
        }
    }

    fn get_widget(&self) -> &Arc<Mutex<CliWidget>> {
        &self.widget
    }

    fn get_widget_mut(&mut self) -> &mut Arc<Mutex<CliWidget>> {
        &mut self.widget
    }
}

impl<'a> RenderWidget for ErrorActionWidget {
    fn render(&self, f: &mut Frame, layout: &MainLayoutUI) {
        let rect = layout.get_full_rect(f);
        f.render_widget(
            self.widget
                .lock()
                .unwrap()
                .render("logs", rect[0])
                .unwrap_or_default(),
            self.centered_rect(rect[0], 50, 30),
        );
    }

    fn get_widget(&self) -> &Arc<Mutex<CliWidget>> {
        &self.widget
    }

    fn get_widget_mut(&mut self) -> &mut Arc<Mutex<CliWidget>> {
        &mut self.widget
    }
}

impl<'a> RenderWidget for BodyWidget {
    fn render(&self, f: &mut Frame, layout: &MainLayoutUI) {
        trace!(
            "rendering widget with data {:?}",
            self.widget.lock().unwrap().data.clone()
        );
        let rect: Rc<[Rect]>;
        if self.full_screen {
            rect = layout.get_full_rect(f);
        } else {
            rect = layout.get_body_rect(f);
        }
        f.render_widget(
            self.widget
                .lock()
                .unwrap()
                .render("logs", rect[self.widget.lock().unwrap().pos])
                .unwrap_or_default(),
            rect[self.widget.lock().unwrap().pos],
        );
    }

    fn get_widget(&self) -> &Arc<Mutex<CliWidget>> {
        &self.widget
    }

    fn get_widget_mut(&mut self) -> &mut Arc<Mutex<CliWidget>> {
        &mut self.widget
    }
}

impl<'a> CliWidget {
    pub fn bordered(
        id: CliWidgetId,
        title: &str,
        pos: usize,
        data: CliWidgetData,
        color_scheme: ColorScheme,
    ) -> Self {
        CliWidget {
            id,
            title: Some(title.to_string()),
            data,
            pos,
            color_scheme,
            is_selected: false,
        }
    }

    pub fn unbordered(id: CliWidgetId, data: CliWidgetData, color_scheme: ColorScheme) -> Self {
        CliWidget {
            id,
            title: None,
            data,
            pos: 0,
            color_scheme,
            is_selected: false,
        }
    }

    fn render(&self, data_key: &str, rect: Rect) -> Option<Paragraph<'a>> {
        if let Some(title) = &self.title {
            if let Some(Some(logs)) = self.data.data.get(data_key) {
                // default Black
                let mut bg_color = Color::Black;
                let mut fg_color = Color::White;
                if self.color_scheme == ColorScheme::White {
                    bg_color = Color::White;
                    fg_color = Color::Black;
                }
                let border_color = match self.is_selected {
                    true => Color::Red,
                    false => fg_color,
                };

                Some(
                    Paragraph::new(logs.join(""))
                        .scroll((Self::calculate_scroll(&logs, &rect), 50))
                        .block(
                            Block::new()
                                .title(title.to_string())
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
        } else {
            None
        }
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
    let header_data_stream = DataStream::new(StreamType::Once, |_| {});
    let header_data = CliWidgetData::new(CliWidgetId::Header, header_data_stream);
    let header_widget = HeaderWidget::new(Arc::new(Mutex::new(CliWidget::unbordered(
        CliWidgetId::Header,
        header_data,
        ColorScheme::default(),
    ))));
    WidgetDescription {
        widget: header_widget,
        event_handler: |_, _| None,
        keymap: |_, _, _| {},
    }
}

pub fn create_login_widget_data<'a>() -> WidgetDescription<BodyWidget> {
    let login_data_stream = DataStream::new(StreamType::Once, |_| {});
    let login_widget_data = CliWidgetData {
        id: CliWidgetId::GetLoginLogs,
        data_stream: login_data_stream,
        thread_started: false,
        initiate_thread: None,
        data: HashMap::default(),
    };
    let login_widget = BodyWidget::new(
        true,
        Arc::new(Mutex::new(CliWidget::bordered(
            CliWidgetId::GetLoginLogs,
            "Logging in...",
            0,
            login_widget_data,
            ColorScheme::White,
        ))),
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
    let logs_data_stream = DataStream::new(StreamType::LeaveOpen, |action_tx| {
        action_tx.send(TUIAction::GetLogs).unwrap();
    });
    let logs_widget_data = CliWidgetData {
        id: CliWidgetId::GetLogs,
        data_stream: logs_data_stream,
        thread_started: false,
        initiate_thread: Some(|a| {
            a.send(TUIAction::GetLogs).unwrap();
        }),
        data: HashMap::default(),
    };
    let logs_widget = BodyWidget::new(
        false,
        Arc::new(Mutex::new(CliWidget::bordered(
            CliWidgetId::GetLogs,
            "Salespoint Logs",
            0,
            logs_widget_data,
            ColorScheme::default(),
        ))),
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
    let pods_data_stream = DataStream::new(StreamType::Periodical, |action_tx| {
        action_tx.send(TUIAction::GetPods).unwrap();
    });
    let pods_widget_data = CliWidgetData {
        id: CliWidgetId::GetPods,
        data_stream: pods_data_stream,
        thread_started: false,
        initiate_thread: Some(|a| {
            a.send(TUIAction::GetPods).unwrap();
        }),
        data: HashMap::default(),
    };
    let pods_widget = BodyWidget::new(
        false,
        Arc::new(Mutex::new(CliWidget::bordered(
            CliWidgetId::GetPods,
            "Salespoint pods",
            1,
            pods_widget_data,
            ColorScheme::default(),
        ))),
    );
    let pods_event_handler = |event: &TUIEvent, store: &mut Store| match event {
        TUIEvent::AddPods(pods) => {
            let widget = store.pods_widget.as_mut().unwrap();
            widget.clear_text_data("logs");
            widget.set_data("logs".to_string(), vec![pods.to_string()]);
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
    let request_login_data_stream = DataStream::new(StreamType::Once, |_| {});
    let login_request_widget_data = CliWidgetData {
        id: CliWidgetId::RequestLogin,
        data_stream: request_login_data_stream,
        thread_started: false,
        initiate_thread: Some(|_| {}),
        data: HashMap::default(),
    };
    let login_request_widget = ErrorActionWidget::new(Arc::new(Mutex::new(CliWidget::bordered(
        CliWidgetId::LoginRequest,
        "",
        1,
        login_request_widget_data,
        ColorScheme::default(),
    ))));
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
