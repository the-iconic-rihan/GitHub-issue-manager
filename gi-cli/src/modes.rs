#[derive(clap::ValueEnum, Clone, Debug)]
pub(crate) enum Mode {
    Configs,
    Persistence,
    Database,
    Server,
    Website,
}

impl ToString for Mode {
    fn to_string(&self) -> String {
        match self {
            Mode::Configs => "github_issue_manager".to_owned(),
            Mode::Persistence => "persistence".to_owned(),
            Mode::Database => "postgresdb".to_owned(),
            Mode::Server => "server".to_owned(),
            Mode::Website => "website".to_owned(),
        }
    }
}
