use std::{collections::HashMap, sync::mpsc::Sender};

use crate::{
    app::DataStream,
    widgets::{BodyWidget, CliWidgetId, ErrorActionWidget, HeaderWidget},
};

pub const DEV: KubeEnvData = KubeEnvData::new(
    "eks-non-prod-myccv-lab-developer",
    "myccv-lab-non-prod-myccv-lab-developer",
    "shared-non-prod-2",
    "myccv-dev-salespoint",
);
pub const TEST: KubeEnvData = KubeEnvData::new(
    "eks-non-prod-myccv-lab-developer",
    "myccv-lab-non-prod-myccv-lab-developer",
    "shared-non-prod-2",
    "myccv-test-salespoint",
);
pub const _DEMO: KubeEnvData = KubeEnvData::new(
    "eks-prod-myccv-lab-developer",
    "myccv-lab-non-prod-myccv-lab-developer",
    "shared-prod-2",
    "myccv-demo-salespoint",
);
pub const PROD: KubeEnvData = KubeEnvData::new(
    "eks-prod-myccv-lab-developer",
    "myccv-lab-prod-myccv-lab-developer",
    "shared-prod-2",
    "myccv-salespoint",
);

#[derive(Clone, Default, Debug)]
pub struct KubeEnvData<'a> {
    pub eks_profile: &'a str,
    pub aws_profile: &'a str,
    pub environment: &'a str,
    pub namespace: &'a str,
}

impl<'a> KubeEnvData<'a> {
    pub const fn new(
        eks_profile: &'a str,
        aws_profile: &'a str,
        environment: &'a str,
        namespace: &'a str,
    ) -> Self {
        KubeEnvData {
            eks_profile,
            aws_profile,
            environment,
            namespace,
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
    pub request_login_widget: Option<ErrorActionWidget>,
}

impl Store {
    pub fn new(
        header_widget: HeaderWidget,
        login_widget: BodyWidget,
        logs_widget: BodyWidget,
        pods_widget: BodyWidget,
        request_login_widget: ErrorActionWidget,
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
            request_login_widget: Some(request_login_widget),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct CliWidgetData {
    pub id: CliWidgetId,
    pub data_stream: DataStream,
    pub thread_started: bool,
    pub initiate_thread: Option<fn(action_tx: &Sender<TUIAction>)>,
    pub data: HashMap<String, Option<Vec<String>>>,
}

impl CliWidgetData {
    pub fn new(id: CliWidgetId, data_stream: DataStream) -> Self {
        CliWidgetData {
            id,
            data_stream,
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
    Test,
    Prod,
}
