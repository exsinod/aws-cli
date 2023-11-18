use std::io::{BufRead, BufReader, Error};
use std::process::ChildStderr;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};
use std::{
    process::{Child, ChildStdout, Command, Stdio},
    str,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use log::{debug, trace, LevelFilter};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::Config;
use regex::Regex;

use crate::{TUIAction, TUIEvent};

pub fn action_thread(
    event_tx: Sender<TUIEvent>,
    action_rx: Receiver<TUIAction>,
    action_tx: Sender<TUIAction>,
) {
    let event_tx_clone = event_tx.clone();
    while let Ok(action) = action_rx.recv() {
        debug!("handling action: {:?}", action);
        match action {
            TUIAction::CheckConnectivity => match check_connectivity(event_tx_clone.clone()) {
                Ok(_) => {
                    event_tx.clone().send(TUIEvent::IsLoggedIn).unwrap();
                    action_tx.clone().send(TUIAction::GetLogs).unwrap();
                }
                Err(error) => {
                    on_error(error, event_tx.clone());
                    event_tx.clone().send(TUIEvent::NeedsLogin).unwrap();
                }
            },
            TUIAction::LogIn => {
                let event_tx_clone = event_tx_clone.clone();
                thread::spawn(move || login(login_command(), event_tx_clone.clone()));
            }
            TUIAction::GetLogs => {
                let event_tx_clone = event_tx_clone.clone();
                match update_kubeconfig(event_tx_clone.clone()) {
                    Ok(_) => {
                        thread::spawn(move || {
                            event_tx_clone.send(TUIEvent::LogThreadStarted).unwrap();
                            get_logs(get_logs_command(), event_tx_clone.clone());
                        });
                    }
                    Err(error) => {
                        on_error(error, event_tx.clone());
                        event_tx.clone().send(TUIEvent::NeedsLogin).unwrap();
                        event_tx.clone().send(TUIEvent::LogThreadStopped).unwrap();
                    }
                }
            }
            TUIAction::GetPods => {
                let event_tx_clone = event_tx_clone.clone();
                match check_connectivity(event_tx_clone.clone()) {
                    Ok(_) => match get_pods() {
                        Ok(output) => {
                            event_tx_clone.send(TUIEvent::AddPods(output)).unwrap();
                        }
                        Err(error) => {
                            on_error(error, event_tx.clone());
                            event_tx.clone().send(TUIEvent::NeedsLogin).unwrap();
                        }
                    },
                    Err(error) => {
                        on_error(error, event_tx.clone());
                        event_tx.clone().send(TUIEvent::NeedsLogin).unwrap();
                    }
                }
            }
        }
    }
}

fn login_command() -> Result<Child, Error> {
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

fn get_logs_command() -> Result<Child, Error> {
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

fn update_kubeconfig_command() -> Result<Child, Error> {
    Command::new("aws")
        .arg("eks")
        .arg("--profile")
        .arg("eks-non-prod-myccv-lab-developer") //config
        .arg("update-kubeconfig")
        .arg("--name")
        .arg("shared-non-prod-2") //config
        // Command::new("cat")
        //     .arg("aws_sso_mock.sh") //config
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

fn get_pods_command() -> Result<Child, Error> {
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

fn update_kubeconfig(event_tx: Sender<TUIEvent>) -> Result<String, String> {
    match update_kubeconfig_command() {
        Ok(child) => wait_for_output_with_timeout(child, event_tx),
        Err(error) => Err(error.to_string()),
    }
}

fn get_pods() -> Result<String, String> {
    match get_pods_command() {
        Ok(child) => wait_for_output(child),
        Err(error) => Err(error.to_string()),
    }
}

fn check_connectivity(event_tx: Sender<TUIEvent>) -> Result<String, String> {
    match get_pods_command() {
        Ok(child) => wait_for_output_with_timeout(child, event_tx),
        Err(error) => Err(error.to_string()),
    }
}

fn get_logs(child: Result<Child, Error>, event_tx: Sender<TUIEvent>) {
    if let Ok(mut child) = child {
        let child_stdout = open_child_stdout(&mut child);
        let child_stderr = open_child_stderr(&mut child);
        let (thread_handle, read_stdout_rx, read_stderr_rx) =
            open_log_channel(child_stdout, child_stderr);
        while !thread_handle.is_finished() {
            if let Ok(error) = read_stderr_rx.recv_timeout(Duration::from_millis(10)) {
                on_error(error.clone(), event_tx.clone());
            }
            if let Ok(line) = read_stdout_rx.recv_timeout(Duration::from_millis(10)) {
                add_logs(event_tx.clone(), line.clone());
            }
        }
    }
}

fn wait_for_output_with_timeout(
    mut child: Child,
    event_tx: Sender<TUIEvent>,
) -> Result<String, String> {
    let now = SystemTime::now();
    let mut result: Option<Result<String, String>> = None;
    let mut send_error = false;
    while result == None {
        match child.try_wait() {
            Ok(Some(status)) => {
                debug!("Process finished {:?}", status.to_string());
                if let true = status.success() {
                    debug!("Process finished successfully");
                    result = match child.wait_with_output() {
                        Ok(output) => Some(Ok(str::from_utf8(&output.stdout).unwrap().to_string())),
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
                trace!("Process still waiting");
                if now + Duration::from_secs(2) < SystemTime::now() && !send_error {
                    event_tx
                        .send(TUIEvent::Error("Uhm... VPN on ?".to_string()))
                        .unwrap();
                    send_error = true;
                }
                if now + Duration::from_secs(60) < SystemTime::now() {
                    debug!("Process timed out");
                    result = Some(Err("timeout".to_string()));
                };
                thread::sleep(Duration::from_millis(10))
            }
            Err(_) => {
                debug!("Process error");
                result = Some(Err("error".to_string()));
            }
        };
    }
    result.unwrap_or(Ok("nothing".to_string()))
}

fn wait_for_output(child: Child) -> Result<String, String> {
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
                Err("Error: {:?}".to_string() + str::from_utf8(output.stderr.as_slice()).unwrap())
            }
        }
    }
}

fn login(child: Result<Child, Error>, event_tx: Sender<TUIEvent>) {
    if let Ok(mut child) = child {
        let child_stdout = open_child_stdout(&mut child);
        let child_stderr = open_child_stderr(&mut child);
        let (thread_handle, read_stdout_rx, read_stderr_rx) =
            open_log_channel(child_stdout, child_stderr);
        while !thread_handle.is_finished() {
            if let Ok(error) = read_stderr_rx.recv_timeout(Duration::from_millis(10)) {
                on_error(error.clone(), event_tx.clone());
            }
            if let Ok(line) = read_stdout_rx.recv_timeout(Duration::from_millis(10)) {
                check_login_status(line.clone(), event_tx.clone());
                add_login_logs(event_tx.clone(), line.clone());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

fn on_error(error: String, event_tx: Sender<TUIEvent>) {
    event_tx.send(TUIEvent::Error(error)).unwrap();
}

fn open_child_stdout(child: &mut Child) -> ChildStdout {
    child.stdout.take().unwrap()
}

fn open_child_stderr(child: &mut Child) -> ChildStderr {
    child.stderr.take().unwrap()
}

fn open_log_channel(
    stdout: ChildStdout,
    stderr: ChildStderr,
) -> (JoinHandle<()>, Receiver<String>, Receiver<String>) {
    let (read_stdout_tx, read_stdout_rx): (Sender<String>, Receiver<String>) = mpsc::channel();
    let (read_stderr_tx, read_stderr_rx): (Sender<String>, Receiver<String>) = mpsc::channel();
    let mut should_break = false;
    let log_channel_thread: JoinHandle<()> = thread::spawn(move || {
        let stderr_thread = thread::spawn(move || {
            debug!("stderr thread started");
            let mut stderr_buf_reader = BufReader::new(stderr);

            while !should_break {
                let mut text_in_stderr_buf = String::new();
                match stderr_buf_reader.read_line(&mut text_in_stderr_buf) {
                    Err(_) => {
                        debug!("stderr_buf_reader error");
                        should_break = true;
                    }
                    Ok(bytes_read) => {
                        if !text_in_stderr_buf.is_empty() {
                            read_stderr_tx.send(text_in_stderr_buf.clone()).unwrap();
                            debug!("stderr_buf_reader read {:?}", text_in_stderr_buf);
                            should_break = true;
                        } else if bytes_read == 0 {
                            debug!("stderr_buf_reader read 0 bytes");
                            should_break = true;
                        }
                        text_in_stderr_buf.clear()
                    }
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let stdout_thread = thread::spawn(move || {
            debug!("stdout thread started");
            let mut stdout_buf_reader = BufReader::new(stdout);

            while !should_break {
                let mut text_in_stdout_buf = String::new();
                match stdout_buf_reader.read_line(&mut text_in_stdout_buf) {
                    Err(_) => {
                        debug!("stdout_buf_reader error");
                        should_break = true;
                    }
                    Ok(bytes_read) => {
                        if !text_in_stdout_buf.is_empty() {
                            read_stdout_tx.send(text_in_stdout_buf.clone()).unwrap();
                            debug!("stdout_buf_reader read {:?}", text_in_stdout_buf);
                        } else if bytes_read == 0 {
                            debug!("stdout_buf_reader read 0 bytes");
                            should_break = true;
                        }
                        text_in_stdout_buf.clear()
                    }
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        while !should_break {
            thread::sleep(Duration::from_millis(10));
        }
        stdout_thread.join().unwrap();
        stderr_thread.join().unwrap();
    });
    (log_channel_thread, read_stdout_rx, read_stderr_rx)
}

fn check_login_status(line: String, event_tx: Sender<TUIEvent>) {
    let re_code = Regex::new(r"[A-Za-z]{4}-[A-Za-z]{4}").unwrap();
    if let Some(code) = re_code.captures(&line) {
        event_tx
            .send(TUIEvent::DisplayLoginCode(
                code.get(0).unwrap().as_str().to_string(),
            ))
            .unwrap();
    }
    if line.contains("Successfully") {
        event_tx.send(TUIEvent::IsLoggedIn).unwrap();
    }
}

fn add_login_logs(event_tx: Sender<TUIEvent>, line: String) {
    event_tx.send(TUIEvent::AddLoginLog(line)).unwrap();
}

fn add_logs(event_tx: Sender<TUIEvent>, line: String) {
    event_tx.send(TUIEvent::AddLog(line)).unwrap();
}

#[test]
fn test_login() {
    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let child = Command::new("sh")
        .arg("-C")
        .arg("test_res/test_login_succeed.sh") //config
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    thread::spawn(move || login(child, event_tx));

    let check_events = vec![
            TUIEvent::AddLoginLog("Attempting to automatically open the SSO authorization page in your default browser.\n".to_string()),
            TUIEvent::AddLoginLog("If the browser does not open or you wish to use a different device to authorize this request, open the following URL:\n".to_string()),
            TUIEvent::AddLoginLog("\n".to_string()),
            TUIEvent::AddLoginLog("https://device.sso.eu-west-1.amazonaws.com/\n".to_string()),
            TUIEvent::AddLoginLog("\n".to_string()),
            TUIEvent::AddLoginLog("Then enter the code:\n".to_string()),
            TUIEvent::AddLoginLog("\n".to_string()),
            TUIEvent::DisplayLoginCode("MQBJ-XSZB".to_string()),
            TUIEvent::AddLoginLog("MQBJ-XSZB\n".to_string()),
            TUIEvent::IsLoggedIn,
            TUIEvent::AddLoginLog("Successfully\n".to_string())];

    let mut events = vec![];

    while events != check_events {
        if let Ok(event) = event_rx.recv_timeout(Duration::from_millis(10)) {
            events.push(event);
        }
    }

    assert!(events == check_events, "events was: {:?}", events);
}

#[test]
fn test_login_fail() {
    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let child = Command::new("sh")
        .arg("-C")
        .arg("test_res/test_login_fail.sh") //config
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    thread::spawn(move || login(child, event_tx));

    let check_events = vec![TUIEvent::Error("this is an unusual error\n".to_string()),
            TUIEvent::AddLoginLog("Attempting to automatically open the SSO authorization page in your default browser.\n".to_string()),
            TUIEvent::AddLoginLog("If the browser does not open or you wish to use a different device to authorize this request, open the following URL:\n".to_string()),
            TUIEvent::AddLoginLog("\n".to_string()),
            TUIEvent::AddLoginLog("https://device.sso.eu-west-1.amazonaws.com/\n".to_string()),
            TUIEvent::AddLoginLog("\n".to_string()),
            TUIEvent::AddLoginLog("Then enter the code:\n".to_string()),
            TUIEvent::AddLoginLog("\n".to_string()),
            TUIEvent::DisplayLoginCode("MQBJ-XSZB".to_string()),
            TUIEvent::AddLoginLog("MQBJ-XSZB\n".to_string())];

    let mut events = vec![];

    while events != check_events {
        if let Ok(event) = event_rx.recv_timeout(Duration::from_millis(10)) {
            events.push(event);
        }
    }

    assert!(events == check_events, "events was: {:?}", events);
}

#[test]
fn test_get_logs() {
    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let (_, action_rx): (Sender<TUIAction>, Receiver<TUIAction>) = mpsc::channel();
    let child = Command::new("tail")
        .arg("-f") //config
        .arg("test_res/get_logs.txt") //config
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    thread::spawn(move || get_logs(child, event_tx));
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
    let child = Command::new("sh")
        .arg("-C")
        .arg("test_res/check_connectivity.sh")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let mut events = vec![];
    let check_events = vec![TUIEvent::Error("Uhm... VPN on ?".to_string())];
    match child {
        Ok(child) => match wait_for_output_with_timeout(child, event_tx) {
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
    enable_logging();
    let child = Command::new("sh")
        .arg("-C")
        .arg("test_res/check_connectivity_fail.sh")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let mut events: Vec<TUIEvent> = vec![];
    let check_events = vec![TUIEvent::Error("Uhm... VPN on ?".to_string())];
    match child {
        Ok(child) => match wait_for_output_with_timeout(child, event_tx) {
            Ok(_) => {}
            Err(error) => {
                while events.is_empty() {
                    if let Ok(event) = event_rx.recv_timeout(Duration::from_millis(10)) {
                        events.push(event);
                    }
                }
                assert!(events == check_events, "events was: {:?}", events);
                assert!(error == "Exit code 1".to_string(), "error was {:?}", error)
            }
        },
        Err(_) => {}
    }
}

fn enable_logging() {
    let stdout = FileAppender::builder()
        .append(false)
        .build("./logs.txt")
        .unwrap();
    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .build(Root::builder().appender("stdout").build(LevelFilter::Trace))
        .unwrap();
    log4rs::init_config(config).unwrap();
}
