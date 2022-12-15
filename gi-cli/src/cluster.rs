use async_process::{Command, Output};

use crate::{
    action::ClusterAction,
    cli::ClusterOpts,
    error::Error,
    modes::Mode,
    namespace::Namespace,
    yaml::{ensure, paths, yamls},
};

pub(crate) struct Cluster {}

impl Cluster {
    /// `github_issue_manager` namespace actions
    async fn namespace(namespace: &Namespace) -> Result<Output, Error> {
        Command::new("kubectl")
            .arg(namespace.to_string())
            .arg("namespace")
            .arg("github_issue_manager")
            .spawn()
            .map_err(|e| Error::Io(e))?
            .output()
            .await
            .map_err(|e| Error::Io(e))
    }

    async fn manual(
        mode: Mode,
        action: &ClusterAction,
        dry_run: bool,
        path: &String,
    ) -> Result<Vec<Output>, Error> {
        let yamls = paths(path, yamls(mode));

        ensure(&yamls)?;

        let paths = yamls
            .iter()
            .map(|p| p.to_str())
            .collect::<Option<Vec<_>>>()
            // `ensure` ensures all paths exist
            .unwrap()
            .join(",");

        let mut command = Command::new("kubectl");

        command.arg(action.to_string()).arg("-f").arg(&paths);

        if dry_run {
            command.arg(if dry_run { "--dry-run=client" } else { "" });
        }

        Ok(vec![command
            .spawn()
            .map_err(|e| Error::Io(e))?
            .output()
            .await
            .map_err(|e| Error::Io(e))?])
    }

    /// Bootstraps clusters.
    async fn bootstrap(path: &String, action: &ClusterAction) -> Result<Vec<Output>, Error> {
        Ok(vec![
            Self::manual(Mode::Configs, action, false, path).await?,
            Self::manual(Mode::Persistence, action, false, path).await?,
            Self::manual(Mode::Database, action, false, path).await?,
            Self::manual(Mode::Server, action, false, path).await?,
            Self::manual(Mode::Website, action, false, path).await?,
        ]
        .into_iter()
        .flatten()
        .collect())
    }

    /// Reboots non-config, non-persistence clusters.
    /// Assumes namespace has been handled.
    async fn reboot(path: &String, action: &ClusterAction) -> Result<Vec<Output>, Error> {
        Ok(vec![
            Self::manual(Mode::Database, action, false, path).await?,
            Self::manual(Mode::Server, action, false, path).await?,
            Self::manual(Mode::Website, action, false, path).await?,
        ]
        .into_iter()
        .flatten()
        .collect())
    }

    pub async fn cluster(args: ClusterOpts) -> Result<Vec<Output>, Error> {
        match args {
            ClusterOpts::Manual {
                mode,
                action,
                dry_run,
                path,
                namespace,
            } => {
                // Namespace is FILO
                //
                // If action is to create,
                // namespace needs to be applied first.
                if let Some(Namespace::Create) = namespace {
                    Self::namespace(&namespace.as_ref().unwrap()).await?;
                }

                let output = Self::manual(mode, &action, dry_run, &path).await;

                // If action is to delete,
                // namespace needs to go last.
                if let Some(Namespace::Delete) = namespace {
                    Self::namespace(&namespace.as_ref().unwrap()).await?;
                }

                output
            }
            ClusterOpts::Bootstrap { path, action } => {
                // Derives namespace action from bootstrap action
                // Namespace is FILO
                //
                // If action is to create,
                // namespace needs to be applied first.
                if let ClusterAction::Apply = &action {
                    Self::namespace(&Namespace::Create).await?;
                    Self::bootstrap(&path, &action).await
                } else {
                    // Deleting namespace should clear all resources.
                    let output = Self::namespace(&Namespace::Delete).await?;
                    Ok(vec![output])
                }
            }
            ClusterOpts::Reboot { path, action } => Self::reboot(&path, &action).await,
        }
    }
}
