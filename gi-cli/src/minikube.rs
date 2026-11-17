use async_process::{Command, Stdio};
use futures_lite::{io::BufReader, AsyncBufReadExt, StreamExt};

use crate::error::Error;

pub(crate) struct Minikube {}

impl Minikube {
    /// Retrieves nginx controller url
    pub async fn svc_url() -> Result<String, Error> {
        let output = Command::new("minikube")
            .arg("service")
            .arg("-n")
            .arg("ingress-nginx")
            .arg("ingress-nginx-controller")
            .arg("--url")
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Io(e))?
            .stdout;

        if let Some(stdout) = output {
            Ok(BufReader::new(stdout)
                .lines()
                .next()
                .await
                .ok_or(Error::Missing(
                    "minikube service list missing url".to_owned(),
                ))??)
        } else {
            Err(Error::Missing(
                "minikube service list missing url".to_owned(),
            ))
        }
    }
}
