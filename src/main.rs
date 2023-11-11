use crossterm::{
    event::{self, poll, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::{Alignment, Backend, Constraint, CrosstermBackend, Direction, Layout},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use regex::Regex;
use std::{
    error::Error,
    io::{self, Read},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};
use std::{process::Child, str};

#[derive(Clone)]
struct Store {
    pub logged_in: bool,
    pub login_code: Option<String>,
    pub login_log: Option<String>,
    pub logs: Option<String>,
}

impl Store {
    fn new() -> Store {
        Store {
            logged_in: false,
            login_code: None,
            login_log: None,
            logs: None,
        }
    }
}

enum TUIEvent {
    DisplayLoginCode(String),
    IsLoggedIn,
    AddLoginLog(String),
    AddLog(String),
}

enum TUIAction {
    LogIn,
    GetLogs,
}

fn read_stdout_and_send(
    mut child: Child,
    event_tx: Sender<TUIEvent>,
    send: fn(event_tx: Sender<TUIEvent>, line: String),
) {
    let mut buf = [1; 1];

    let mut child_stdout = child.stdout.take().unwrap();
    loop {
        let mut line: Option<String> = None;
        let mut line_construct = String::new();
        while let None = line {
            match child_stdout.read_exact(&mut buf) {
                Ok(_) => (),
                Err(_) => (),
            }
            if let Ok(text_in_buf) = str::from_utf8(&buf) {
                let mut line_check = line_construct.to_string();
                line_check.push_str(&text_in_buf);
                if line_check.contains("\n") {
                    line = Some(line_check.clone().to_string());
                } else if line_check == line_construct {
                    break;
                } else {
                    line_construct = line_check.clone();
                }
            } else {
                line = Some(line_construct.clone().to_string());
            };
        }
        send(event_tx.clone(), line.unwrap())
    }
}

fn add_logs(event_tx: Sender<TUIEvent>, line: String) {
    event_tx.send(TUIEvent::AddLog(line)).unwrap();
}

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (event_tx, event_rx): (Sender<TUIEvent>, Receiver<TUIEvent>) = mpsc::channel();
    let (action_tx, action_rx): (Sender<TUIAction>, Receiver<TUIAction>) = mpsc::channel();
    let (store_tx, store_rx): (Sender<Store>, Receiver<Store>) = mpsc::channel();

    // event thread
    thread::spawn(move || {
        let mut store = Store::new();
        loop {
            match event_rx.try_recv() {
                Ok(event) => match event {
                    TUIEvent::IsLoggedIn => {
                        store.logged_in = true;
                    }
                    TUIEvent::DisplayLoginCode(code) => {
                        store.login_code = Some(code);
                    }
                    TUIEvent::AddLoginLog(log_part) => {
                        if let Some(mut log) = store.login_log {
                            log.push_str(log_part.as_str());
                            store.login_log = Some(log);
                        } else {
                            store.login_log = Some(log_part);
                        }
                    }
                    TUIEvent::AddLog(log_part) => {
                        if let Some(mut log) = store.logs {
                            log.push_str(log_part.as_str());
                            store.logs = Some(log);
                        } else {
                            store.logs = Some(log_part);
                        }
                    }
                },
                Err(_) => {}
            }
            match store_tx.clone().send(store.clone()) {
                Ok(_) => (),
                Err(err) => println!("{}", err),
            }
            thread::sleep(Duration::from_millis(5));
        }
    });

    let action_tx_clone = action_tx.clone();

    // action thread
    thread::spawn(move || {
        let event_tx_clone = event_tx.clone();
        let action_tx_clone = action_tx.clone();
        loop {
            match action_rx.try_recv() {
                Ok(event) => match event {
                    TUIAction::LogIn => {
                        let event_tx_clone = event_tx_clone.clone();
                        let action_tx_clone = action_tx_clone.clone();
                        thread::spawn(move || {
                            let mut should_break = false;
                            let mut output = Command::new("sh")
                                .arg("-C")
                                .arg("./aws_sso_mock.sh")
                                // .arg("sso")
                                // .arg("login")
                                // .arg("--profile")
                                // .arg("eks-non-prod-myccv-lab-developer")
                                .stdout(Stdio::piped())
                                .stderr(Stdio::null())
                                .spawn()
                                .expect("fail");
                            let mut buf = [1; 1];

                            let mut child_stdout = output.stdout.take().unwrap();
                            loop {
                                let mut line: Option<String> = None;
                                let mut line_construct = String::new();
                                while let None = line {
                                    match child_stdout.read_exact(&mut buf) {
                                        Ok(_) => (),
                                        Err(_) => (),
                                    }
                                    if let Ok(text_in_buf) = str::from_utf8(&buf) {
                                        let mut line_check = line_construct.to_string();
                                        line_check.push_str(&text_in_buf);
                                        if line_check.ends_with("\n")
                                            && !line_check.chars().all(|c| c == '\n')
                                        {
                                            line = Some(line_check.clone().to_string());
                                        } else if line_check == line_construct {
                                            should_break = true;
                                            break;
                                        } else {
                                            line_construct = line_check.clone();
                                        }
                                    } else {
                                        should_break = true;
                                        break;
                                    };
                                }
                                let finished_line = line.unwrap();
                                let re_code = Regex::new(r"[A-Za-z]{4}-[A-Za-z]{4}").unwrap();
                                if let Some(code) = re_code.captures(&finished_line) {
                                    event_tx_clone
                                        .send(TUIEvent::DisplayLoginCode(
                                            code.get(0).unwrap().as_str().to_string(),
                                        ))
                                        .unwrap();
                                }
                                if finished_line.contains("Successfully") {
                                    event_tx_clone.send(TUIEvent::IsLoggedIn).unwrap();
                                    action_tx_clone.send(TUIAction::GetLogs).unwrap();
                                }

                                event_tx_clone
                                    .send(TUIEvent::AddLoginLog(finished_line.to_string()))
                                    .unwrap();
                                if should_break {
                                    break;
                                }
                            }
                        });
                    }
                    TUIAction::GetLogs => {
                        let event_tx_clone = event_tx_clone.clone();
                        thread::spawn(move || {
                            // Command::new("aws")
                            //     .arg("eks")
                            //     .arg("--profile")
                            //     .arg("eks-non-prod-myccv-lab-developer")
                            //     .arg("update-kubeconfig")
                            //     .arg("--name")
                            //     .arg("shared-non-prod-2")
                            //     .stdout(Stdio::piped())
                            //     .stderr(Stdio::null())
                            //     .spawn()
                            //     .expect("fail");
                            // let child = Command::new("kubectl")
                            //     .arg("logs")
                            //     .arg("-n")
                            //     .arg("myccv-dev-salespoint")
                            //     .arg("-l")
                            //     .arg("component=salespoint-v2")
                            //     .arg("-c")
                            //     .arg("salespoint-v2")
                            //     .arg("-f")
                            //     .arg("--prefix=true")
                            //     .stdout(Stdio::piped())
                            //     .stderr(Stdio::null())
                            //     .spawn()
                            //     .expect("fail");
                            let child = Command::new("cat")
                                .arg("src/main.rs")
                                .stdout(Stdio::piped())
                                .stderr(Stdio::null())
                                .spawn()
                                .expect("fail");
                            read_stdout_and_send(child, event_tx_clone, add_logs)
                        });
                    }
                },
                Err(_) => {}
            }
        }
    });
    action_tx_clone.send(TUIAction::LogIn)?;

    // create app and run it
    let res = run_app(&mut terminal, &store_rx);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, store_rx: &Receiver<Store>) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, store_rx))?;

        if poll(Duration::from_millis(5))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break Ok(()),
                    _ => {}
                }
            }
        } else {
        }
    }
}

