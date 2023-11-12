mod action_thread;
mod event_thread;
mod ui;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
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
}

impl Store {
    fn new() -> Store {
        Store {
            error: None,
            logged_in: false,
            login_code: None,
            login_log: None,
            logs: None,
        }
    }
}

pub enum TUIEvent {
    NeedsLogin,
    DisplayLoginCode(String),
    IsLoggedIn,
    AddLoginLog(String),
    AddLog(String),
}

pub enum TUIAction {
    LogIn,
    GetLogs,
}

enum UserInput {
    Quit,
    Pauze,
}

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let (action_tx, action_rx): (Sender<TUIAction>, Receiver<TUIAction>) = mpsc::channel();
    let (store_tx, store_rx): (Sender<Store>, Receiver<Store>) = mpsc::channel();

    // clone to move in to event thread
    let action_tx_clone = action_tx.clone();

    // event thread
    thread::spawn(move || {
        event_thread::event_thread(event_rx, store_tx, action_tx_clone);
    });

    // action thread
    thread::spawn(move || {
        action_thread::action_thread(event_tx, action_rx, action_tx);
    });

    // create app and run it
    let res = run_app(&mut terminal, &store_rx);

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

fn run_app<B: Backend>(terminal: &mut Terminal<B>, store_rx: &Receiver<Store>) -> io::Result<()> {
    let (user_input_tx, user_input_rx): (Sender<UserInput>, Receiver<UserInput>) = mpsc::channel();
    thread::spawn(move || {
        while let Event::Key(key) = event::read().unwrap() {
            match key.code {
                KeyCode::Char('q') => user_input_tx.send(UserInput::Quit).unwrap(),
                _ => {}
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
            terminal.draw(|f| ui::ui(f, updated_store)).unwrap();
        }
    }
    Ok(())
}
