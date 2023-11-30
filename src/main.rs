mod action_handler;
mod app;
mod structs;
pub mod truncator;
mod ui;
mod widget_data_store;
mod widgets;
use app::App;
use crossterm::{
    event::{DisableMouseCapture, KeyCode},
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
use structs::{KubeEnv, Store, TUIAction, TUIEvent};
use truncator::TopTruncator;
use widget_data_store::WidgetDataStore;
use widgets::{
    create_header_widget_data, create_login_widget_data, create_logs_widget_data,
    create_pods_widget_data, create_tail_widget_data,
};

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

    // widgets
    let header_widget_data = create_header_widget_data();
    let login_widget_data = create_login_widget_data();
    let logs_widget_data = create_logs_widget_data();
    let pods_widget_data = create_pods_widget_data();
    let tail_widget_data = create_tail_widget_data();

    // store
    let mut store = Store::new(
        header_widget_data.get_widget(),
        login_widget_data.get_widget(),
        logs_widget_data.get_widget(),
        pods_widget_data.get_widget(),
        tail_widget_data.get_widget(),
    );

    // truncator
    let truncator = Box::new(TopTruncator::new(50));

    // clone to move in to action thread
    let action_tx_clone = action_tx.clone();

    // widget data store
    thread::spawn(move || {
        let mut widget_data_store = WidgetDataStore::new(
            event_rx,
            &mut store,
            store_tx.clone(),
            action_tx_clone,
            truncator,
        );

        let widget_event_handlers = vec![
            login_widget_data.get_event_handler(),
            logs_widget_data.get_event_handler(),
            pods_widget_data.get_event_handler(),
            tail_widget_data.get_event_handler(),
        ];
        widget_data_store.start(widget_event_handlers)
    });

    // clone to move in to action thread
    let event_tx_clone = event_tx.clone();

    // action thread
    thread::spawn(move || {
        action_handler::start(event_tx_clone, action_rx);
    });

    // init state
    event_tx.send(TUIEvent::EnvChange(KubeEnv::Dev)).unwrap();

    // package the extended keymaps in a Vec
    let mut extended_keymap: Vec<fn(KeyCode, &Store, &Sender<TUIEvent>)> = vec![];
    extended_keymap.push(header_widget_data.get_keymap());

    // create app and run it
    let res = App::new(&mut terminal, event_tx, action_tx, &extended_keymap).run_app(store_rx);

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
