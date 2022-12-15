use reqwest::{Client, RequestBuilder, Response};
use serde::Serialize;

use crate::{action::CommanderAction, cli::ControlOpts, error::Error, minikube::Minikube};

pub(crate) struct Control {
    request: RequestBuilder,
}

impl Control {
    pub async fn control(args: ControlOpts) -> Result<Response, Error> {
        let control = Self::new().await?;

        match args {
            ControlOpts::Commander {
                commander_action,
                limit,
            } => control.request(&commander_action, limit).await,
            ControlOpts::Admin {
                commander_action,
                limit,
            } => control.request(&commander_action, limit).await,
        }
    }

    async fn new() -> Result<Self, Error> {
        dotenv::dotenv().ok();

        let svc_url = Minikube::svc_url().await?;

        let url = reqwest::Url::parse(&svc_url)
            .unwrap()
            .join("api/v1/command")
            .unwrap();

        let authorization = std::env::var("COMMANDER_ACCESS_TOKEN").map_err(|e| Error::Var(e))?;

        let request = Client::new().post(url).bearer_auth(authorization);

        Ok(Self { request })
    }

    async fn request(&self, action: &CommanderAction, limit: u16) -> Result<Response, Error> {
        #[derive(Serialize)]
        struct Request<'r> {
            action: &'r CommanderAction,
            target: &'r str,
            limit: u16,
        }

        let request = Request {
            action,
            limit,
            target: "issues",
        };

        let response = self
            .request
            .try_clone()
            .ok_or(Error::Other("Cloning request builder failed.".to_owned()))?
            .json(&request)
            .send()
            .await?;

        Ok(response)
    }
}
