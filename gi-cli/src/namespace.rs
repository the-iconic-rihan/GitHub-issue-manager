#[derive(clap::ValueEnum, Clone, Debug)]
pub(crate) enum Namespace {
    Create,
    Delete,
}

impl ToString for Namespace {
    fn to_string(&self) -> String {
        match self {
            Namespace::Create => "create".to_owned(),
            Namespace::Delete => "delete".to_owned(),
        }
    }
}
