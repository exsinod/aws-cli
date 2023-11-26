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

#[derive(Clone, Debug)]
pub struct Store {
    pub request_login: bool,
    pub logged_in: bool,
    pub env_change_possible: bool,
    pub login_code: Option<String>,
    pub header_widget: Option<HeaderWidget>,
    pub login_widget: Option<BodyWidget>,
    pub logs_widget: Option<BodyWidget>,
    pub pods_widget: Option<BodyWidget>,
}

impl Store {
    pub fn new(
        header_widget: HeaderWidget,
        login_widget: BodyWidget,
        body_widget: BodyWidget,
        pods_widget: BodyWidget,
    ) -> Store {
        Store {
            request_login: false,
            logged_in: false,
            env_change_possible: false,
            login_code: None,
            header_widget: Some(header_widget),
            login_widget: Some(login_widget),
            logs_widget: Some(body_widget),
            pods_widget: Some(pods_widget),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct CliWidgetData {
    pub id: CliWidgetId,
    pub thread_started: bool,
    pub initiate_thread: Option<fn(action_tx: Sender<TUIAction>)>,
    pub data: HashMap<String, Option<String>>,
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

#[derive(Debug, PartialEq)]
pub enum TUIEvent {
    Error(TUIError),
    CheckConnectivity,
    ClearError,
    RequestLoginStart,
    RequestLoginStop,
    RequestEnvChange,
    EnvChange(KubeEnv),
    RequestLoginInput(String),
    Input(String),
    NeedsLogin,
    DisplayLoginCode(String),
    IsLoggedIn,
    IsConnected,
    AddLoginLog(String),
    LogThreadStarted(CliWidgetId),
    LogThreadStopped(CliWidgetId),
    AddLog(String),
    AddPods(String),
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
