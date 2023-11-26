use std::{
    collections::HashMap,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use log::{debug, trace};

use crate::{
    structs::{CliWidgetData, KubeEnv, TUIError, DEV},
    widgets::{BodyWidget, CliWidget, HeaderWidget, RenderWidget, create_header_widget_data, create_login_widget_data, create_logs_widget_data, create_pods_widget_data},
    CliWidgetId, Store, TUIAction, TUIEvent,
};

pub struct WidgetDataStore {
    event_rx: Receiver<TUIEvent>,
    store_tx: Sender<Store>,
    action_tx: Sender<TUIAction>,
}

impl WidgetDataStore {
    pub fn new(
        event_rx: Receiver<TUIEvent>,
        store_tx: Sender<Store>,
        action_tx: Sender<TUIAction>,
    ) -> Self {
        WidgetDataStore {
            event_rx,
            store_tx,
            action_tx,
        }
    }

    pub fn start(
        &self,
        header_widget_data: HeaderWidget,
        login_widget_data: BodyWidget,
        logs_widget_data: BodyWidget,
        pods_widget_data: BodyWidget,
    ) {
        let store = &mut Store::new(
            header_widget_data,
            login_widget_data,
            logs_widget_data,
            pods_widget_data,
        );
        Self::send_store(self.store_tx.clone(), store);
        while let Ok(event) = self.event_rx.recv() {
            trace!("handling event: {:?}", event);
            let store_tx_clone = self.store_tx.clone();
            let action_tx_clone = self.action_tx.clone();
            match event {
                TUIEvent::RequestEnvChange => {
                    store.env_change_possible = true;
                    Self::send_store(store_tx_clone.clone(), store)
                }
                TUIEvent::EnvChange(env) => {
                    action_tx_clone
                        .send(TUIAction::ChangeEnv(env.clone()))
                        .unwrap();
                    store.env_change_possible = false;
                    store
                        .header_widget
                        .as_mut()
                        .unwrap()
                        .set_text_data("kube_info".to_string(), format!("{:?}", env).to_string());
                    Self::send_store(store_tx_clone.clone(), store)
                }
                TUIEvent::Error(error) => match error {
                    TUIError::VPN => {
                        store
                            .header_widget
                            .as_mut()
                            .unwrap()
                            .set_text_data("error".to_string(), "Uhm... VPN on ?".to_string());
                        Self::send_store(store_tx_clone.clone(), store);
                    }
                    TUIError::KEY(error) | TUIError::API(error) => {
                        store
                            .header_widget
                            .as_mut()
                            .unwrap()
                            .set_text_data("error".to_string(), error);
                        Self::send_store(store_tx_clone.clone(), store);
                    }
                },
                TUIEvent::ClearError => {
                    if let Some(header_widget) = store.header_widget.as_mut() {
                        header_widget.clear_text_data("error".to_string());
                        Self::send_store(store_tx_clone.clone(), store);
                    }
                    Self::send_store(store_tx_clone.clone(), store);
                }
                TUIEvent::CheckConnectivity => {
                    store.request_login = false;
                    Self::send_store(self.store_tx.clone(), store);
                    action_tx_clone.send(TUIAction::CheckConnectivity).unwrap();
                }
                TUIEvent::RequestLoginStart => {
                    store.request_login = true;
                    Self::send_store(self.store_tx.clone(), store)
                }
                TUIEvent::RequestLoginStop => {
                    store.request_login = false;
                    Self::send_store(self.store_tx.clone(), store)
                }
                TUIEvent::RequestLoginInput(input) => {
                    if input == "1" {
                        self.action_tx.send(TUIAction::GetLogs).unwrap();
                    } else if input == "2" {
                        self.action_tx.send(TUIAction::LogIn).unwrap();
                    } else {
                        debug!("input was {:?}", input);
                    }
                    Self::send_store(self.store_tx.clone(), store)
                }
                TUIEvent::NeedsLogin => {
                    self.action_tx.send(TUIAction::LogIn).unwrap();
                }
                TUIEvent::IsLoggedIn => {
                    store.logged_in = true;
                    if let Some(login_widget) = store.login_widget.as_mut() {
                        login_widget.clear_text_data("logs".to_string());
                    }
                    Self::send_store(self.store_tx.clone(), store);
                    self.action_tx.send(TUIAction::CheckConnectivity).unwrap();
                }
                TUIEvent::IsConnected => {
                    store.logged_in = true;
                    store
                        .header_widget
                        .as_mut()
                        .unwrap()
                        .set_text_data("login_info".to_string(), "LOGGED IN".to_string());
                    Self::send_store(self.store_tx.clone(), store);
                }
                TUIEvent::DisplayLoginCode(code) => {
                    store.login_code = Some(code);
                    Self::send_store(self.store_tx.clone(), store)
                }
                TUIEvent::AddLoginLog(log_part) => {
                    Self::add_to_widget_data(store.login_widget.as_mut().unwrap(), log_part);
                    Self::send_store(self.store_tx.clone(), store)
                }
                TUIEvent::AddLog(log_part) => {
                    Self::add_to_widget_data(store.logs_widget.as_mut().unwrap(), log_part);
                    Self::send_store(self.store_tx.clone(), store)
                }
                TUIEvent::AddPods(pods) => {
                    store
                        .pods_widget
                        .as_mut()
                        .unwrap()
                        .set_text_data("logs".to_string(), pods);
                    Self::send_store(self.store_tx.clone(), store)
                }
                TUIEvent::LogThreadStarted(id) => match id {
                    CliWidgetId::GetLogs => {
                        if let Some(widget_data) = store.logs_widget.as_mut() {
                            widget_data.get_data().initiate_thread.unwrap()(action_tx_clone);
                            widget_data.set_thread_started(true);
                            Self::send_store(self.store_tx.clone(), store)
                        }
                    }
                    CliWidgetId::GetPods => {
                        if let Some(widget_data) = store.pods_widget.as_mut() {
                            widget_data
                                .get_data()
                                // .get_data("logs".to_string())
                                .initiate_thread
                                .unwrap()(action_tx_clone);
                            widget_data.set_thread_started(true);
                            Self::send_store(self.store_tx.clone(), store)
                        }
                    }
                    _ => {}
                },
                TUIEvent::LogThreadStopped(id) => {
                    store.logs_widget.as_mut().and_then(|d| {
                        d.get_data().initiate_thread.unwrap()(action_tx_clone);
                        Some(d.get_data().thread_started = false)
                    });
                    Self::send_store(self.store_tx.clone(), store)
                }
                _ => {}
            }
        }
    }

    fn send_store(store_tx: Sender<Store>, store: &mut Store) {
        match store_tx.send(store.clone()) {
            Ok(_) => (),
            Err(err) => println!("{}", err),
        }
    }

    fn add_to_widget_data<'a>(widget: &mut BodyWidget, text: String) -> &mut BodyWidget {
        if let Some(Some(existing_test)) = &mut widget.get_data().data.get_mut("logs") {
            existing_test.push_str(text.as_str());
            widget.set_text_data("logs".to_string(), existing_test.to_string());
        } else {
            widget.set_text_data("logs".to_string(), text);
        }
        widget
    }
}

