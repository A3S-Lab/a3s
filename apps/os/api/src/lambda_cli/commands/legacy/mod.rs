//! Legacy commands for backward compatibility.
//!
//! These commands use the old A3sfile format.

pub mod deploy;
pub mod init;
pub mod list;
pub mod status;
pub mod task;
pub mod undeploy;

pub use deploy::DeployCommand;
pub use init::InitCommand;
pub use list::ListCommand;
pub use status::StatusCommand;
pub use task::TaskCommands;
pub use undeploy::UndeployCommand;
