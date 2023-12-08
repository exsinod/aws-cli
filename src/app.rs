use std::{
    io::{self},
    sync::mpsc::{Receiver, Sender},
    time::{Duration, SystemTime},
};

use crossterm::event::{self, Event, KeyCode};
use log::debug;
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};

use crate::{
    structs::{Direction2, KubeEnv, Store, TUIAction, TUIError, TUIEvent, UserInput},
    ui::{MainLayoutUI, SingleLayoutUI, UI},
    widgets::RenderWidget,
};

struct ThreadManage {
    logs_thread_started: bool,
    pods_thread_started: bool,
}

impl ThreadManage {
    fn new(logs_thread_started: bool, pods_thread_started: bool) -> Self {
        ThreadManage {
            logs_thread_started,
            pods_thread_started,
        }
    }
}

pub struct App<'a, B>
where
    B: Backend,
{
    is_running: bool,
    terminal: &'a mut Terminal<B>,
    event_tx: Sender<TUIEvent>,
    action_tx: Sender<TUIAction>,
    extended_keymap: &'a Vec<fn(KeyCode, &Store, &Sender<TUIEvent>)>,
}

impl<'a, B: Backend> App<'a, B> {
    pub fn new(
        terminal: &'a mut Terminal<B>,
        event_tx: Sender<TUIEvent>,
        action_tx: Sender<TUIAction>,
        extended_keymap: &'a Vec<fn(KeyCode, &Store, &Sender<TUIEvent>)>,
    ) -> Self {
        App {
            is_running: true,
            terminal,
            event_tx,
            action_tx,
            extended_keymap,
        }
    }

    pub fn run_app(&mut self, store_rx: Receiver<Store>) -> io::Result<()> {
        let mut store_presenter = StorePresenter::init(
            &mut self.terminal,
            self.extended_keymap,
            &store_rx,
            &self.event_tx,
            &self.action_tx,
        )
        .unwrap();
        while self.is_running {
            store_presenter.update_data_streams();
            if let Some(input) = store_presenter.handle_user_input() {
                match input {
                    UserInput::Quit => {
                        debug!("Exiting");
                        self.is_running = false;
                    }
                    UserInput::ChangeEnv => {
                        debug!("Change env mode");
                        self.event_tx.send(TUIEvent::RequestEnvChange).unwrap();
                    }
                    _ => {}
                }
            }
            store_presenter.present();
            store_presenter.update_store();
        }
        Ok(())
    }
}

struct StorePresenter<'a, B>
where
    B: Backend,
{
    terminal: &'a mut Terminal<B>,
    extended_keymap: &'a Vec<fn(KeyCode, &Store, &Sender<TUIEvent>)>,
    store_rx: &'a Receiver<Store>,
    event_tx: &'a Sender<TUIEvent>,
    action_tx: &'a Sender<TUIAction>,
    store: Store,
    now: SystemTime,
    thread_mngt: ThreadManage,
}

impl<'a, B: Backend> StorePresenter<'a, B> {
    fn init(
        terminal: &'a mut Terminal<B>,
        extended_keymap: &'a Vec<fn(KeyCode, &Store, &Sender<TUIEvent>)>,
        store_rx: &'a Receiver<Store>,
        event_tx: &'a Sender<TUIEvent>,
        action_tx: &'a Sender<TUIAction>,
    ) -> Result<Self, String> {
        if let Ok(updated_store) = store_rx.recv() {
            Ok(StorePresenter {
                terminal,
                extended_keymap,
                store_rx,
                store: updated_store,
                event_tx,
                action_tx,
                now: SystemTime::now(),
                thread_mngt: ThreadManage::new(false, false),
            })
        } else {
            Err("nope".to_string())
        }
    }
    fn present(&mut self) {
        let main_layout = MainLayoutUI::new();
        let mut ui = UI::main(&main_layout);
        let mut widgets: Vec<Box<&dyn RenderWidget>> = vec![];
        widgets.push(Box::new(self.store.header_widget.as_ref().unwrap()));
        if let Some(login_widget) = &self.store.login_widget {
            if let Some(Some(_)) = login_widget.get_data().data.get("logs") {
                widgets.push(Box::new(self.store.login_widget.as_ref().unwrap()));
            } else if self.store.logged_in {
                widgets.push(Box::new(self.store.pods_widget.as_ref().unwrap()));
                widgets.push(Box::new(self.store.logs_widget.as_ref().unwrap()));
            } else if self.store.request_login {
                widgets.push(Box::new(self.store.request_login_widget.as_ref().unwrap()));
            }
        } else {
        }
        ui.add_to_widgets(widgets);
        self.terminal.draw(|f| ui.ui(f)).unwrap();
    }

    fn handle_user_input(&self) -> Option<UserInput> {
        let mut user_input: Option<UserInput> = None;
        if let Ok(true) = event::poll(Duration::from_millis(10)) {
            if let Ok(Event::Key(key)) = event::read() {
                user_input = Self::handle_primary_keys(key.code).or_else(|| {
                    Self::handle_direction_keys(key.code).or_else(|| {
                        if self.store.env_change_possible {
                            match key.code {
                                KeyCode::Char('1') => {
                                    self.event_tx
                                        .send(TUIEvent::EnvChange(KubeEnv::Dev))
                                        .unwrap();
                                }
                                KeyCode::Char('2') => {
                                    self.event_tx
                                        .send(TUIEvent::EnvChange(KubeEnv::Test))
                                        .unwrap();
                                }
                                KeyCode::Char('3') => {
                                    self.event_tx
                                        .send(TUIEvent::EnvChange(KubeEnv::Prod))
                                        .unwrap();
                                }
                                _ => {}
                            }
                        } else if self.store.request_login {
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
                            for check in self.extended_keymap {
                                check(key.code, &self.store, &self.event_tx)
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

    fn update_store(&mut self) {
        if let Ok(updated_store) = self.store_rx.recv_timeout(Duration::from_millis(200)) {
            self.store = updated_store
        }
    }

    fn update_data_streams(&mut self) {
        let data_streams: Vec<DataStream> = vec![];
        if self.store.logged_in {
            if self.now.elapsed().unwrap().as_millis() % 50 == 0 {
                debug!("trigger periodical");
                (self.store.pods_widget.as_ref().unwrap().get_data().data_stream.init_action)(self.action_tx)
            }
            if !self
                .store
                .logs_widget
                .as_ref()
                .unwrap()
                .get_data()
                .thread_started
            {}
            if !self.thread_mngt.logs_thread_started {
                debug!("initiate logs thread");
                if let Some(widget_data) = &self.store.logs_widget {
                    widget_data.get_data().initiate_thread.unwrap()(self.action_tx);
                }
                self.thread_mngt.logs_thread_started = true;
            }
            // if !self.thread_mngt.pods_thread_started {
            //     debug!("initiate pods thread");
            //     if let Some(widget_data) = &self.store.pods_widget {
            //         widget_data.get_data().initiate_thread.unwrap()(self.action_tx);
            //     }
            //     self.thread_mngt.pods_thread_started = true;
            // }
        }
    }
}

#[derive(Debug, Clone)]
pub enum StreamType {
    Once,
    Periodical,
    LeaveOpen,
}

#[derive(Debug, Clone)]
pub struct DataStream {
    stream_type: StreamType,
    init_action: fn(&Sender<TUIAction>),
}

impl DataStream {
    pub fn new(stream_type: StreamType, init_action: fn(&Sender<TUIAction>)) -> Self {
        DataStream { stream_type, init_action }
    }
}