#[test]
fn test_error_events() {
    crate::init_logging().unwrap();
    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let (action_tx, action_rx): (Sender<TUIAction>, Receiver<TUIAction>) = mpsc::channel();
    let (store_tx, store_rx): (Sender<Store>, Receiver<Store>) = mpsc::channel();

    let widget_data_store = WidgetDataStore::new(event_rx, store_tx, action_tx);
    thread::spawn(move || {
        widget_data_store.start(
            create_header_widget_data(),
            create_login_widget_data(),
            create_logs_widget_data(),
            create_pods_widget_data(),
        )
    });
    if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
        assert!(
            updated_store
                .clone()
                .header_widget
                .unwrap()
                .get_data().data
                .get("error")
                == None,
            "store was: {:?}",
            updated_store
        )
    }

    event_tx.send(TUIEvent::Error(TUIError::VPN)).unwrap();

    if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
        assert!(
            updated_store
                .clone()
                .header_widget
                .unwrap()
                .get_data().data
                .get("error")
                == Some(Some("Uhm... VPN on ?".to_string())).as_ref(),
            "store was: {:?}",
            updated_store
        )
    }

    if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
        assert!(
            updated_store
                .clone()
                .header_widget
                .unwrap()
                .get_data().data
                .get("error")
                == None,
            "store was: {:?}",
            updated_store
        )
    }

    event_tx
        .send(TUIEvent::Error(TUIError::API("this errored".to_string())))
        .unwrap();

    if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
        assert!(
            updated_store
                .clone()
                .header_widget
                .unwrap()
                .get_data().data
                .get("error")
                == Some(Some("this errored".to_string())).as_ref(),
            "store was: {:?}",
            updated_store
        )
    }
}

