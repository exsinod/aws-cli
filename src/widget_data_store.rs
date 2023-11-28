use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use log::{debug, error, trace};

use crate::{
    structs::TUIError, truncator::Truncatorix, widgets::RenderWidget, CliWidgetId, Store,
    TUIAction, TUIEvent,
};

pub struct WidgetDataStore<'a> {
    event_rx: Receiver<TUIEvent>,
    store: &'a mut Store,
    store_tx: Sender<Store>,
    action_tx: Sender<TUIAction>,
    truncator: Box<dyn Truncatorix>,
}

impl<'a> WidgetDataStore<'a> {
    pub fn new(
        event_rx: Receiver<TUIEvent>,
        store: &'a mut Store,
        store_tx: Sender<Store>,
        action_tx: Sender<TUIAction>,
        truncator: Box<dyn Truncatorix>,
    ) -> Self {
        WidgetDataStore {
            event_rx,
            store,
            store_tx,
            action_tx,
            truncator,
        }
    }

    fn start_truncator(&mut self) {
        self.truncator.start();
    }

    pub fn start(
        &mut self,
        login_event_handler: fn(&TUIEvent, &mut Store),
        logs_event_handler: fn(&TUIEvent, &mut Store),
        pods_event_handler: fn(&TUIEvent, &mut Store),
        tail_event_handler: fn(&TUIEvent, &mut Store),
    ) {
        self.start_truncator();
        self.send();
        while let Ok(event) = self.event_rx.recv() {
            trace!("handling event: {:?}", event);
            let action_tx_clone = self.action_tx.clone();
            match event {
                TUIEvent::RequestEnvChange => {
                    self.store.env_change_possible = true;
                    self.send();
                }
                TUIEvent::EnvChange(env) => {
                    action_tx_clone
                        .send(TUIAction::ChangeEnv(env.clone()))
                        .unwrap();
                    self.store.env_change_possible = false;
                    self.store.header_widget.as_mut().unwrap().set_data(
                        "kube_info".to_string(),
                        vec![format!("{:?}", env).to_string()],
                    );
                    self.send();
                }
                TUIEvent::Error(error) => match error {
                    TUIError::VPN => {
                        self.store
                            .header_widget
                            .as_mut()
                            .unwrap()
                            .set_data("error".to_string(), vec!["Uhm... VPN on ?".to_string()]);
                        self.send();
                    }
                    TUIError::KEY(error) | TUIError::API(error) => {
                        self.store
                            .header_widget
                            .as_mut()
                            .unwrap()
                            .set_data("error".to_string(), vec![error]);
                        self.send();
                    }
                },
                TUIEvent::ClearError => {
                    if let Some(header_widget) = self.store.header_widget.as_mut() {
                        header_widget.clear_text_data("error".to_string());
                        self.send();
                    }
                }
                TUIEvent::CheckConnectivity => {
                    self.store.request_login = false;
                    action_tx_clone.send(TUIAction::CheckConnectivity).unwrap();
                    self.send();
                }
                TUIEvent::RequestLoginStart => {
                    self.store.request_login = true;
                    self.send();
                }
                TUIEvent::RequestLoginStop => {
                    self.store.request_login = false;
                    self.send();
                }
                TUIEvent::RequestLoginInput(input) => {
                    if input == "1" {
                        self.action_tx.send(TUIAction::GetLogs).unwrap();
                    } else if input == "2" {
                        self.action_tx.send(TUIAction::LogIn).unwrap();
                    } else {
                        debug!("input was {:?}", input);
                    }
                    self.send();
                }
                TUIEvent::NeedsLogin => {
                    self.action_tx.send(TUIAction::LogIn).unwrap();
                }
                TUIEvent::IsLoggedIn => {
                    self.store.logged_in = true;
                    if let Some(login_widget) = self.store.login_widget.as_mut() {
                        login_widget.clear_text_data("logs".to_string());
                    }
                    self.action_tx.send(TUIAction::CheckConnectivity).unwrap();
                    self.send();
                }
                TUIEvent::IsConnected => {
                    self.store.logged_in = true;
                    self.store
                        .header_widget
                        .as_mut()
                        .unwrap()
                        .set_data("login_info".to_string(), vec!["LOGGED IN".to_string()]);
                    self.send();
                }
                TUIEvent::DisplayLoginCode(code) => {
                    self.store.login_code = Some(code);
                    self.send();
                }
                TUIEvent::LogThreadStarted(id) => match id {
                    CliWidgetId::GetLogs => {
                        if let Some(widget_data) = self.store.logs_widget.as_mut() {
                            widget_data.get_data().initiate_thread.unwrap()(action_tx_clone);
                            widget_data.set_thread_started(true);
                            self.send();
                        }
                    }
                    CliWidgetId::GetPods => {
                        if let Some(widget_data) = self.store.pods_widget.as_mut() {
                            widget_data.get_data().initiate_thread.unwrap()(action_tx_clone);
                            widget_data.set_thread_started(true);
                            self.send();
                        }
                    }
                    CliWidgetId::Tail => {
                        if let Some(widget_data) = self.store.tail_widget.as_mut() {
                            widget_data.get_data().initiate_thread.unwrap()(action_tx_clone);
                            widget_data.set_thread_started(true);
                            self.send();
                        }
                    }
                    _ => {}
                },
                TUIEvent::LogThreadStopped(id) => {
                    self.store.logs_widget.as_mut().and_then(|d| {
                        d.get_data().initiate_thread.unwrap()(action_tx_clone);
                        Some(d.get_data().thread_started = false)
                    });
                    self.send();
                }
                event => {
                    for f in vec![
                        login_event_handler,
                        logs_event_handler,
                        pods_event_handler,
                        tail_event_handler,
                    ]
                    .as_slice()
                    {
                        f(&event, self.store);
                    }
                    self.send();
                }
            }
            if let Some(()) = self.truncator.poll() {
                self.truncator.truncate(self.store)
            }
        }
    }

