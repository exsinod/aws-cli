use std::sync::mpsc::{Receiver, Sender};

use crate::{Store, TUIAction, TUIEvent};

pub fn event_thread(
    event_rx: Receiver<TUIEvent>,
    store_tx: Sender<Store>,
    action_tx: Sender<TUIAction>,
) {
    let mut store = Store::new();
    send_store(store_tx.clone(), store.clone());
    while let Ok(event) = event_rx.recv() {
        match event {
            TUIEvent::Error(error) => {
                store.error = Some(error);
                send_store(store_tx.clone(), store.clone())
            }
            TUIEvent::NeedsLogin => {
                store.error = Some("Require login".to_string());
                action_tx.send(TUIAction::LogIn).unwrap();
                send_store(store_tx.clone(), store.clone())
            }
            TUIEvent::IsLoggedIn => {
                store.error = None;
                store.logged_in = true;
                send_store(store_tx.clone(), store.clone())
            }
            TUIEvent::DisplayLoginCode(code) => {
                store.login_code = Some(code);
                send_store(store_tx.clone(), store.clone())
            }
            TUIEvent::AddLoginLog(log_part) => {
                add_to(&mut store.login_log, log_part);
                send_store(store_tx.clone(), store.clone())
            }
            TUIEvent::AddLog(log_part) => {
                add_to(&mut store.logs, log_part);
                send_store(store_tx.clone(), store.clone())
            }
            TUIEvent::LogThreadStarted => {
                store.log_thread_started = true;
                send_store(store_tx.clone(), store.clone())
            }
        }
    }
}

fn send_store(store_tx: Sender<Store>, store: Store) {
    match store_tx.send(store.clone()) {
        Ok(_) => (),
        Err(err) => println!("{}", err),
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
