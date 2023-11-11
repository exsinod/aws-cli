use std::{
    sync::mpsc::{Receiver, Sender},
    thread,
    time::Duration,
};

use crate::{Store, TUIEvent};

pub fn event_thread(event_rx: Receiver<TUIEvent>, store_tx: Sender<Store>) {
    let mut store = Store::new();
    loop {
        match event_rx.try_recv() {
            Ok(event) => match event {
                TUIEvent::IsLoggedIn => {
                    store.logged_in = true;
                }
                TUIEvent::DisplayLoginCode(code) => {
                    store.login_code = Some(code);
                }
                TUIEvent::AddLoginLog(log_part) => {
                    add_to(&mut store.login_log, log_part);
                }
                TUIEvent::AddLog(log_part) => {
                    add_to(&mut store.logs, log_part);
                }
            },
            Err(_) => {}
        }
        match store_tx.send(store.clone()) {
            Ok(_) => (),
            Err(err) => println!("{}", err),
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn add_to(logs: &mut Option<String>, log_part: String) {
    if let Some(log) = logs {
        log.push_str(log_part.as_str());
        *logs = Some(log.to_string());
    } else {
        *logs = Some(log_part);
    }
}
