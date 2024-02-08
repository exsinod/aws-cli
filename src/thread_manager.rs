use std::io::BufRead;
use std::sync::{Arc, Mutex};
use std::{
    collections::HashMap,
    io::BufReader,
    process::{Child, ChildStderr, ChildStdout},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use log::{debug, trace};

use crate::aws_api::{AwsAPIHandler, IOEventSender};
use crate::structs::TUIEvent;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum WidgetTaskId {
    CheckConnectivity,
    GetLoginLogs,
    GetLogs,
    GetPods,
}

pub struct ThreadManager<'a> {
    test: HashMap<WidgetTaskId, Arc<Mutex<bool>>>,
    event_tx: &'a Sender<TUIEvent>,
    threads: HashMap<WidgetTaskId, JoinHandle<()>>,
}

impl<'a> IOEventSender<TUIEvent> for ThreadManager<'a> {
    fn event_tx(&self) -> &Sender<TUIEvent> {
        self.event_tx
    }
}

impl<'a> ThreadManager<'a> {
    pub fn new(event_tx: &'a Sender<TUIEvent>) -> Self {
        ThreadManager {
            test: HashMap::default(),
            event_tx,
            threads: HashMap::default(),
        }
    }

    pub fn stop_threads(&mut self) {
        for a in self.test.values_mut() {
            debug!("setting to {:?}", a);
            *a.lock().unwrap() = true;
        }
    }

    pub fn run_thread(
        &mut self,
        id: WidgetTaskId,
        mut child: Child,
        success_fn: fn(&str, &AwsAPIHandler),
        error_fn: fn(&str, &AwsAPIHandler),
        aws_api_handler: AwsAPIHandler,
    ) {
        if let None = self.threads.get(&id) {
            let stop_thread = Arc::new(Mutex::new(false));
            let id_to_insert = id.clone();
            self.test.insert(id, Arc::clone(&stop_thread));
            let child_stdout = self.open_child_stdout(&mut child);
            let child_stderr = self.open_child_stderr(&mut child);
            let (thread_handle, read_stdout_rx, read_stderr_rx) =
                self.open_log_channel(child_stdout, child_stderr);
            let join_handle = thread::spawn(move || {
                while !thread_handle.is_finished() || *stop_thread.lock().unwrap() {
                    debug!("stop thread is {:?}", *stop_thread);
                    if let Ok(error) = read_stderr_rx.recv_timeout(Duration::from_millis(10)) {
                        error_fn(&error, &aws_api_handler)
                        // aws_ap.on_error(&error);
                    }
                    if let Ok(line) = read_stdout_rx.recv_timeout(Duration::from_millis(10)) {
                        success_fn(&line, &aws_api_handler)
                        // aws_ap.add_login_logs(&line);
                        // aws_ap.check_login_status(&line);
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            });
            self.threads.insert(id_to_insert.clone(), join_handle);
        } else {
            debug!("ignoring, thread {:?} already running", id);
        }
    }

    pub fn run_thread_timeout(
        &mut self,
        id: WidgetTaskId,
        mut child: Child,
        success_fn: fn(&str, &AwsAPIHandler),
        error_fn: fn(&str, &AwsAPIHandler),
        aws_api_handler: AwsAPIHandler,
    ) {
        if let None = self.threads.get(&id) {
            let stop_thread = Arc::new(Mutex::new(false));
            let id_to_insert = id.clone();
            self.test.insert(id, stop_thread.clone());
            let child_stdout = self.open_child_stdout(&mut child);
            let child_stderr = self.open_child_stderr(&mut child);
            let (thread_handle, read_stdout_rx, read_stderr_rx) =
                self.open_log_channel(child_stdout, child_stderr);
            let join_handle = thread::spawn(move || {
                let mut has_error = false;
                while !thread_handle.is_finished() && !*stop_thread.lock().unwrap() {
                    if let Ok(error) = read_stderr_rx.recv_timeout(Duration::from_millis(10)) {
                        error_fn(&error, &aws_api_handler);
                        has_error = true;
                    }
                    if let Ok(line) = read_stdout_rx.recv_timeout(Duration::from_millis(10)) {
                        success_fn(&line, &aws_api_handler);
                    }
                }
                if !child.wait().unwrap().success() || has_error {
                    debug!("child had errors");
                    error_fn("process experienced some errors", &aws_api_handler);
                } else {
                    debug!("child had no errors");
                }
            });
            self.threads.insert(id_to_insert, join_handle);
        } else {
            debug!("ignoring, thread {:?} already running", id);
        }
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
                                debug!("stderr_buf_reader read {:?}", text_in_stderr_buf);
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
