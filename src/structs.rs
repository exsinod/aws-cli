use std::{collections::HashMap, sync::mpsc::Sender};

use crate::widgets::{BodyWidget, CliWidgetId, HeaderWidget};

pub const DEV: KubeEnvData =
    KubeEnvData::new("eks-non-prod-myccv-lab-developer", "shared-non-prod-2");
pub const PROD: KubeEnvData = KubeEnvData::new("eks-prod-myccv-lab-developer", "shared-prod-2");

pub struct KubeEnvData<'a> {
    pub profile: &'a str,
    pub environment: &'a str,
}

impl<'a> KubeEnvData<'a> {
    pub const fn new(profile: &'a str, environment: &'a str) -> Self {
        KubeEnvData {
            profile,
            environment,
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct Store {
    pub ui_state: UIState,
    pub request_login: bool,
    pub logged_in: bool,
    pub env_change_possible: bool,
    pub login_code: Option<String>,
    pub header_widget: Option<HeaderWidget>,
    pub login_widget: Option<BodyWidget>,
    pub logs_widget: Option<BodyWidget>,
    pub pods_widget: Option<BodyWidget>,
    pub tail_widget: Option<BodyWidget>,
}

impl Store {
    pub fn new(
        header_widget: HeaderWidget,
        login_widget: BodyWidget,
        logs_widget: BodyWidget,
        pods_widget: BodyWidget,
        tail_widget: BodyWidget,
    ) -> Store {
        Store {
            ui_state: UIState::LoggingIn,
            request_login: false,
            logged_in: false,
            env_change_possible: false,
            login_code: None,
            header_widget: Some(header_widget),
            login_widget: Some(login_widget),
            logs_widget: Some(logs_widget),
            pods_widget: Some(pods_widget),
            tail_widget: Some(tail_widget),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct CliWidgetData {
    pub id: CliWidgetId,
    pub thread_started: bool,
    pub initiate_thread: Option<fn(action_tx: &Sender<TUIAction>)>,
    pub data: HashMap<String, Option<Vec<String>>>,
}

impl CliWidgetData {
    pub fn new(id: CliWidgetId) -> Self {
        CliWidgetData {
            id,
            thread_started: false,
            initiate_thread: None,
            data: HashMap::default(),
        }
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub enum UIState {
    #[default]
    Init,
    UserInput,
    LoggingIn,
    LoggedIn,
}

#[derive(Debug, PartialEq)]
pub enum TUIEvent {
    Error(TUIError),
    CheckConnectivity,
    ClearError,
    RequestLoginStart,
    RequestLoginStop,
    RequestEnvChange,
    EnvChange(KubeEnv),
    NeedsLogin,
    DisplayLoginCode(String),
    IsLoggedIn,
    IsConnected,
    AddLoginLog(String),
    AddLog(String),
    AddPods(String),
    AddTailLog(String),
}

#[derive(Debug, PartialEq)]
pub enum TUIError {
    VPN,
    KEY(String),
    API(String),
}

#[derive(Debug, PartialEq)]
pub enum TUIAction {
    CheckConnectivity,
    LogIn,
    ChangeEnv(KubeEnv),
    GetLogs,
    GetPods,
    GetTail,
}

#[derive(Debug, PartialEq)]
pub enum UserInput {
    Quit,
    ChangeEnv,
    Direction(Direction2),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Direction2 {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Debug, PartialEq)]
pub enum KubeEnv {
    Dev,
    Prod,
}
