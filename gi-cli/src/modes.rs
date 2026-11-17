#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Configs,
    Persistence,
    Database,
    Server,
    Website,
}
