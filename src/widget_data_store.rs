pub use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use log::{debug, error, trace};

use crate::{
    structs::{TUIError, UIState},
    truncator::{TopTruncator, Truncatorix},
    widgets::RenderWidget,
    Store, TUIAction, TUIEvent,
};

pub struct WidgetDataStore<'a> {
    event_rx: Receiver<TUIEvent>,
    store: &'a mut Store,
    store_tx: &'a Sender<Store>,
    action_tx: &'a Sender<TUIAction>,
    truncator: TopTruncator,
}

impl<'a> WidgetDataStore<'a> {
    pub fn run(
        event_rx: Receiver<TUIEvent>,
        mut store: Store,
        store_tx: Sender<Store>,
        action_tx: Sender<TUIAction>,
        truncator: TopTruncator,
        widget_event_handlers: Vec<fn(&TUIEvent, &mut Store) -> Option<()>>,
    ) {
        let action_tx = action_tx.clone();
        thread::spawn(move || {
            let mut widget_data_store =
                WidgetDataStore::new(event_rx, &mut store, &store_tx, &action_tx, truncator);

            widget_data_store.start(widget_event_handlers)
        });
    }
    pub fn new(
        event_rx: Receiver<TUIEvent>,
        store: &'a mut Store,
        store_tx: &'a Sender<Store>,
        action_tx: &'a Sender<TUIAction>,
        truncator: TopTruncator,
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

    pub fn start(&mut self, event_handlers: Vec<fn(&TUIEvent, &mut Store) -> Option<()>>) {
        self.start_truncator();
        self.send();
        while let Ok(event) = self.event_rx.recv() {
            debug!("handling event: {:?}", event);
            let action_tx_clone = self.action_tx.clone();
            match event {
                TUIEvent::RequestEnvChange => {
                    self.store.env_change_possible = true;
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
                }
                TUIEvent::Error(error) => match error {
                    TUIError::VPN => {
                        self.store
                            .header_widget
                            .as_mut()
                            .unwrap()
                            .set_data("error".to_string(), vec!["Uhm... VPN on ?".to_string()]);
                    }
                    TUIError::KEY(error) | TUIError::API(error) => {
                        self.store
                            .header_widget
                            .as_mut()
                            .unwrap()
                            .set_data("error".to_string(), vec![error]);
                    }
                },
                TUIEvent::ClearError => {
                    if let Some(header_widget) = self.store.header_widget.as_mut() {
                        header_widget.clear_text_data("error".to_string());
                    }
                }
                TUIEvent::CheckConnectivity => {
                    self.store.request_login = false;
                    action_tx_clone.send(TUIAction::CheckConnectivity).unwrap();
                }
                TUIEvent::RequestLoginStart => {
                    self.store.request_login = true;
                }
                TUIEvent::RequestLoginStop => {
                    self.store.request_login = false;
                }
                TUIEvent::NeedsLogin => {
                    self.store.ui_state = UIState::LoggingIn;
                    self.action_tx.send(TUIAction::LogIn).unwrap();
                }
                TUIEvent::IsLoggedIn => {
                    debug!("logged in");
                    self.store.logged_in = true;
                    if let Some(login_widget) = self.store.login_widget.as_mut() {
                        login_widget.clear_text_data("logs".to_string());
                    }
                    if let Some(header_widget) = self.store.header_widget.as_mut() {
                        header_widget.set_data("logged in".to_string(), vec![true.to_string()]);
                    }
                    self.action_tx.send(TUIAction::CheckConnectivity).unwrap();
                }
                TUIEvent::IsConnected => {
                    self.store.logged_in = true;
                    if let Some(login_widget) = self.store.login_widget.as_mut() {
                        login_widget.clear_text_data("logs".to_string());
                    }
                    if let Some(header_widget) = self.store.header_widget.as_mut() {
                        header_widget.set_data("logged in".to_string(), vec![true.to_string()]);
                        header_widget
                            .set_data("login_info".to_string(), vec!["LOGGED IN".to_string()]);
                    }
                }
                TUIEvent::DisplayLoginCode(code) => {
                    self.store.login_code = Some(code);
                }
                event => {
                    let mut event_handlers = event_handlers.iter();
                    let mut b = Some(());
                    while let Some(()) = b {
                        if let Some(next_handler) = event_handlers.next() {
                            b = next_handler(&event, &mut self.store)
                        }
                    }
                }
            }
            if let Some(()) = self.truncator.poll() {
                self.truncator.truncate(self.store)
            }
            self.send()
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
//             pods_widget_data.get_widget().clone(),
//         );
//         let mut widget_data_store = WidgetDataStore::new(
//             event_rx,
//             &mut store,
//             &store_tx,
//             &action_tx,
//             crate::truncator::TopTruncator::new(50),
//         );
//         widget_data_store.start(vec![
//             login_widget_data.get_event_handler(),
//             logs_widget_data.get_event_handler(),
//             pods_widget_data.get_event_handler(),
//         ])
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
//             pods_widget_data.get_widget().clone(),
//         );
//         let mut widget_data_store = WidgetDataStore::new(
//             event_rx,
//             &mut store,
//             store_tx,
//             action_tx,
//             Box::new(crate::truncator::NoopTruncator::new()),
//         );
//         widget_data_store.start(vec![
//             login_widget_data.get_event_handler(),
//             logs_widget_data.get_event_handler(),
//             pods_widget_data.get_event_handler(),
//         ])
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
//             pods_widget_data.get_widget().clone(),
//         );
//         let mut widget_data_store = WidgetDataStore::new(
//             event_rx,
//             &mut store,
//             store_tx,
//             action_tx,
//             Box::new(crate::truncator::NoopTruncator::new()),
//         );
//         widget_data_store.start(vec![
//             login_widget_data.get_event_handler(),
//             logs_widget_data.get_event_handler(),
//             pods_widget_data.get_event_handler(),
//         ])
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
//             pods_widget_data.get_widget().clone(),
//         );
//         let mut widget_data_store = WidgetDataStore::new(
//             event_rx,
//             &mut store,
//             store_tx,
//             action_tx,
//             Box::new(crate::truncator::NoopTruncator::new()),
//         );
//         widget_data_store.start(vec![
//             login_widget_data.get_event_handler(),
//             logs_widget_data.get_event_handler(),
//             pods_widget_data.get_event_handler(),
//         ])
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
