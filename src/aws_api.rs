use std::process::{Command, Stdio};
use std::str;
pub use std::{
    io::Error,
    process::Child,
    sync::mpsc::{self, Receiver, Sender},
    thread::{self},
    time::{Duration, Instant},
};

use log::{debug, trace};
use regex::Regex;

pub use crate::structs::{KubeEnvData, TUIAction};
use crate::structs::{TUIError, TUIEvent, DEV};
use crate::thread_manager::{ThreadManager, WidgetTaskId};

pub trait APIConnectivity<'a> {
    fn check_connectivity_command(&self) -> Result<Child, Error>;
    fn update_config_command(&self, kube_env: &KubeEnvData) -> Result<Child, Error>;
    fn update_config(&mut self, kube_env: &KubeEnvData<'a>) -> Result<String, String>;
    fn handle_output(&self, child: Child) -> Result<String, String>;

    fn check_connectivity(&self) -> Result<String, String> {
        match self.check_connectivity_command() {
            Ok(child) => self.handle_output(child),
            Err(error) => Err(error.to_string()),
        }
    }
}

pub trait IOEventSender<E> {
    fn event_tx(&self) -> &Sender<E>;

    fn wait_for_output(&self, child: Child) -> Result<String, String> {
        let process = child.wait_with_output();
        match process {
            Err(err) => {
                // did not reach this part so far...
                Err("Unknown error: {:?}".to_string() + &err.to_string())
            }
            Ok(output) => {
                if output.status.success() {
                    Ok(str::from_utf8(&output.stdout).unwrap().to_string())
                } else {
                    Err("Error: {:?}".to_string()
                        + str::from_utf8(output.stderr.as_slice()).unwrap())
                }
            }
        }
    }

    fn wait_for_output_with_timeout(
        &self,
        mut child: Child,
        timeout_fn: fn(&Sender<E>),
    ) -> Result<String, String> {
        let now = Instant::now();
        let mut result: Option<Result<String, String>> = None;
        let mut send_error = true;
        while result == None {
            match child.try_wait() {
                Ok(Some(status)) => {
                    debug!("wait with timeout finished {:?}", status.to_string());
                    if let true = status.success() {
                        result = match child.wait_with_output() {
                            Ok(output) => {
                                Some(Ok(str::from_utf8(&output.stdout).unwrap().to_string()))
                            }
                            Err(_) => Some(Err("Error wait_with_output".to_string())),
                        };
                        break;
                    } else {
                        result = Some(Err(
                            "Exit code ".to_string() + &status.code().unwrap().to_string()
                        ));
                    }
                }
                Ok(None) => {
                    trace!("wait with timeout still waiting");
                    if now.elapsed().as_secs() > 1 {
                        if send_error {
                            send_error = false;
                            // self.event_tx().send(TUIEvent::Error(TUIError::VPN)).unwrap();
                            timeout_fn(self.event_tx());
                        }
                    }
                    if now + Duration::from_secs(60) < Instant::now() {
                        debug!("wait with timeout timed out");
                        result = Some(Err("timeout".to_string()));
                    };
                    thread::sleep(Duration::from_millis(100))
                }
                Err(_) => {
                    debug!("wait with timeout error");
                    result = Some(Err("error".to_string()));
                }
            };
        }
        result.unwrap_or(Err("nothing".to_string()))
    }
}

pub trait AwsApiCommands {
    fn login_command(&self, kube_env: &KubeEnvData) -> Result<Child, Error>;
    fn get_logs_command(&self, kube_env: &KubeEnvData) -> Result<Child, Error>;
    fn get_pods_command(&self, kube_env: &KubeEnvData) -> Result<Child, Error>;
}

