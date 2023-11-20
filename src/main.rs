mod action_thread;
mod event_thread;
mod ui;
mod widgets;
use crossterm::{
    event::{self, DisableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::{debug, trace, LevelFilter};
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    Config,
};
use ratatui::{
    prelude::{Backend, CrosstermBackend},
    Terminal,
};
use ui::UI;
use widgets::{CliWidget, CliWidgetId};

use std::{
    error::Error,
    io,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
    vec,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Store {
    pub error: Option<String>,
    pub login_request: bool,
    pub logged_in: bool,
    pub login_code: Option<String>,
    pub login_log: Option<String>,
    pub logs: Option<String>,
    pub log_thread_started: bool,
    pub pods: Option<String>,
}

impl Store {
    fn new() -> Store {
        Store {
            error: None,
            login_request: false,
            logged_in: false,
            login_code: None,
            login_log: None,
            logs: None,
            log_thread_started: false,
            pods: None,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum TUIEvent {
    Error(TUIError),
    CheckConnectivity,
    ClearError,
    RequestLogin,
    RequestLoginInput(String),
    Input(String),
    NeedsLogin,
    DisplayLoginCode(String),
    IsLoggedIn,
    IsConnected,
    AddLoginLog(String),
    LogThreadStarted,
    LogThreadStopped,
    AddLog(String),
    AddPods(String),
}

#[derive(Debug, PartialEq)]
pub enum TUIError {
    VPN,
    KEY(String),
    API(String),
}

#[derive(Debug, PartialEq)]
pub enum TUIAction {
    CheckConnectivity,
    LogIn,
    GetLogs,
    GetPods,
}

enum UserInput {
    Quit,
    Direction(Direction),
}

#[derive(Clone)]
enum Direction {
    Left,
    Right,
    Up,
    Down,
}

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    init_logging()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let (action_tx, action_rx): (Sender<TUIAction>, Receiver<TUIAction>) = mpsc::channel();
    let (store_tx, store_rx): (Sender<Store>, Receiver<Store>) = mpsc::channel();

    // clone to move in to event thread
    let action_tx_clone = action_tx.clone();
    let event_tx_clone = event_tx.clone();

    // event thread
    thread::spawn(move || {
        event_thread::event_thread(event_rx, store_tx, action_tx_clone);
    });

    // action thread
    thread::spawn(move || {
        action_thread::action_thread(event_tx_clone.clone(), action_rx);
    });

    // create app and run it
    let res = run_app(&mut terminal, store_rx, event_tx, action_tx);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    store_rx: Receiver<Store>,
    event_tx: Sender<TUIEvent>,
    action_tx: Sender<TUIAction>,
) -> io::Result<()> {
    let should_quit = false;
    do_run_app(should_quit, terminal, store_rx, event_tx, action_tx)
}

fn do_run_app<B: Backend>(
    mut should_quit: bool,
    terminal: &mut Terminal<B>,
    store_rx: Receiver<Store>,
    event_tx: Sender<TUIEvent>,
    action_tx: Sender<TUIAction>,
) -> io::Result<()> {
    let (user_input_tx, user_input_rx): (Sender<UserInput>, Receiver<UserInput>) = mpsc::channel();
    let mut store: Store = Store::new();
    let mut direction: Option<Direction> = None;

    let mut ui = UI::new(store.clone(), action_tx.clone());

    ui.add_widgets(create_widgets());
    ui.store = store.clone();

    while let false = should_quit {
        handle_user_input(user_input_tx.clone(), event_tx.clone(), store.clone());
        if let Ok(input) = user_input_rx.try_recv() {
            match input {
                UserInput::Quit => {
                    should_quit = true;
                }
                UserInput::Direction(dir) => direction = Some(dir),
            }
        }

        if let Some(dir) = &direction {
            ui.ui_transform.direction = Some(dir.clone());
        }

        while let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
            store = updated_store;
        }

        if ui.store != store {
            ui.store = store.clone();
            trace!("updating ui.store {:?}", ui.store);
            terminal.draw(|f| ui.ui(f)).unwrap();
        }
    }
    Ok(())
}

fn create_widgets() -> Vec<CliWidget> {
    let mut login_widget = CliWidget::new(CliWidgetId::Login, "Login".to_string());
    login_widget.data_fn = Some(|f| f.login_log);

    let mut logs_widget = CliWidget::new(CliWidgetId::GetLogs, "Salespoint Logs".to_string());
    logs_widget.data_fn = Some(|f| f.logs);

    let mut pods_widget = CliWidget::new(CliWidgetId::GetPods, "Pods".to_string());
    pods_widget.data_fn = Some(|f| f.pods);

    vec![login_widget, logs_widget, pods_widget]
}

fn handle_user_input(user_input_tx: Sender<UserInput>, event_tx: Sender<TUIEvent>, store: Store) {
    if let Ok(true) = event::poll(Duration::from_millis(10)) {
        if let Ok(Event::Key(mut key)) = event::read() {
            handle_primary_keys(key.code, user_input_tx.clone());
            key.code = handle_direction_keys(key.code, user_input_tx);
            if store.login_request {
                match key.code {
                    KeyCode::Char('1') => {
                        event_tx.send(TUIEvent::ClearError).unwrap();
                        event_tx.send(TUIEvent::CheckConnectivity).unwrap();
                    }
                    KeyCode::Char('2') => event_tx.send(TUIEvent::NeedsLogin).unwrap(),
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Null => {}
                    _ => event_tx
                        .send(TUIEvent::Error(TUIError::KEY(
                            "Unrecognised key: ".to_string()
                                + &format!("{:?}", key.code).to_string()
                                + " Press q to quit",
                        )))
                        .unwrap(),
                };
            }
        } else {
            debug!("error from input thread");
        }
    }
}

fn handle_primary_keys(keycode: KeyCode, user_input_tx: Sender<UserInput>) -> KeyCode {
    return if let KeyCode::Char('q') = keycode {
        user_input_tx.send(UserInput::Quit).unwrap();
        KeyCode::Null
    } else {
        keycode
    };
}

fn handle_direction_keys(keycode: KeyCode, user_input_tx: Sender<UserInput>) -> KeyCode {
    if keycode == KeyCode::Char('h') {
        user_input_tx
            .send(UserInput::Direction(Direction::Left))
            .unwrap();
        KeyCode::Null
    } else if keycode == KeyCode::Char('j') {
        user_input_tx
            .send(UserInput::Direction(Direction::Down))
            .unwrap();
        KeyCode::Null
    } else if keycode == KeyCode::Char('k') {
        user_input_tx
            .send(UserInput::Direction(Direction::Up))
            .unwrap();
        KeyCode::Null
    } else if keycode == KeyCode::Char('l') {
        user_input_tx
            .send(UserInput::Direction(Direction::Right))
            .unwrap();
        KeyCode::Null
    } else {
        keycode
    }
}

pub fn init_logging() -> Result<(), String> {
    let stdout = FileAppender::builder()
        .append(false)
        .build("./logs.txt")
        .unwrap();
    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .build(Root::builder().appender("stdout").build(LevelFilter::Debug))
        .unwrap();
    debug!("Logging init");
    if let Ok(_) = log4rs::init_config(config) {
        Ok(())
    } else {
        Err("error".to_string())
    }
}
