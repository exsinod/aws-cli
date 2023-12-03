use std::{
    collections::HashMap,
    sync::mpsc::Sender,
    thread::{self, JoinHandle},
    time::Instant,
};

use log::debug;

use crate::aws_api::IOEventSender;
use crate::structs::TUIEvent;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum WidgetTaskId {
    CheckConnectivity,
    GetLoginLogs,
    GetLogs,
    GetPods,
}

pub struct ThreadManager<'a> {
    event_tx: &'a Sender<TUIEvent>,
    threads: HashMap<WidgetTaskId, JoinHandle<()>>,
}

impl<'a> IOEventSender<TUIEvent> for ThreadManager<'a> {}

impl<'a> ThreadManager<'a> {
    pub fn new(event_tx: &'a Sender<TUIEvent>) -> Self {
        ThreadManager {
            event_tx,
            threads: HashMap::default(),
        }
    }

    pub fn run_thread(&mut self, id: WidgetTaskId, task: fn(&Sender<TUIEvent>)) {
        if let None = self.threads.get(&id) {
            let id_to_insert = id.clone();
            let event_tx = self.event_tx.clone();
            let join_handle = thread::spawn(move || {
                task(&event_tx)
            });
            self.threads.insert(id_to_insert, join_handle);
        } else {
            debug!("ignoring, thread {:?} already running", id);
        }
    }

    pub fn _run_thread_timeout(
        &mut self,
        id: WidgetTaskId,
        task: fn(&Sender<TUIEvent>),
        _timeout_fn: fn(Instant) -> bool,
    ) {
        if let None = self.threads.get(&id) {
            let event_tx = self.event_tx.clone();
            let id_to_insert = id.clone();
            let join_handle = thread::spawn(move || {
                task(&event_tx)
            });
            self.threads.insert(id_to_insert, join_handle);
        } else {
            debug!("ignoring, thread {:?} already running", id);
        }
    }
}
