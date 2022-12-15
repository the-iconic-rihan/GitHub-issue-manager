#[derive(Debug)]
pub(crate) struct Output {
    cluster: Option<Vec<async_process::Output>>,
    control: Option<reqwest::Response>,
}

impl From<Vec<async_process::Output>> for Output {
    fn from(cluster: Vec<async_process::Output>) -> Self {
        Self {
            cluster: Some(cluster),
            control: None,
        }
    }
}

impl From<reqwest::Response> for Output {
    fn from(control: reqwest::Response) -> Self {
        Self {
            cluster: None,
            control: Some(control),
        }
    }
}
