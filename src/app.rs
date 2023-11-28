use std::{
    io,
    sync::mpsc::{Receiver, Sender},
    time::Duration,
};

use crossterm::event::{self, Event, KeyCode};
use log::{debug, trace};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};

use crate::{
    structs::{Direction2, KubeEnv, Store, TUIError, TUIEvent, UserInput},
    ui::{MainLayoutUI, SingleLayoutUI, UI},
    widgets::{CliWidgetId, RenderWidget},
};

struct ThreadManage {
    logs_thread_started: bool,
    pods_thread_started: bool,
    tail_thread_started: bool,
}

impl ThreadManage {
    fn new(
        logs_thread_started: bool,
        pods_thread_started: bool,
        tail_thread_started: bool,
    ) -> Self {
        ThreadManage {
            logs_thread_started,
            pods_thread_started,
            tail_thread_started,
        }
    }
}

pub struct App<'a, B>
where
    B: Backend,
{
    terminal: &'a mut Terminal<B>,
    store_rx: Receiver<Store>,
    event_tx: Sender<TUIEvent>,
    thread_mngt: Option<ThreadManage>,
    extended_keymap: Vec<fn(KeyCode)>,
}

impl<'a, B: Backend> App<'a, B> {
    pub fn new(
        terminal: &'a mut Terminal<B>,
        store_rx: Receiver<Store>,
        event_tx: Sender<TUIEvent>,
        extended_keymap: Vec<fn(KeyCode)>,
    ) -> Self {
        App {
            terminal,
            store_rx,
            event_tx,
            thread_mngt: None,
            extended_keymap,
        }
    }

