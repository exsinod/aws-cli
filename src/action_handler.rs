use std::{
    sync::mpsc::{Receiver, Sender},
    thread,
};

use log::debug;

use crate::thread_manager::WidgetTaskId;
use crate::{
    structs::{KubeEnv, DEV, PROD},
    aws_api::{APIConnectivity, AwsAPI},
};
use crate::{TUIAction, TUIEvent};

pub struct ActionHandler<'a> {
    event_tx: &'a Sender<TUIEvent>,
    action_rx: Receiver<TUIAction>,
    aws_api: AwsAPI<'a>,
}

impl<'a> ActionHandler<'a> {
    pub fn run(event_tx: &Sender<TUIEvent>, action_rx: Receiver<TUIAction>) {
        let event_tx = event_tx.clone();
        thread::spawn(move || {
            let mut action_handler = ActionHandler {
                event_tx: &event_tx,
                action_rx,
                aws_api: AwsAPI::new(&event_tx),
            };
            action_handler.start()
        });
    }

    pub fn start(&mut self) {
        while let Ok(action) = self.action_rx.recv() {
            debug!("handling action: {:?}", action);
            match action {
                TUIAction::ChangeEnv(env) => {
                    let env_data = match env {
                        KubeEnv::Dev => DEV,
                        KubeEnv::Prod => PROD,
                    };
                    match self.aws_api.check_connectivity() {
                        Ok(_) => match self.aws_api.update_config(env_data) {
                            Ok(_) => {
                                self.event_tx.send(TUIEvent::IsConnected).unwrap();
                                self.event_tx.send(TUIEvent::ClearError).unwrap();
                            }
                            Err(_) => {
                                self.event_tx.send(TUIEvent::RequestLoginStart).unwrap();
                            }
                        },
                        Err(error) => {
                            // self.task_manager.on_error(&error);
                            self.event_tx.send(TUIEvent::RequestLoginStart).unwrap();
                        }
                    };
                }
                TUIAction::CheckConnectivity => match self.aws_api.check_connectivity() {
                    Ok(_) => {
                        self.event_tx.send(TUIEvent::IsConnected).unwrap();
                        self.event_tx.send(TUIEvent::ClearError).unwrap();
                    }
                    Err(error) => {
                        // self.thread_manager.on_error(&error);
                        self.event_tx.send(TUIEvent::RequestLoginStart).unwrap();
                    }
                },
                TUIAction::LogIn => {
                    self.aws_api.login();
                    // self.thread_manager.run_thread(Task {
                    // id: WidgetTaskId::GetLoginLogs,
                    // command_fn: || self.aws_api.,
                    // success_fn: |e| {},
                    // error_fn: |e| {}
                    // });
                }
                TUIAction::GetLogs => {
                    self.aws_api.get_logs();
                }
                TUIAction::GetPods => {
                    self.aws_api.run_task(
                        WidgetTaskId::GetPods,
                        self.aws_api.get_pods_command(),
                        |output: String, event_tx: Sender<TUIEvent>| {
                            event_tx.send(TUIEvent::AddPods(output)).unwrap();
                        },
                        |event_tx: Sender<TUIEvent>| {
                            event_tx.send(TUIEvent::RequestLoginStart).unwrap();
                        },
                    );
                }

                TUIAction::GetTail => {
                    // self.thread_manager.run_task(
                    //     WidgetTaskId::GetPods,
                    //     |output: String, event_tx: Sender<TUIEvent>| {
                    //         event_tx.send(TUIEvent::AddPods(output)).unwrap();
                    //     },
                    //     |event_tx: Sender<TUIEvent>| {
                    //         event_tx.send(TUIEvent::RequestLoginStart).unwrap();
                    //     },
                    // );
                }
            }
        }
    }
}
