use std::io::BufRead;
use std::process::{Command, Stdio};
use std::str;
use std::{
    io::{BufReader, Error},
    process::{Child, ChildStderr, ChildStdout},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use log::{debug, trace};
use regex::Regex;

use crate::structs::KubeEnvData;
use crate::thread_manager::{ThreadManager, WidgetTaskId};
use crate::structs::{TUIError, TUIEvent};

pub trait APIConnectivity {
    fn check_connectivity_command(&self) -> Result<Child, Error>;
    fn update_config_command(&self, kube_env: KubeEnvData) -> Result<Child, Error>;
    fn handle_output(&self, child: Child) -> Result<String, String>;

    fn check_connectivity(&self) -> Result<String, String> {
        match self.check_connectivity_command() {
            Ok(child) => self.handle_output(child),
            Err(error) => Err(error.to_string()),
        }
    }

    fn update_config(&self, kube_env: KubeEnvData) -> Result<String, String> {
        match self.update_config_command(kube_env) {
            Ok(child) => self.handle_output(child),
            Err(error) => Err(error.to_string()),
        }
    }
}

pub trait IOEventSender<E> {
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
        event_tx: &Sender<E>,
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
                            // event_tx.send(TUIEvent::Error(TUIError::VPN)).unwrap();
                            timeout_fn(event_tx);
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

    fn open_child_stdout(&self, child: &mut Child) -> ChildStdout {
        child.stdout.take().unwrap()
    }

    fn open_child_stderr(&self, child: &mut Child) -> ChildStderr {
        child.stderr.take().unwrap()
    }

    fn open_log_channel(
        &self,
        stdout: ChildStdout,
        stderr: ChildStderr,
    ) -> (JoinHandle<()>, Receiver<String>, Receiver<String>) {
        let (read_stdout_tx, read_stdout_rx): (Sender<String>, Receiver<String>) = mpsc::channel();
        let (read_stderr_tx, read_stderr_rx): (Sender<String>, Receiver<String>) = mpsc::channel();
        let (err_tx, err_rx): (Sender<String>, Receiver<String>) = mpsc::channel();
        let err_tx_clone = err_tx.clone();
        let mut should_break = false;
        let log_channel_thread: JoinHandle<()> = thread::spawn(move || {
            let stderr_thread = thread::spawn(move || {
                debug!("stderr thread started");
                let mut should_break = false;
                let mut stderr_buf_reader = BufReader::new(stderr);

                while !should_break {
                    let mut text_in_stderr_buf = String::new();
                    match stderr_buf_reader.read_line(&mut text_in_stderr_buf) {
                        Err(_) => {
                            debug!("stderr_buf_reader error");
                            err_tx_clone
                                .send("stderr buf reader error".to_string())
                                .unwrap();
                            should_break = true;
                        }
                        Ok(bytes_read) => {
                            if !text_in_stderr_buf.is_empty() {
                                read_stderr_tx.send(text_in_stderr_buf.clone()).unwrap();
                                err_tx_clone
                                    .send(format!(
                                        "stderr buf reader error: {:?}",
                                        text_in_stderr_buf
                                    ))
                                    .unwrap();
                                should_break = true;
                                trace!("stderr_buf_reader read {:?}", text_in_stderr_buf);
                            } else if bytes_read == 0 {
                                debug!("stderr_buf_reader EOF");
                                err_tx_clone
                                    .send("stderr buf reader EOF".to_string())
                                    .unwrap();
                                should_break = true;
                            }
                            text_in_stderr_buf.clear()
                        }
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            });
            let stdout_thread = thread::spawn(move || {
                let mut should_break = false;
                let mut stdout_buf_reader = BufReader::new(stdout);

                while !should_break {
                    let mut text_in_stdout_buf = String::new();
                    match stdout_buf_reader.read_line(&mut text_in_stdout_buf) {
                        Err(_) => {
                            debug!("stdout_buf_reader error");
                            err_tx.send("stdout buf reader error".to_string()).unwrap();
                            should_break = true;
                        }
                        Ok(bytes_read) => {
                            if !text_in_stdout_buf.is_empty() {
                                read_stdout_tx
                                    .send(text_in_stdout_buf.clone())
                                    .unwrap_or(());
                                trace!("stdout_buf_reader read {:?}", text_in_stdout_buf);
                            } else if bytes_read == 0 {
                                debug!("stdout_buf_reader EOF");
                                err_tx
                                    .send("stdout buf reader EOF".to_string())
                                    .unwrap_or(());
                                should_break = true;
                            }
                            text_in_stdout_buf.clear()
                        }
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            });
            while !should_break {
                if let Ok(error) = err_rx.recv() {
                    debug!("received {:?}", error);
                    should_break = true;
                }
            }
            stdout_thread.join().unwrap();
            stderr_thread.join().unwrap();
            debug!("stdout and stderr threads stopped");
        });
        (log_channel_thread, read_stdout_rx, read_stderr_rx)
    }
}

pub struct AwsAPI<'a> {
    thread_manager: ThreadManager<'a>,
    event_tx: &'a Sender<TUIEvent>,
}

impl IOEventSender<TUIEvent> for AwsAPI<'_> {}

impl APIConnectivity for AwsAPI<'_> {
    fn update_config_command(&self, kube_env: KubeEnvData) -> Result<Child, Error> {
        Command::new("aws")
            .arg("eks")
            .arg("--profile")
            .arg(kube_env.profile) //config
            .arg("update-kubeconfig")
            .arg("--name")
            .arg(kube_env.environment) //config
            // Command::new("cat")
            //     .arg("aws_sso_mock.sh") //config
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn check_connectivity_command(&self) -> Result<Child, Error> {
        Command::new("kubectl")
            .arg("get")
            .arg("-n")
            .arg("myccv-dev-salespoint") //config
            .arg("pods")
            // Command::new("cat")
            //     .arg("aws_sso_mock.sh") //config
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn handle_output(&self, child: Child) -> Result<String, String> {
        self.wait_for_output_with_timeout(child, self.event_tx, |event_tx| {})
    }
}

impl<'a> AwsAPI<'a> {
    pub fn new(event_tx: &'a Sender<TUIEvent>) -> Self {
        AwsAPI {
            thread_manager: ThreadManager::new(event_tx),
            event_tx,
        }
    }
    pub fn run_task(
        &mut self,
        id: WidgetTaskId,
        child: Result<Child, Error>,
        success_fn: fn(output: String, Sender<TUIEvent>),
        error_fn: fn(Sender<TUIEvent>),
    ) {
        if let Ok(child) = child {
            match self.wait_for_output_with_timeout(child, self.event_tx, |event_tx| {}) {
                Ok(output) => {
                    debug!("output is {:?}", output);
                    self.event_tx.send(TUIEvent::AddPods(output)).unwrap();
                }
                Err(error) => {
                    self.on_error(&error);
                    self.event_tx.send(TUIEvent::RequestLoginStart).unwrap();
                }
            }
        }
    }

    pub fn login(&mut self) {
        self.thread_manager
            .run_thread(WidgetTaskId::GetLoginLogs, |event_tx| {
                let aws_ap = AwsAPI::new(event_tx);
                if let Ok(mut child) = aws_ap.login_command() {
                    let child_stdout = aws_ap.open_child_stdout(&mut child);
                    let child_stderr = aws_ap.open_child_stderr(&mut child);
                    let (thread_handle, read_stdout_rx, read_stderr_rx) =
                        aws_ap.open_log_channel(child_stdout, child_stderr);
                    while !thread_handle.is_finished() {
                        if let Ok(error) = read_stderr_rx.recv_timeout(Duration::from_millis(10)) {
                            aws_ap.on_error(&error);
                        }
                        if let Ok(line) = read_stdout_rx.recv_timeout(Duration::from_millis(10)) {
                            aws_ap.add_login_logs(&line);
                            aws_ap.check_login_status(&line);
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                }
            });
    }

    pub fn get_logs(&mut self) {
        self.thread_manager
            .run_thread(WidgetTaskId::GetLoginLogs, |event_tx| {
                let aws_ap = AwsAPI::new(event_tx);
                return if let Ok(mut child) = aws_ap.get_logs_command() {
                    let now = Instant::now();
                    let mut has_error = false;
                    let child_stdout = aws_ap.open_child_stdout(&mut child);
                    let child_stderr = aws_ap.open_child_stderr(&mut child);
                    debug!("open_log_channel for get_logs");
                    let (thread_handle, read_stdout_rx, read_stderr_rx) =
                        aws_ap.open_log_channel(child_stdout, child_stderr);
                    while !thread_handle.is_finished() {
                        if false {
                            // TODO: properly do this
                            child.kill().unwrap();
                        }
                        if let Ok(error) = read_stderr_rx.recv_timeout(Duration::from_millis(10)) {
                            aws_ap.on_error(&error);
                            has_error = true;
                        }
                        if let Ok(line) = read_stdout_rx.recv_timeout(Duration::from_millis(10)) {
                            aws_ap.add_logs(&line);
                        }
                    }
                    if !child.wait().unwrap().success() || has_error {
                        debug!("child had errors");
                        // Err("process experienced some errors".to_string())
                    } else {
                        debug!("child had no errors");
                        // Ok(())
                    }
                } else {
                    // Err("process quit immediately".to_string())
                };
            });
    }

    pub fn get_pods(&self) -> Result<String, String> {
        match self.get_pods_command() {
            Ok(child) => self.wait_for_output(child),
            Err(error) => Err(error.to_string()),
        }
    }

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

    pub fn login_command(&self) -> Result<Child, Error> {
        // Command::new("cat")
        //     .arg("aws_sso_mock.sh") //config
        Command::new("aws")
            .arg("sso")
            .arg("login")
            .arg("--profile")
            .arg("myccv-lab-non-prod-myccv-lab-developer") //config
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn get_logs_command(&self) -> Result<Child, Error> {
        // Command::new("tail")
        //     .arg("-f")
        //     .arg("src/main.rs")
        Command::new("kubectl")
            .arg("logs")
            .arg("-n")
            .arg("myccv-dev-salespoint") //config
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

    pub fn get_pods_command(&self) -> Result<Child, Error> {
        Command::new("kubectl")
            .arg("get")
            .arg("-n")
            .arg("myccv-dev-salespoint") //config
            .arg("pods")
            // Command::new("cat")
            //     .arg("aws_sso_mock.sh") //config
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn get_tail_command(&self) -> Result<Child, Error> {
        Command::new("cat")
            .arg("logs.txt")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }
}

pub struct Commands {}

impl Commands {
    fn login_command(&self) -> Result<Child, Error> {
        // Command::new("cat")
        //     .arg("aws_sso_mock.sh") //config
        Command::new("aws")
            .arg("sso")
            .arg("login")
            .arg("--profile")
            .arg("myccv-lab-non-prod-myccv-lab-developer") //config
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn get_logs_command(&self) -> Result<Child, Error> {
        // Command::new("tail")
        //     .arg("-f")
        //     .arg("src/main.rs")
        Command::new("kubectl")
            .arg("logs")
            .arg("-n")
            .arg("myccv-dev-salespoint") //config
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

    fn get_tail_command(&self) -> Result<Child, Error> {
        Command::new("cat")
            .arg("logs.txt")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    fn update_kubeconfig_command(&self, kube_env: KubeEnvData) -> Result<Child, Error> {
        Command::new("aws")
            .arg("eks")
            .arg("--profile")
            .arg(kube_env.profile) //config
            .arg("update-kubeconfig")
            .arg("--name")
            .arg(kube_env.environment) //config
            // Command::new("cat")
            //     .arg("aws_sso_mock.sh") //config
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }

    pub fn get_pods_command() -> Result<Child, Error> {
        Command::new("kubectl")
            .arg("get")
            .arg("-n")
            .arg("myccv-dev-salespoint") //config
            .arg("pods")
            // Command::new("cat")
            //     .arg("aws_sso_mock.sh") //config
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    }
}