    pub fn run_app(&mut self) -> io::Result<()> {
        let mut should_quit = false;
        let mut store: Option<Store> = None;
        let logs_thread_started = false;
        let login_logs_thread_started = false;
        let tail_thread_started = false;
        self.thread_mngt = Some(ThreadManage::new(
            logs_thread_started,
            login_logs_thread_started,
            tail_thread_started,
        ));

        while let false = should_quit {
            while let Ok(updated_store) = self.store_rx.recv_timeout(Duration::from_millis(20)) {
                trace!("got store {:?}", updated_store);
                store = Some(updated_store.clone());
                let mut ui = UI::main(&MainLayoutUI::new());
                ui.widgets
                    .push(Box::new(updated_store.clone().header_widget.unwrap()));
                self.initiate_threads(updated_store.clone());
                if updated_store
                    .clone()
                    .login_widget
                    .unwrap()
                    .get_data()
                    .data
                    .get("logs")
                    .is_some()
                {
                    ui.widgets
                        .push(Box::new(updated_store.login_widget.unwrap()));
                } else if updated_store.logged_in {
                    ui.widgets
                        .push(Box::new(updated_store.pods_widget.unwrap()));
                    ui.widgets
                        .push(Box::new(updated_store.logs_widget.unwrap()));
                } else if updated_store.request_login {
                    ui = UI::single(&SingleLayoutUI::new());
                    ui.widget_fn = Some(|f, layout| {
                        f.render_widget(
                            Paragraph::new(
                                "\nWhat do you want to do?\n\n
                                1. retry (I forgot to turn on my VPN)\n
                                2. Login to AWS",
                            )
                            .block(
                                Block::default()
                                    .borders(Borders::all())
                                    .title("It seems I can't reach your resources..."),
                            ),
                            Self::centered_rect(layout, 50, 30),
                        )
                    });
                } else {
                }
                self.terminal.draw(|f| ui.ui(f)).unwrap();
            }
            if let Some(store) = store.as_ref() {
                let user_input =
                    self.handle_user_input(store.clone(), self.extended_keymap.clone());
                if let Some(input) = user_input {
                    match input {
                        UserInput::Quit => {
                            debug!("Exiting");
                            should_quit = true;
                        }
                        UserInput::ChangeEnv => {
                            debug!("Change env mode");
                            self.event_tx.send(TUIEvent::RequestEnvChange).unwrap();
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
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
    fn initiate_threads(&mut self, updated_store: Store) {
        if updated_store.logged_in {
            if !self.thread_mngt.as_mut().unwrap().logs_thread_started {
                debug!("initiate logs thread");
                self.event_tx
                    .send(TUIEvent::LogThreadStarted(CliWidgetId::GetLogs))
                    .unwrap();
                self.thread_mngt.as_mut().unwrap().logs_thread_started = true;
            }
            if !self.thread_mngt.as_mut().unwrap().tail_thread_started {
                debug!("initiate tail thread");
                self.event_tx
                    .send(TUIEvent::LogThreadStarted(CliWidgetId::Tail))
                    .unwrap();
                self.thread_mngt.as_mut().unwrap().tail_thread_started = true;
            }
            if !self.thread_mngt.as_mut().unwrap().pods_thread_started {
                debug!("initiate pods thread");
                self.event_tx
                    .send(TUIEvent::LogThreadStarted(CliWidgetId::GetPods))
                    .unwrap();
                self.thread_mngt.as_mut().unwrap().pods_thread_started = true;
            }
        }
    }

    fn handle_user_input(
        &self,
        store: Store,
        extended_keymap: Vec<fn(KeyCode)>,
    ) -> Option<UserInput> {
        let mut user_input: Option<UserInput> = None;
        if let Ok(true) = event::poll(Duration::from_millis(10)) {
            if let Ok(Event::Key(key)) = event::read() {
                user_input = Self::handle_primary_keys(key.code).or_else(|| {
                    Self::handle_direction_keys(key.code).or_else(|| {
                        if store.env_change_possible {
                            match key.code {
                                KeyCode::Char('1') => {
                                    self.event_tx
                                        .send(TUIEvent::EnvChange(KubeEnv::Dev))
                                        .unwrap();
                                }
                                KeyCode::Char('2') => {
                                    self.event_tx
                                        .send(TUIEvent::EnvChange(KubeEnv::Prod))
                                        .unwrap();
                                }
                                _ => {}
                            }
                        } else if store.request_login {
                            match key.code {
                                KeyCode::Char('1') => {
                                    self.event_tx.send(TUIEvent::RequestLoginStop).unwrap();
                                    self.event_tx.send(TUIEvent::ClearError).unwrap();
                                    self.event_tx.send(TUIEvent::CheckConnectivity).unwrap();
                                }
                                KeyCode::Char('2') => {
                                    self.event_tx.send(TUIEvent::RequestLoginStop).unwrap();
                                    self.event_tx.send(TUIEvent::NeedsLogin).unwrap()
                                }
                                _ => {
                                    match key.code {
                                        KeyCode::Null => {}
                                        _ => {
                                            self.event_tx
                                                .send(TUIEvent::Error(TUIError::KEY(
                                                    "Unrecognised key: ".to_string()
                                                        + &format!("{:?}", key.code).to_string()
                                                        + " Press q to quit",
                                                )))
                                                .unwrap();
                                        }
                                    };
                                }
                            }
                        } else {
                            for check in extended_keymap {
                                check(key.code)
                            }
                            match key.code {
                                KeyCode::Null => {}
                                _ => {
                                    self.event_tx
                                        .send(TUIEvent::Error(TUIError::KEY(
                                            "Unrecognised key: ".to_string()
                                                + &format!("{:?}", key.code).to_string()
                                                + " Press q to quit",
                                        )))
                                        .unwrap();
                                }
                            };
                        }
                        None
                    })
                });
            }
        }
        user_input
    }

    fn handle_primary_keys(keycode: KeyCode) -> Option<UserInput> {
        return if let KeyCode::Char('q') = keycode {
            Some(UserInput::Quit)
        } else if let KeyCode::Char('E') = keycode {
            Some(UserInput::ChangeEnv)
        } else {
            None
        };
    }

    fn handle_direction_keys(keycode: KeyCode) -> Option<UserInput> {
        return if keycode == KeyCode::Char('h') {
            Some(UserInput::Direction(Direction2::Left))
        } else if keycode == KeyCode::Char('j') {
            Some(UserInput::Direction(Direction2::Down))
        } else if keycode == KeyCode::Char('k') {
            Some(UserInput::Direction(Direction2::Up))
        } else if keycode == KeyCode::Char('l') {
            Some(UserInput::Direction(Direction2::Right))
        } else {
            None
        };
    }
}
