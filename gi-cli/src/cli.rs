use clap::{Parser, Subcommand};

use crate::{
    action::{ClusterAction, CommanderAction},
    modes::Mode,
    namespace::Namespace,
};

/// Github_Issue_Manager CLI
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
#[clap(name = "gi-cli")]
#[clap(about = "Github_Issue_Manager cli tool", long_about = None)]
pub(crate) enum Cli {
    #[clap(subcommand)]
    #[clap(about = "Cluster deployment subcommand")]
    Cluster(ClusterOpts),
    #[clap(subcommand)]
    #[clap(about = "Server control panel interface")]
    Control(ControlOpts),
}

#[derive(Debug, Subcommand)]
pub(crate) enum ClusterOpts {
    #[clap(about = "To configure manually")]
    Manual {
        /// K8s modes
        #[clap(short, long, value_enum)]
        mode: Mode,

        /// apply or delete action
        #[clap(short, long, value_enum)]
        action: ClusterAction,

        /// dry-run flag for modes
        #[clap(short, long, action)]
        dry_run: bool,

        /// k8s folder path
        #[clap(short, long, value_parser)]
        path: String,

        /// Namespace actions
        #[clap(short, long, value_enum)]
        namespace: Option<Namespace>,
    },

    #[clap(about = "To bootstrap cluster with defaults")]
    Bootstrap {
        /// k8s folder path
        #[clap(short, long, value_parser)]
        path: String,

        /// apply or delete action
        #[clap(short, long, value_enum)]
        action: ClusterAction,
    },

    #[clap(about = "To reboot application cluster, not persistence and configs")]
    Reboot {
        /// k8s folder path
        #[clap(short, long, value_parser)]
        path: String,

        /// apply or delete action
        #[clap(short, long, value_enum)]
        action: ClusterAction,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ControlOpts {
    #[clap(about = "Commander control panel interface")]
    Commander {
        #[clap(short, long, value_enum)]
        commander_action: CommanderAction,
        #[clap(short, long, value_parser)]
        limit: u16,
    },

    #[clap(about = "Administrator control panel interface")]
    Admin {
        #[clap(long, value_enum)]
        commander_action: CommanderAction,
        #[clap(long, value_parser, default_value_t = 100)]
        limit: u16,
    },
}
