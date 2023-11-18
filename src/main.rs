mod action_thread;
mod event_thread;
mod ui;
use crossterm::{
    event::{self, DisableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::{LevelFilter, debug};
use log4rs::{append::file::FileAppender, Config, config::{Appender, Root}};
use ratatui::{
    prelude::{Backend, CrosstermBackend},
    Terminal,
};

use std::{
    error::Error,
    io,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

#[derive(Clone)]
pub struct Store {
    pub error: Option<String>,
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
    Error(String),
    NeedsLogin,
    DisplayLoginCode(String),
    IsLoggedIn,
    AddLoginLog(String),
    LogThreadStarted,
    LogThreadStopped,
    AddLog(String),
    AddPods(String),
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
    Pauze,
}

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let stdout = FileAppender::builder().append(false).build("./logs.txt").unwrap();
    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .build(Root::builder().appender("stdout").build(LevelFilter::Debug))
        .unwrap();
    let _handle = log4rs::init_config(config).unwrap();
    debug!("Logging init");

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

    // clone to move in to action thread
    let action_tx_clone = action_tx.clone();

    // action thread
    thread::spawn(move || {
        action_thread::action_thread(event_tx_clone.clone(), action_rx, action_tx_clone.clone());
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
    let (user_input_tx, user_input_rx): (Sender<UserInput>, Receiver<UserInput>) = mpsc::channel();
    thread::spawn(move || {
        while let Ok(Event::Key(key)) = event::read() {
            match key.code {
                KeyCode::Char('q') => user_input_tx.send(UserInput::Quit).unwrap(),
                _ => event_tx
                    .send(TUIEvent::Error(
                        "Unrecognised key: ".to_string() + &format!("{:?}", key.code).to_string() + " Press q to quit",
                    ))
                    .unwrap(),
            }
        }
    });
    let mut should_quit = false;
    while let false = should_quit {
        if let Ok(input) = user_input_rx.try_recv() {
            if let UserInput::Quit = input {
                should_quit = true;
            }
        }
        while let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(1)) {
            terminal
                .draw(|f| ui::ui(f, updated_store, action_tx.clone()))
                .unwrap();
        }
    }
    Ok(())
}
