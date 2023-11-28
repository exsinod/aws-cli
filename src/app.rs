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
    structs::{Direction2, KubeEnv, Store, TUIAction, TUIError, TUIEvent, UserInput},
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
    store: Option<Store>,
    store_rx: Receiver<Store>,
    event_tx: Sender<TUIEvent>,
    action_tx: Sender<TUIAction>,
    thread_mngt: Option<ThreadManage>,
    extended_keymap: &'a Vec<fn(KeyCode)>,
}

impl<'a, B: Backend> App<'a, B> {
    pub fn new(
        terminal: &'a mut Terminal<B>,
        store_rx: Receiver<Store>,
        event_tx: Sender<TUIEvent>,
        action_tx: Sender<TUIAction>,
        extended_keymap: &'a Vec<fn(KeyCode)>,
    ) -> Self {
        App {
            terminal,
            store: None,
            store_rx,
            event_tx,
            action_tx,
            thread_mngt: None,
            extended_keymap,
        }
    }

    pub fn run_app(&mut self) -> io::Result<()> {
        let mut should_quit = false;
        let logs_thread_started = false;
        let login_logs_thread_started = false;
        let tail_thread_started = false;
        self.thread_mngt = Some(ThreadManage::new(
            logs_thread_started,
            login_logs_thread_started,
            tail_thread_started,
        ));

        while let false = should_quit {
            if let Some(store) = self.store.as_ref() {
                let user_input = self.handle_user_input(store, &self.extended_keymap);
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
            while let Ok(updated_store) = self.store_rx.recv_timeout(Duration::from_millis(20)) {
                trace!("got store {:?}", updated_store);
                self.store = Some(updated_store.clone());
                let mut ui = UI::main(&MainLayoutUI::new());
                let mut widgets: Vec<Box<&dyn RenderWidget>> = vec![];
                widgets.push(Box::new(updated_store.header_widget.as_ref().unwrap()));
                self.initiate_threads(&updated_store);
                if updated_store
                    .clone()
                    .login_widget
                    .unwrap()
                    .get_data_mut()
                    .data
                    .get("logs")
                    .is_some()
                {
                    widgets.push(Box::new(updated_store.login_widget.as_ref().unwrap()));
                } else if updated_store.logged_in {
                    widgets.push(Box::new(updated_store.pods_widget.as_ref().unwrap()));
                    widgets.push(Box::new(updated_store.logs_widget.as_ref().unwrap()));
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
                ui.add_to_widgets(widgets);
                self.terminal.draw(|f| ui.ui(f)).unwrap();
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

    fn initiate_threads(&mut self, updated_store: &Store) {
        if updated_store.logged_in {
            if !self.thread_mngt.as_mut().unwrap().logs_thread_started {
                debug!("initiate logs thread");
                if let Some(widget_data) = &self.store.as_ref().unwrap().logs_widget {
                    widget_data.get_data().initiate_thread.unwrap()(self.action_tx.clone());
                }
                self.thread_mngt.as_mut().unwrap().logs_thread_started = true;
            }
            if !self.thread_mngt.as_mut().unwrap().tail_thread_started {
                debug!("initiate tail thread");
                if let Some(widget_data) = &self.store.as_ref().unwrap().tail_widget {
                    widget_data.get_data().initiate_thread.unwrap()(self.action_tx.clone());
                }
                self.thread_mngt.as_mut().unwrap().tail_thread_started = true;
            }
            if !self.thread_mngt.as_mut().unwrap().pods_thread_started {
                debug!("initiate pods thread");
                if let Some(widget_data) = &self.store.as_ref().unwrap().pods_widget {
                    widget_data.get_data().initiate_thread.unwrap()(self.action_tx.clone());
                }
                self.thread_mngt.as_mut().unwrap().pods_thread_started = true;
            }
        }
    }

    fn handle_user_input(
        &self,
        store: &Store,
        extended_keymap: &Vec<fn(KeyCode)>,
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