fn ui(f: &mut Frame<'_>, store_rx: &Receiver<Store>) {
    let mut scroll_to: u16 = 0;
    if let Ok(store) = store_rx.try_recv() {
        if let Some(login_log) = &store.login_log {
            let lines = login_log.to_string().matches("\n").count();
            scroll_to = scroll_to + lines as u16;
        } else {
            ()
        };
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Max(1), Constraint::Percentage(90)])
            .split(f.size());
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_layout[1]);
        let lines: u16;
        let height = layout[1].height - 4;
        if height > scroll_to {
            lines = 0;
        } else {
            lines = scroll_to - height;
        }
        f.render_widget(
            Paragraph::new(if store.logged_in {
                Span::styled(
                    "LOGGED IN".to_string(),
                    Style::default().fg(Color::LightGreen),
                )
            } else if let Some(code) = store.login_code {
                Span::styled(code, Style::default().fg(ratatui::style::Color::Yellow))
            } else {
                Span::styled(
                    "busy".to_string(),
                    Style::default().fg(ratatui::style::Color::Red),
                )
            })
            .block(Block::new().borders(Borders::NONE))
            .alignment(Alignment::Right),
            main_layout[0],
        );
        f.render_widget(
            Paragraph::new(if let Some(log) = store.logs {
                log.to_string()
            } else {
                "".to_string()
            })
            .scroll((0, 0))
            .block(Block::new().title("Paragraph").borders(Borders::ALL))
            .style(Style::new().fg(Color::White).bg(Color::Black))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false }),
            layout[0],
        );
        f.render_widget(
            Paragraph::new(if let Some(log) = store.login_log {
                log.to_string()
            } else {
                "".to_string()
            })
            .scroll((lines, 0))
            .block(Block::new().title("Paragraph").borders(Borders::ALL))
            .style(Style::new().bg(Color::White).fg(Color::Black))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false }),
            layout[1],
        );
    };
}
