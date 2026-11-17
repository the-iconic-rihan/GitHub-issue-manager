pub(crate) mod action;
pub(crate) mod cli;
pub(crate) mod cluster;
pub(crate) mod control;
pub(crate) mod error;
pub(crate) mod instruct;
pub(crate) mod minikube;
pub(crate) mod modes;
pub(crate) mod namespace;
pub(crate) mod output;
pub(crate) mod yaml;

use clap::Parser;
use instruct::Instruct;

use crate::{cli::Cli, minikube::Minikube};

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let args = Cli::parse();

    let output = Instruct::instruct(args)
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let svc_url = Minikube::svc_url()
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    println!("Output: {:#?}", output);
    println!("Url: {:#?}", svc_url);

    Ok(())
}