#[test]
fn test_check_connectivity_event() {
    crate::init_logging().unwrap();
    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let (action_tx, action_rx): (Sender<TUIAction>, Receiver<TUIAction>) = mpsc::channel();
    let (store_tx, store_rx): (Sender<Store>, Receiver<Store>) = mpsc::channel();

    let widget_data_store = WidgetDataStore::new(event_rx, store_tx, action_tx);
    thread::spawn(move || {
        widget_data_store.start(
            create_header_widget_data(),
            create_login_widget_data(),
            create_logs_widget_data(),
            create_pods_widget_data(),
        )
    });
    if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
        assert!(
            !updated_store.request_login,
            "store was: {:?}",
            updated_store
        )
    }

    event_tx.send(TUIEvent::CheckConnectivity).unwrap();

    let mut actions = vec![];
    let check_actions = vec![TUIAction::CheckConnectivity];
    while actions != check_actions {
        if let Ok(action) = action_rx.recv_timeout(Duration::from_millis(10)) {
            actions.push(action);
        }
    }

    assert!(actions == check_actions, "was {:?}", actions);
}

#[test]
fn test_login_event() {
    crate::init_logging().unwrap();
    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let (action_tx, action_rx): (Sender<TUIAction>, Receiver<TUIAction>) = mpsc::channel();
    let (store_tx, store_rx): (Sender<Store>, Receiver<Store>) = mpsc::channel();

    let widget_data_store = WidgetDataStore::new(event_rx, store_tx, action_tx);
    thread::spawn(move || {
        widget_data_store.start(
            create_header_widget_data(),
            create_login_widget_data(),
            create_logs_widget_data(),
            create_pods_widget_data(),
        )
    });
    if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
        assert!(!updated_store.logged_in, "store was: {:?}", updated_store)
    }

    event_tx.send(TUIEvent::NeedsLogin).unwrap();

    let mut actions = vec![];
    let check_actions = vec![TUIAction::LogIn];
    while actions != check_actions {
        if let Ok(action) = action_rx.recv_timeout(Duration::from_millis(10)) {
            actions.push(action);
        }
    }

    assert!(actions == check_actions, "was {:?}", actions);
}

#[test]
fn test_add_log_event() {
    crate::init_logging().unwrap();
    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let (action_tx, _): (Sender<TUIAction>, Receiver<TUIAction>) = mpsc::channel();
    let (store_tx, store_rx): (Sender<Store>, Receiver<Store>) = mpsc::channel();

    let widget_data_store = WidgetDataStore::new(event_rx, store_tx, action_tx);
    thread::spawn(move || {
        widget_data_store.start(
            create_header_widget_data(),
            create_login_widget_data(),
            create_logs_widget_data(),
            create_pods_widget_data(),
        )
    });
    if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
        // updated_store.body_widget.unwrap().get_data("logs".to_string()).text == Some("this errored".to_string()),

        assert!(
            updated_store
                .clone()
                .logs_widget
                .unwrap()
                .get_data().data
                .get("logs")
                == None,
            "store was: {:?}",
            updated_store
        )
    }

    event_tx
        .send(TUIEvent::AddLog("this is a new line\n".to_string()))
        .unwrap();

    if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
        assert!(
            updated_store
                .clone()
                .logs_widget
                .unwrap()
                .get_data().data
                .get("logs")
                == Some(Some("this is a new line\n".to_string())).as_ref(),
            "store was: {:?}",
            updated_store
        )
    }

    event_tx
        .send(TUIEvent::AddLog("and some extra.".to_string()))
        .unwrap();

    if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
        assert!(
            updated_store
                .clone()
                .logs_widget
                .unwrap()
                .get_data().data
                .get("logs")
                == Some(Some("this is a new line\nand some extra.".to_string())).as_ref(),
            "store was: {:?}",
            updated_store
        )
    }
}
