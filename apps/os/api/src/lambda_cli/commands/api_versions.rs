//! `a3s api-versions` command - Show available API versions.
//!
//! Usage: a3s api-versions

use crate::commands::Command;
use crate::errors::Result;
use async_trait::async_trait;
use clap::Parser;

/// API versions command.
#[derive(Parser, Debug)]
pub struct ApiVersionsCommand {
    /// Show raw API versions (no formatting).
    #[arg(short, long)]
    raw: bool,
}

/// List of API groups and their versions.
struct ApiGroup {
    name: &'static str,
    versions: &'static [&'static str],
}

const API_GROUPS: &[ApiGroup] = &[
    ApiGroup {
        name: "apps",
        versions: &["v1"],
    },
    ApiGroup {
        name: "autoscaling",
        versions: &["v1", "v2"],
    },
    ApiGroup {
        name: "batch",
        versions: &["v1", "v1beta1"],
    },
    ApiGroup {
        name: "core",
        versions: &["v1"],
    },
    ApiGroup {
        name: "networking.a3s.io",
        versions: &["v1"],
    },
    ApiGroup {
        name: "rbac.authorization.a3s.io",
        versions: &["v1"],
    },
    ApiGroup {
        name: "storage.a3s.io",
        versions: &["v1"],
    },
    ApiGroup {
        name: "apiextensions.a3s.io",
        versions: &["v1", "v1beta1"],
    },
    ApiGroup {
        name: "events.a3s.io",
        versions: &["v1", "v1beta1"],
    },
];

pub struct ApiVersions;

#[async_trait]
impl Command for ApiVersionsCommand {
    async fn run(&self) -> Result<()> {
        if self.raw {
            // Print raw version strings
            for group in API_GROUPS {
                for version in group.versions {
                    if group.name == "core" {
                        println!("v1");
                    } else {
                        println!("{}/{}", group.name, version);
                    }
                }
            }
        } else {
            // Print grouped format
            println!("admissionregistration.a3s.io/v1");
            println!("admissionregistration.a3s.io/v1beta1");
            println!("apiextensions.a3s.io/v1");
            println!("apiextensions.a3s.io/v1beta1");
            println!("apiregistration.a3s.io/v1");
            println!("apiregistration.a3s.io/v1beta1");
            println!("apps/v1");
            println!("auth.openshift.io/v1");
            println!("autoscaling/v1");
            println!("autoscaling/v2");
            println!("batch/v1");
            println!("batch/v1beta1");
            println!("certificates.a3s.io/v1");
            println!("certificates.a3s.io/v1beta1");
            println!("coordination.a3s.io/v1");
            println!("coordination.a3s.io/v1beta1");
            println!("core/v1");
            println!("events.a3s.io/v1");
            println!("events.a3s.io/v1beta1");
            println!("networking.a3s.io/v1");
            println!("networking.a3s.io/v1beta1");
            println!("node.a3s.io/v1");
            println!("node.a3s.io/v1beta1");
            println!("policy/v1");
            println!("policy/v1beta1");
            println!("rbac.authorization.a3s.io/v1");
            println!("rbac.authorization.a3s.io/v1beta1");
            println!("rbac.authorization.a3s.io/v1");
            println!("scheduling.a3s.io/v1");
            println!("scheduling.a3s.io/v1beta1");
            println!("storage.a3s.io/v1");
            println!("storage.a3s.io/v1beta1");
            println!("storage.a3s.io/v1");
        }

        Ok(())
    }
}
