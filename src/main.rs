mod action_handler;
mod app;
mod structs;
mod ui;
mod widget_data_store;
mod widgets;
use app::App;
use crossterm::{
    event::DisableMouseCapture,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::{debug, LevelFilter};
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Root},
    Config,
};
use ratatui::{layout::Direction, prelude::CrosstermBackend, Terminal};
use structs::{Store, TUIAction, TUIEvent};
use widget_data_store::{
    create_header_widget_data, create_login_widget_data, create_logs_widget_data, WidgetDataStore,
};
use widgets::CliWidgetId;

use std::{
    error::Error,
    io,
    sync::{
        mpsc::{self, Receiver, Sender},
        Once,
    },
    thread,
};

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    init_logging()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create all needed channels
    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let (action_tx, action_rx): (Sender<TUIAction>, Receiver<TUIAction>) = mpsc::channel();
    let (store_tx, store_rx): (Sender<Store>, Receiver<Store>) = mpsc::channel();

    // clone to move in to event thread
    let action_tx_clone = action_tx.clone();
    let event_tx_clone = event_tx.clone();

    // event thread
    thread::spawn(move || {
        let widget_data_store =
            WidgetDataStore::new(event_rx, store_tx.clone(), action_tx_clone.clone());
        widget_data_store.start(
            create_header_widget_data(),
            create_login_widget_data(),
            create_logs_widget_data(),
        )
    });

    // action thread
    thread::spawn(move || {
        action_handler::start(event_tx_clone.clone(), action_rx);
    });

    // create app and run it
    let res = App::new(&mut terminal, store_rx, event_tx, action_tx).run_app();

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

static INIT_LOGGING: Once = Once::new();

pub fn init_logging() -> io::Result<()> {
    INIT_LOGGING.call_once(|| {
        let stdout = FileAppender::builder()
            .append(false)
            .build("./logs.txt")
            .unwrap();
        let config = Config::builder()
            .appender(Appender::builder().build("stdout", Box::new(stdout)))
            .build(Root::builder().appender("stdout").build(LevelFilter::Debug))
            .unwrap();
        debug!("Logging init");
        if let Ok(_) = log4rs::init_config(config) {}
    });
    Ok(())
}
