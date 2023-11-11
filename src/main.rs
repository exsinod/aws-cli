mod action_thread;
mod event_thread;
mod ui;
use crossterm::{
    event::{self, poll, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
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
    pub logged_in: bool,
    pub login_code: Option<String>,
    pub login_log: Option<String>,
    pub logs: Option<String>,
}

impl Store {
    fn new() -> Store {
        Store {
            logged_in: false,
            login_code: None,
            login_log: None,
            logs: None,
        }
    }
}

pub enum TUIEvent {
    DisplayLoginCode(String),
    IsLoggedIn,
    AddLoginLog(String),
    AddLog(String),
}

pub enum TUIAction {
    LogIn,
    GetLogs,
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

    // event thread
    thread::spawn(move || {
        event_thread::event_thread(event_rx, store_tx);
    });

    // clone to send action after moving action_tx in thread
    let action_tx_clone = action_tx.clone();

    // action thread
    thread::spawn(move || {
        action_thread::action_thread(event_tx, action_rx, action_tx);
    });

    // initialize store
    action_tx_clone.send(TUIAction::LogIn).unwrap();

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
    loop {
        terminal.draw(|f| ui::ui(f, store_rx))?;

        if poll(Duration::from_millis(5))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break Ok(()),
                    _ => {}
                }
            }
        }
    }
}
