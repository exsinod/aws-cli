use std::io::{BufRead, BufReader};
use std::time::Duration;
use std::{
    process::{Child, ChildStdout, Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use regex::Regex;

use crate::{TUIAction, TUIEvent};

pub fn action_thread(
    event_tx: Sender<TUIEvent>,
    action_rx: Receiver<TUIAction>,
    action_tx: Sender<TUIAction>,
) {
    let event_tx_clone = event_tx.clone();
    let action_tx_clone = action_tx.clone();
    while let Ok(event) = action_rx.recv() {
        match event {
            TUIAction::LogIn => {
                let event_tx_clone = event_tx_clone.clone();
                let action_tx_clone = action_tx_clone.clone();
                thread::spawn(move || {
                    // let output = Command::new("sh")
                    // .arg("-C")
                    // .arg("./aws_sso_mock.sh")
                    let output = Command::new("aws")
                        .arg("sso")
                        .arg("login")
                        .arg("--profile")
                        .arg("eks-non-prod-myccv-lab-developer")
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .spawn()
                        .expect("fail");
                    read_stdout_check_and_send(
                        output,
                        event_tx_clone.clone(),
                        action_tx_clone.clone(),
                        check_login_status,
                        add_login_logs,
                    )
                });
            }
            TUIAction::GetLogs => {
                let action_tx_clone = action_tx.clone();
                let event_tx_clone = event_tx_clone.clone();
                thread::spawn(move || {
                    Command::new("aws")
                        .arg("eks")
                        .arg("--profile")
                        .arg("eks-non-prod-myccv-lab-developer")
                        .arg("update-kubeconfig")
                        .arg("--name")
                        .arg("shared-non-prod-2")
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .spawn()
                        .expect("fail");
                    if let Ok(output) = Command::new("kubectl")
                        .arg("logs")
                        .arg("-n")
                        .arg("myccv-dev-salespoint")
                        .arg("-l")
                        .arg("component=salespoint-v2")
                        .arg("-c")
                        .arg("salespoint-v2")
                        .arg("-f")
                        .arg("--prefix=true")
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .spawn()
                    {
                        event_tx_clone.clone().send(TUIEvent::IsLoggedIn).unwrap();
                        read_stdout_and_send(output, event_tx_clone, add_logs)
                    } else {
                        action_tx_clone.clone().send(TUIAction::LogIn).unwrap()
                    }
                    // let child = Command::new("cat")
                    //     .arg("src/main.rs")
                    //     .stdout(Stdio::piped())
                    //     .stderr(Stdio::null())
                    //     .spawn()
                    //     .expect("fail");
                });
            }
        }
    }
}

fn split_on_new_line(line: String) -> (Option<Vec<String>>, Option<String>) {
    if let Some((first, last)) = line.rsplit_once("\n") {
        let split_line: Vec<String> = first.split("\n").map(|l| l.to_string() + "\n").collect();
        (Some(split_line), Some(last.to_string()))
    } else {
        (None, None)
    }
}

fn get_log_iterator(stdout: ChildStdout) -> Receiver<String> {
    let (read_tx, read_rx): (Sender<String>, Receiver<String>) = mpsc::channel();
    thread::spawn(move || {
        let mut buf_reader = BufReader::new(stdout);

        let mut line_construct = String::new();
        loop {
            let mut text_in_buf = String::new();
            if let Ok(0) = buf_reader.read_line(&mut text_in_buf) {
                thread::sleep(Duration::from_secs(2))
            } else {
                let mut line_check = line_construct.to_string();
                line_check.push_str(&text_in_buf);
                let splits = split_on_new_line(line_check.clone());
                if let (Some(new_lines), Some(rest)) = splits.clone() {
                    line_construct = rest.clone();
                    for nline in new_lines {
                        read_tx.send(nline).unwrap();
                    }
                } else {
                    line_construct = line_check;
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
    });
    read_rx
}

fn read_stdout_check_and_send(
    child: Child,
    event_tx: Sender<TUIEvent>,
    action_tx: Sender<TUIAction>,
    check: fn(line: String, event_tx: Sender<TUIEvent>, action_tx: Sender<TUIAction>),
    send: fn(event_tx: Sender<TUIEvent>, line: String),
) {
    let rx = get_log_iterator(child.stdout.unwrap());
    loop {
        if let Ok(line) = rx.recv() {
            check(line.clone(), event_tx.clone(), action_tx.clone());
            send(event_tx.clone(), line.clone())
        }
    }
}

fn read_stdout_and_send(
    child: Child,
    event_tx: Sender<TUIEvent>,
    send: fn(event_tx: Sender<TUIEvent>, line: String),
) {
    let rx = get_log_iterator(child.stdout.unwrap());
    loop {
        if let Ok(line) = rx.recv() {
            send(event_tx.clone(), line.clone())
        }
    }
}

fn check_login_status(line: String, event_tx: Sender<TUIEvent>, action_tx: Sender<TUIAction>) {
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
        action_tx.send(TUIAction::GetLogs).unwrap();
    }
}

fn add_login_logs(event_tx: Sender<TUIEvent>, line: String) {
    event_tx.send(TUIEvent::AddLoginLog(line)).unwrap();
}

fn add_logs(event_tx: Sender<TUIEvent>, line: String) {
    event_tx.send(TUIEvent::AddLog(line)).unwrap();
}
