use serde::Serialize;

#[derive(clap::ValueEnum, Clone, Debug)]
pub(crate) enum ClusterAction {
    Apply,
    Delete,
}

impl ToString for ClusterAction {
    fn to_string(&self) -> String {
        match self {
            ClusterAction::Apply => "apply".to_owned(),
            ClusterAction::Delete => "delete".to_owned(),
        }
    }
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CommanderAction {
    Insert,
    Update,
}

impl ToString for CommanderAction {
    fn to_string(&self) -> String {
        match self {
            CommanderAction::Insert => "insert".to_owned(),
            CommanderAction::Update => "update".to_owned(),
        }
    }
}