pub struct AwsApiCommandsProvider {}
impl AwsApiCommandsProvider {
    pub fn new() -> Self {
        AwsApiCommandsProvider {}
    }
}
impl AwsApiCommands for AwsApiCommandsProvider {
    fn login_command(&self, kube_env: &KubeEnvData) -> Result<Child, Error> {
        Command::new("aws")
            .arg("sso")
            .arg("login")
            .arg("--profile")
            .arg(kube_env.aws_profile) //config
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn get_logs_command(&self, kube_env: &KubeEnvData) -> Result<Child, Error> {
        Command::new("kubectl")
            .arg("logs")
            .arg("-n")
            .arg(kube_env.namespace) //config
            .arg("-l")
            .arg("component=salespoint-v2") //config
            .arg("-c")
            .arg("salespoint-v2") //config
            .arg("-f")
            .arg("--prefix=true")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn get_pods_command(&self, kube_env: &KubeEnvData) -> Result<Child, Error> {
        debug!("getting from {:?}", kube_env);
        Command::new("kubectl")
            .arg("get")
            .arg("-n")
            .arg(kube_env.namespace) //config
            .arg("pods")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }
}

#[derive(Clone)]
pub struct AwsAPIHandler {
    event_tx: Sender<TUIEvent>,
}
impl AwsAPIHandler {
    pub fn on_error(&self, error: &str) {
        self.event_tx
            .send(TUIEvent::Error(TUIError::API(error.to_string())))
            .unwrap();
    }

    pub fn check_login_status(&self, line: &str) {
        let re_code = Regex::new(r"[A-Za-z]{4}-[A-Za-z]{4}").unwrap();
        if let Some(code) = re_code.captures(&line) {
            self.event_tx
                .send(TUIEvent::DisplayLoginCode(
                    code.get(0).unwrap().as_str().to_string(),
                ))
                .unwrap();
        }
        if line.contains("Successfully") {
            self.event_tx.send(TUIEvent::IsLoggedIn).unwrap();
        }
    }

    pub fn add_login_logs(&self, line: &str) {
        self.event_tx
            .send(TUIEvent::AddLoginLog(line.to_string()))
            .unwrap();
    }

    pub fn add_logs(&self, line: &str) {
        self.event_tx
            .send(TUIEvent::AddLog(line.to_string()))
            .unwrap();
    }

    pub fn add_pods(&self, pods: &str) {
        self.event_tx
            .send(TUIEvent::AddPods(pods.to_string()))
            .unwrap();
    }
}

pub struct AwsAPI<'a> {
    kube_env: KubeEnvData<'a>,
    commands_provider: Box<dyn AwsApiCommands + Send>,
    handler: AwsAPIHandler,
    thread_manager: ThreadManager<'a>,
    event_tx: &'a Sender<TUIEvent>,
}

impl<'a> IOEventSender<TUIEvent> for AwsAPI<'a> {
    fn event_tx(&self) -> &Sender<TUIEvent> {
        self.event_tx
    }
}

impl<'a> APIConnectivity<'a> for AwsAPI<'a> {
    fn update_config_command(&self, kube_env: &KubeEnvData) -> Result<Child, Error> {
        Command::new("aws")
            .arg("eks")
            .arg("--profile")
            .arg(kube_env.eks_profile) //config
            .arg("update-kubeconfig")
            .arg("--name")
            .arg(kube_env.environment) //config
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn check_connectivity_command(&self) -> Result<Child, Error> {
        Command::new("kubectl")
            .arg("get")
            .arg("-n")
            .arg(self.kube_env.namespace) //config
            .arg("pods")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn update_config(&mut self, kube_env: &KubeEnvData<'a>) -> Result<String, String> {
        match self.update_config_command(kube_env) {
            Ok(child) => {
                self.thread_manager.stop_threads();
                let result = self.handle_output(child);
                result
            }
            Err(error) => Err(error.to_string()),
        }
    }

    fn handle_output(&self, child: Child) -> Result<String, String> {
        self.wait_for_output_with_timeout(child, |_| {})
    }
}

impl<'a> AwsAPI<'a> {
    pub fn new(event_tx: &'a Sender<TUIEvent>) -> Self {
        AwsAPI {
            kube_env: DEV,
            commands_provider: Box::new(AwsApiCommandsProvider::new()),
            handler: AwsAPIHandler {
                event_tx: event_tx.clone(),
            },
            thread_manager: ThreadManager::new(event_tx),
            event_tx,
        }
    }

    pub fn set_commands_provider(&mut self, commands_provider: Box<dyn AwsApiCommands + Send>) {
        self.commands_provider = commands_provider
    }

    pub fn set_kube_env(&mut self, kube_env: &KubeEnvData<'a>) {
        self.kube_env = kube_env.clone();
    }

    pub fn login(&mut self) {
        if let Ok(child) = self.commands_provider.login_command(&self.kube_env) {
            self.thread_manager.run_thread_timeout(
                WidgetTaskId::GetLoginLogs,
                child,
                |line, handler| {
                    handler.add_login_logs(&line);
                    handler.check_login_status(&line);
                },
                |error, handler| handler.on_error(error),
                self.handler.clone(),
            );
        }
    }

    pub fn get_logs(&mut self) {
        return if let Ok(child) = self.commands_provider.get_logs_command(&self.kube_env) {
            self.thread_manager.run_thread_timeout(
                WidgetTaskId::GetLogs,
                child,
                |line, handler| {
                    handler.add_logs(&line);
                },
                |error, handler| handler.on_error(error),
                self.handler.clone(),
            );
        };
    }

    pub fn get_pods(&self) {
        if let Ok(child) = self.commands_provider.get_pods_command(&self.kube_env) {
            match self.wait_for_output_with_timeout(child, |_| {}) {
                Ok(output) => {
                    self.handler.add_pods(&output);
                }
                Err(error) => {
                    self.handler.on_error(&error);
                    self.event_tx.send(TUIEvent::RequestLoginStart).unwrap();
                }
            }
        }
    }
}

struct TestAwsApiCommandProvider {
    _event_tx: Sender<TUIEvent>,
}
impl AwsApiCommands for TestAwsApiCommandProvider {
    fn login_command(&self, _: &KubeEnvData) -> Result<Child, Error> {
        Command::new("sh")
            .arg("-C")
            .arg("test_res/test_login_succeed.sh") //config
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn get_logs_command(&self, _: &KubeEnvData) -> Result<Child, Error> {
        Command::new("tail")
            .arg("-f") //config
            .arg("test_res/get_logs.txt") //config
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn get_pods_command(&self, kube_env: &KubeEnvData) -> Result<Child, Error> {
        todo!()
    }
}

impl TestAwsApiCommandProvider {
    pub fn new(event_tx: Sender<TUIEvent>) -> Self {
        TestAwsApiCommandProvider {
            _event_tx: event_tx,
        }
    }
}

struct TestAwsApiCommandFailProvider {}
impl AwsApiCommands for TestAwsApiCommandFailProvider {
    fn login_command(&self, _: &KubeEnvData) -> Result<Child, Error> {
        Command::new("sh")
            .arg("-C")
            .arg("test_res/test_login_fail.sh") //config
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn get_logs_command(&self, _: &KubeEnvData) -> Result<Child, Error> {
        Command::new("sh")
            .arg("-C") //config
            .arg("test_res/long_living_process_quits_unexpectedly.sh") //config
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn get_pods_command(&self, _: &KubeEnvData) -> Result<Child, Error> {
        todo!()
    }
}

impl TestAwsApiCommandFailProvider {
    pub fn new(_: Sender<TUIEvent>) -> Self {
        TestAwsApiCommandFailProvider {}
    }
}

#[test]
fn test_login_succeed() {
    crate::init_logging().unwrap();
    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();

    thread::spawn(move || {
        let event_tx_clone = event_tx.clone();
        let mut aws_api = AwsAPI::new(&event_tx_clone);
        aws_api.set_commands_provider(Box::new(TestAwsApiCommandProvider::new(event_tx)));
        aws_api.login()
    });

    let check_events = vec![
            TUIEvent::AddLoginLog("Attempting to automatically open the SSO authorization page in your default browser.\n".to_string()),
            TUIEvent::AddLoginLog("If the browser does not open or you wish to use a different device to authorize this request, open the following URL:\n".to_string()),
            TUIEvent::AddLoginLog("\n".to_string()),
            TUIEvent::AddLoginLog("https://device.sso.eu-west-1.amazonaws.com/\n".to_string()),
            TUIEvent::AddLoginLog("\n".to_string()),
            TUIEvent::AddLoginLog("Then enter the code:\n".to_string()),
            TUIEvent::AddLoginLog("\n".to_string()),
            TUIEvent::AddLoginLog("MQBJ-XSZB\n".to_string()),
            TUIEvent::DisplayLoginCode("MQBJ-XSZB".to_string()),
            TUIEvent::AddLoginLog("Successfully\n".to_string()),
    TUIEvent::IsLoggedIn];

    let mut events = vec![];

    thread::sleep(Duration::from_secs(5));

    while events != check_events {
        if let Ok(event) = event_rx.recv_timeout(Duration::from_millis(10)) {
            events.push(event);
        }
    }

    assert!(events == check_events, "events was: {:?}", events);
}

#[test]
fn test_login_fail() {
    crate::init_logging().unwrap();
    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();

    thread::spawn(move || {
        let event_tx_clone = event_tx.clone();
        let mut aws_api = AwsAPI::new(&event_tx_clone);
        aws_api.set_commands_provider(Box::new(TestAwsApiCommandFailProvider::new(event_tx)));
        aws_api.login()
    });

    let check_events = vec![TUIEvent::Error(TUIError::API(
        "this is an unusual error\n".to_string(),
    ))];

    let mut events = vec![];

    while events != check_events {
        if let Ok(event) = event_rx.recv_timeout(Duration::from_millis(10)) {
            events.push(event);
        }
    }

    assert!(events == check_events, "events was: {:?}", events);
}

#[test]
fn test_open_log_channel() {
    crate::init_logging().unwrap();
    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();

    thread::spawn(move || {
        let event_tx_clone = event_tx.clone();
        let mut aws_api = AwsAPI::new(&event_tx_clone);
        aws_api.set_commands_provider(Box::new(TestAwsApiCommandFailProvider::new(event_tx)));
        aws_api.get_logs()
    });

    let mut events = vec![];
    let check_events = vec![
        TUIEvent::AddLog("Beginning...\n".to_string()),
        TUIEvent::Error(TUIError::API("process experienced some errors".to_string())),
    ];

    while events != check_events {
        if let Ok(event) = event_rx.recv_timeout(Duration::from_millis(10)) {
            events.push(event);
        }
    }

    assert!(events == check_events, "events was: {:?}", events);
}

#[test]
fn test_get_logs() {
    crate::init_logging().unwrap();
    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let (_, action_rx): (Sender<TUIAction>, Receiver<TUIAction>) = mpsc::channel();

    thread::spawn(move || {
        let event_tx_clone = event_tx.clone();
        let mut aws_api = AwsAPI::new(&event_tx_clone);
        aws_api.set_commands_provider(Box::new(TestAwsApiCommandProvider::new(event_tx)));
        aws_api.get_logs();
    });
    let mut events = vec![];
    let mut actions = vec![];
    let check_events = vec![TUIEvent::AddLog("Lorem ipsum dolor sit amet, consectetur adipiscing elit. Mauris vitae efficitur elit, sit amet euismod magna. \n".to_string()),
TUIEvent::AddLog("Nulla mattis eros vel erat varius elementum a nec ex. \n".to_string()),
TUIEvent::AddLog("Vestibulum ante ipsum primis in faucibus orci luctus et ultrices posuere cubilia curae; Donec sit amet porttitor lorem. \n".to_string()),
TUIEvent::AddLog("Pellentesque consectetur orci sit amet turpis auctor, ac pretium arcu consectetur. \n".to_string()),
TUIEvent::AddLog("Duis blandit nisl non sem mattis, eget mattis enim lacinia. \n".to_string()),
TUIEvent::AddLog("Cras vestibulum efficitur lacus. Vivamus ac ultrices libero. \n".to_string()),
TUIEvent::AddLog("Integer venenatis convallis massa vitae tempus. Pellentesque a commodo lectus, ac maximus lectus. \n".to_string()),
TUIEvent::AddLog("Quisque ex magna, vulputate nec porttitor sed, ullamcorper sit amet nisi. \n".to_string()),
TUIEvent::AddLog("Nullam placerat metus lectus, congue commodo mi commodo in. \n".to_string()),
TUIEvent::AddLog("Nullam volutpat magna ut leo auctor, sollicitudin pharetra tellus malesuada.\n".to_string())];

    while events != check_events {
        if let Ok(event) = event_rx.recv_timeout(Duration::from_millis(10)) {
            events.push(event);
        }
    }
    while actions != [] {
        if let Ok(action) = action_rx.recv_timeout(Duration::from_millis(10)) {
            actions.push(action);
        }
    }

    assert!(events == check_events, "events was: {:?}", events);

    assert!(actions.is_empty(), "actions was: {:?}", actions);
}

#[test]
fn test_wait_with_output_timeout() {
    crate::init_logging().unwrap();
    let child = Command::new("sh")
        .arg("-C")
        .arg("test_res/check_connectivity.sh")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let mut events = vec![];
    let check_events = vec![TUIEvent::Error(TUIError::VPN)];
    let aws_api = AwsAPI::new(&event_tx);
    match child {
        Ok(child) => match aws_api.wait_for_output_with_timeout(child, |event_tx| {
            event_tx.send(TUIEvent::Error(TUIError::VPN)).unwrap();
        }) {
            Ok(output) => {
                while events.is_empty() {
                    if let Ok(event) = event_rx.recv_timeout(Duration::from_millis(10)) {
                        events.push(event);
                    }
                }
                assert!(events == check_events, "events was: {:?}", events);
                assert!(output == "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Mauris vitae efficitur elit, sit amet euismod magna. \nNulla mattis eros vel erat varius elementum a nec ex. \nVestibulum ante ipsum primis in faucibus orci luctus et ultrices posuere cubilia curae; Donec sit amet porttitor lorem. \nPellentesque consectetur orci sit amet turpis auctor, ac pretium arcu consectetur. \nDuis blandit nisl non sem mattis, eget mattis enim lacinia. \nCras vestibulum efficitur lacus. Vivamus ac ultrices libero. \nInteger venenatis convallis massa vitae tempus. Pellentesque a commodo lectus, ac maximus lectus. \nQuisque ex magna, vulputate nec porttitor sed, ullamcorper sit amet nisi. \nNullam placerat metus lectus, congue commodo mi commodo in. \nNullam volutpat magna ut leo auctor, sollicitudin pharetra tellus malesuada.\n".to_string(), "output was {:?}", output)
            }
            Err(_) => {}
        },
        Err(_) => {}
    }
}

#[test]
fn test_wait_with_output_timeout_fail() {
    crate::init_logging().unwrap();
    let child = Command::new("sh")
        .arg("-C")
        .arg("test_res/check_connectivity_fail.sh")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let (event_tx, _): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let aws_api = AwsAPI::new(&event_tx);
    match child {
        Ok(child) => match aws_api.wait_for_output_with_timeout(child, |_| {}) {
            Ok(_) => {}
            Err(error) => {
                assert!(error == "Exit code 1".to_string(), "error was: {:?}", error);
            }
        },
        Err(_) => {}
    }
}
