use std::path::{Path, PathBuf};

use crate::{error::Error, modes::Mode};

const YAMLS: [&'static str; 14] = [
    "github_issue_manager/github_issue_manager-client-side-networkpolicy.yaml",
    "github_issue_manager/github_issue_manager-ingress.yaml",
    "github_issue_manager/github_issue_manager-persistentvolumeclaim.yaml",
    "github_issue_manager/github_issue_manager-secret.yaml",
    "persistence/github_issue_manager-persistentvolume.yaml",
    "persistence/github_issue_manager-storageclass.yaml",
    "postgresdb/postgresdb-configmap.yaml",
    "postgresdb/postgresdb-service.yaml",
    "postgresdb/postgresdb-statefulset.yaml",
    "server/server-deployment.yaml",
    "server/server-service.yaml",
    "server/server-side-networkpolicy.yaml",
    "website/website-deployment.yaml",
    "website/website-service.yaml",
];

pub(crate) fn yamls(mode: Mode) -> Vec<String> {
    YAMLS
        .into_iter()
        .filter(|s| s.starts_with(&mode.to_string()))
        .map(str::to_owned)
        .collect()
}

/// Create path for yaml
pub(crate) fn path(base: &str, yaml: &str) -> PathBuf {
    Path::new(base).join(yaml)
}

/// Create paths for yamls
pub(crate) fn paths(base: &str, yamls: Vec<String>) -> Vec<PathBuf> {
    yamls.iter().map(|yaml| path(base, yaml)).collect()
}

/// Ensure that all paths exist
pub(crate) fn ensure(config_yamls: &Vec<PathBuf>) -> Result<(), Error> {
    for yaml in config_yamls {
        if !yaml.exists() {
            return Err(Error::Missing(format!("{:#?} is missing", yaml.as_path())));
        }
    }

    Ok(())
}
