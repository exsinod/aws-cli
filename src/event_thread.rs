use std::sync::mpsc::{Receiver, Sender};

use log::debug;

use crate::{Store, TUIAction, TUIEvent};

pub fn event_thread(
    event_rx: Receiver<TUIEvent>,
    store_tx: Sender<Store>,
    action_tx: Sender<TUIAction>,
) {
    let mut store = Store::new();
    send_store(store_tx.clone(), store.clone());
    action_tx.send(TUIAction::CheckConnectivity).unwrap();
    while let Ok(event) = event_rx.recv() {
        debug!("handling event: {:?}", event);
        let store_tx_clone = store_tx.clone();
        match event {
            TUIEvent::Error(error) => match error {
                crate::TUIError::VPN => {
                    store.error = Some("Uhm... VPN on ?".to_string());
                    send_store(store_tx_clone.clone(), store.clone());
                }
                crate::TUIError::KEY(error) | crate::TUIError::API(error) => {
                    store.error = Some(error);
                    send_store(store_tx_clone.clone(), store.clone());
                }
            },
            TUIEvent::ClearError => {
                store.error = None;
                send_store(store_tx_clone.clone(), store.clone());
            }
            TUIEvent::CheckConnectivity => {
                store.login_request = false;
                send_store(store_tx.clone(), store.clone());
                action_tx.send(TUIAction::CheckConnectivity).unwrap();
            }
            TUIEvent::RequestLogin => {
                store.login_request = true;
                send_store(store_tx.clone(), store.clone())
            }
            TUIEvent::RequestLoginInput(input) => {
                if input == "1" {
                    action_tx.send(TUIAction::GetLogs).unwrap();
                } else if input == "2" {
                    action_tx.send(TUIAction::LogIn).unwrap();
                } else {
                    debug!("input was {:?}", input);
                }
                send_store(store_tx.clone(), store.clone())
            }
            TUIEvent::NeedsLogin => {
                action_tx.send(TUIAction::LogIn).unwrap();
            }
            TUIEvent::IsLoggedIn => {
                store.logged_in = true;
                send_store(store_tx.clone(), store.clone());
                action_tx.send(TUIAction::CheckConnectivity).unwrap();
            }
            TUIEvent::IsConnected => {
                store.logged_in = true;
                send_store(store_tx.clone(), store.clone());
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
            TUIEvent::LogThreadStopped => {
                store.log_thread_started = false;
                send_store(store_tx.clone(), store.clone())
            }
            TUIEvent::AddPods(pods) => {
                store.pods = Some(pods);
                send_store(store_tx.clone(), store.clone())
            }
            _ => {}
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
