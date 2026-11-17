use crate::{cli::Cli, cluster::Cluster, control::Control, error::Error, output::Output};

pub(crate) struct Instruct {}

impl Instruct {
    pub async fn instruct(args: Cli) -> Result<Output, Error> {
        match args {
            Cli::Cluster(cluster_opts) => {
                let output = Cluster::cluster(cluster_opts).await?;
                Ok(output.into())
            }

            Cli::Control(control_opts) => {
                let output = Control::control(control_opts).await?;
                Ok(output.into())
            }
        }
    }
}