    fn send(&self) {
        match self.store_tx.send(self.store.clone()) {
            Ok(_) => trace!("sending store {:?}", self.store.clone()),
            Err(err) => error!("Error sending to store_tx: {}", err),
        }
    }
}
//
// #[test]
// fn test_error_events() {
//     crate::init_logging().unwrap();
//     let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
//     let (action_tx, action_rx): (Sender<TUIAction>, Receiver<TUIAction>) = mpsc::channel();
//     let (store_tx, store_rx): (Sender<Store>, Receiver<Store>) = mpsc::channel();
//
//     thread::spawn(move || {
//         let header_widget_data = crate::widgets::create_header_widget_data();
//         let login_widget_data = crate::widgets::create_login_widget_data();
//         let logs_widget_data = crate::widgets::create_logs_widget_data();
//         let pods_widget_data = crate::widgets::create_pods_widget_data();
//
//         let mut store = Store::new(
//             header_widget_data.get_widget().clone(),
//             login_widget_data.get_widget().clone(),
//             logs_widget_data.get_widget().clone(),
//             pods_widget_data.get_widget().clone(),
//         );
//         let mut widget_data_store = WidgetDataStore::new(
//             event_rx,
//             &mut store,
//             store_tx,
//             action_tx,
//             Box::new(crate::SimpleTruncator::new()),
//         );
//         widget_data_store.start(
//             login_widget_data.get_event_handler(),
//             logs_widget_data.get_event_handler(),
//             pods_widget_data.get_event_handler(),
//         )
//     });
//     if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
//         assert!(
//             updated_store
//                 .clone()
//                 .header_widget
//                 .unwrap()
//                 .get_data()
//                 .data
//                 .get("error")
//                 == None,
//             "store was: {:?}",
//             updated_store
//         )
//     }
//
//     event_tx.send(TUIEvent::Error(TUIError::VPN)).unwrap();
//
//     if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
//         assert!(
//             updated_store
//                 .clone()
//                 .header_widget
//                 .unwrap()
//                 .get_data()
//                 .data
//                 .get("error")
//                 == Some(Some(vec!["Uhm... VPN on ?".to_string()])).as_ref(),
//             "store was: {:?}",
//             updated_store
//         )
//     }
//
//     if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
//         assert!(
//             updated_store
//                 .clone()
//                 .header_widget
//                 .unwrap()
//                 .get_data()
//                 .data
//                 .get("error")
//                 == None,
//             "store was: {:?}",
//             updated_store
//         )
//     }
//
//     event_tx
//         .send(TUIEvent::Error(TUIError::API("this errored".to_string())))
//         .unwrap();
//
//     if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
//         assert!(
//             updated_store
//                 .clone()
//                 .header_widget
//                 .unwrap()
//                 .get_data()
//                 .data
//                 .get("error")
//                 == Some(Some(vec!["this errored".to_string()])).as_ref(),
//             "store was: {:?}",
//             updated_store
//         )
//     }
// }
//
// #[test]
// fn test_check_connectivity_event() {
//     crate::init_logging().unwrap();
//     let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
//     let (action_tx, action_rx): (Sender<TUIAction>, Receiver<TUIAction>) = mpsc::channel();
//     let (store_tx, store_rx): (Sender<Store>, Receiver<Store>) = mpsc::channel();
//
//     thread::spawn(move || {
//         let header_widget_data = crate::widgets::create_header_widget_data();
//         let login_widget_data = crate::widgets::create_login_widget_data();
//         let logs_widget_data = crate::widgets::create_logs_widget_data();
//         let pods_widget_data = crate::widgets::create_pods_widget_data();
//
//         let mut store = Store::new(
//             header_widget_data.get_widget().clone(),
//             login_widget_data.get_widget().clone(),
//             logs_widget_data.get_widget().clone(),
//             pods_widget_data.get_widget().clone(),
//         );
//         let mut widget_data_store = WidgetDataStore::new(
//             event_rx,
//             &mut store,
//             store_tx,
//             action_tx,
//             Box::new(crate::SimpleTruncator::new()),
//         );
//         widget_data_store.start(
//             login_widget_data.get_event_handler(),
//             logs_widget_data.get_event_handler(),
//             pods_widget_data.get_event_handler(),
//         )
//     });
//     if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
//         assert!(
//             !updated_store.request_login,
//             "store was: {:?}",
//             updated_store
//         )
//     }
//
//     event_tx.send(TUIEvent::CheckConnectivity).unwrap();
//
//     let mut actions = vec![];
//     let check_actions = vec![TUIAction::CheckConnectivity];
//     while actions != check_actions {
//         if let Ok(action) = action_rx.recv_timeout(Duration::from_millis(10)) {
//             actions.push(action);
//         }
//     }
//
//     assert!(actions == check_actions, "was {:?}", actions);
// }
//
// #[test]
// fn test_login_event() {
//     crate::init_logging().unwrap();
//     let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
//     let (action_tx, action_rx): (Sender<TUIAction>, Receiver<TUIAction>) = mpsc::channel();
//     let (store_tx, store_rx): (Sender<Store>, Receiver<Store>) = mpsc::channel();
//
//     thread::spawn(move || {
//         let header_widget_data = crate::widgets::create_header_widget_data();
//         let login_widget_data = crate::widgets::create_login_widget_data();
//         let logs_widget_data = crate::widgets::create_logs_widget_data();
//         let pods_widget_data = crate::widgets::create_pods_widget_data();
//
//         let mut store = Store::new(
//             header_widget_data.get_widget().clone(),
//             login_widget_data.get_widget().clone(),
//             logs_widget_data.get_widget().clone(),
//             pods_widget_data.get_widget().clone(),
//         );
//         let mut widget_data_store = WidgetDataStore::new(
//             event_rx,
//             &mut store,
//             store_tx,
//             action_tx,
//             Box::new(crate::SimpleTruncator::new()),
//         );
//         widget_data_store.start(
//             login_widget_data.get_event_handler(),
//             logs_widget_data.get_event_handler(),
//             pods_widget_data.get_event_handler(),
//         )
//     });
//     if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
//         assert!(!updated_store.logged_in, "store was: {:?}", updated_store)
//     }
//
//     event_tx.send(TUIEvent::NeedsLogin).unwrap();
//
//     let mut actions = vec![];
//     let check_actions = vec![TUIAction::LogIn];
//     while actions != check_actions {
//         if let Ok(action) = action_rx.recv_timeout(Duration::from_millis(10)) {
//             actions.push(action);
//         }
//     }
//
//     assert!(actions == check_actions, "was {:?}", actions);
// }
//
// #[test]
// fn test_add_log_event() {
//     crate::init_logging().unwrap();
//     let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
//     let (action_tx, _): (Sender<TUIAction>, Receiver<TUIAction>) = mpsc::channel();
//     let (store_tx, store_rx): (Sender<Store>, Receiver<Store>) = mpsc::channel();
//
//     thread::spawn(move || {
//         let header_widget_data = crate::widgets::create_header_widget_data();
//         let login_widget_data = crate::widgets::create_login_widget_data();
//         let logs_widget_data = crate::widgets::create_logs_widget_data();
//         let pods_widget_data = crate::widgets::create_pods_widget_data();
//
//         let mut store = Store::new(
//             header_widget_data.get_widget().clone(),
//             login_widget_data.get_widget().clone(),
//             logs_widget_data.get_widget().clone(),
//             pods_widget_data.get_widget().clone(),
//         );
//         let mut widget_data_store = WidgetDataStore::new(
//             event_rx,
//             &mut store,
//             store_tx,
//             action_tx,
//             Box::new(crate::SimpleTruncator::new()),
//         );
//         widget_data_store.start(
//             login_widget_data.get_event_handler(),
//             logs_widget_data.get_event_handler(),
//             pods_widget_data.get_event_handler(),
//         )
//     });
//     if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
//         assert!(
//             updated_store
//                 .clone()
//                 .logs_widget
//                 .unwrap()
//                 .get_data()
//                 .data
//                 .get("logs")
//                 == None,
//             "store was: {:?}",
//             updated_store
//         )
//     }
//
//     event_tx
//         .send(TUIEvent::AddLog("this is a new line\n".to_string()))
//         .unwrap();
//
//     if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
//         assert!(
//             updated_store
//                 .clone()
//                 .logs_widget
//                 .unwrap()
//                 .get_data()
//                 .data
//                 .get("logs")
//                 == Some(Some(vec!["this is a new line\n".to_string()])).as_ref(),
//             "store was: {:?}",
//             updated_store
//         )
//     }
//
//     event_tx
//         .send(TUIEvent::AddLog("and some extra.".to_string()))
//         .unwrap();
//
//     if let Ok(updated_store) = store_rx.recv_timeout(Duration::from_millis(10)) {
//         assert!(
//             updated_store
//                 .clone()
//                 .logs_widget
//                 .unwrap()
//                 .get_data()
//                 .data
//                 .get("logs")
//                 == Some(Some(vec![
//                     "this is a new line\n".to_string(),
//                     "and some extra.".to_string()
//                 ]))
//                 .as_ref(),
//             "store was: {:?}",
//             updated_store
//         )
//     }
// }
