use std::{
    sync::mpsc::{Receiver, Sender},
    thread,
};

use log::debug;

use crate::{
    aws_api::{APIConnectivity, AwsAPI},
    structs::{KubeEnv, DEV, PROD, TEST},
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
                        KubeEnv::Test => TEST,
                        KubeEnv::Prod => PROD,
                    };
                    match self.aws_api.check_connectivity() {
                        Ok(_) => match self.aws_api.update_config(&env_data) {
                            Ok(_) => {
                                self.aws_api.set_kube_env(&env_data);
                                self.event_tx.send(TUIEvent::IsConnected).unwrap();
                                self.event_tx.send(TUIEvent::ClearError).unwrap();
                            }
                            Err(error) => {
                                debug!("error: {:?}", error);
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
                }
                TUIAction::GetLogs => {
                    self.aws_api.get_logs();
                }
                TUIAction::GetPods => {
                    self.aws_api.get_pods();
                }
            }
        }
    }
}
