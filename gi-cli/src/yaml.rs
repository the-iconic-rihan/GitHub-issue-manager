use std::path::{Path, PathBuf};

use crate::{error::Error, modes::Mode};

/// Manifests (relative to the `k8s/` directory) applied for each mode. The Secret is not
/// listed: it is created from `k8s/githubissuemanager-secret.env.example` (see README).
pub(crate) fn yamls(mode: Mode) -> Vec<String> {
    let files: &[&str] = match mode {
        Mode::Configs => &[
            "00-namespace.yaml",
            "setting/githubissuemanager-configmap.yaml",
            "setting/githubissuemanager-ingress.yaml",
        ],
        Mode::Persistence => &["setting/githubissuemanager-persistentvolumeclaim.yaml"],
        Mode::Database => &[
            "database/postgresdb-service.yaml",
            "database/postgresdb-statefulset.yaml",
            "database/postgresdb-networkpolicy.yaml",
        ],
        Mode::Server => &[
            "server/server-service.yaml",
            "server/server-deployment.yaml",
            "server/server-side-networkpolicy.yaml",
        ],
        Mode::Website => &[
            "website/website-service.yaml",
            "website/website-deployment.yaml",
            "website/website-networkpolicy.yaml",
        ],
    };
    files.iter().map(|s| (*s).to_owned()).collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_manifest_exists_in_the_repository() {
        let k8s = concat!(env!("CARGO_MANIFEST_DIR"), "/../k8s");
        for mode in [
            Mode::Configs,
            Mode::Persistence,
            Mode::Database,
            Mode::Server,
            Mode::Website,
        ] {
            ensure(&paths(k8s, yamls(mode))).unwrap_or_else(|e| panic!("{mode:?}: {e}"));
        }
    }
}
