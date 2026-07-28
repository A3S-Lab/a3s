use std::path::{Path, PathBuf};

use crate::tui::panels;

use super::AssetCommandContext;

pub(crate) fn resolve_agent_dev(
    path_arg: Option<PathBuf>,
    context: &AssetCommandContext,
) -> anyhow::Result<panels::agent::AgentDevSession> {
    let config_root = &context.directories.agent;
    let target = resolve_optional_target(path_arg, context)?;
    let file = choose_agent_file(&target)?;
    let root = asset_root_for_file(config_root, &file);
    panels::agent::agent_dev_session_from_file(&root, &file).map_err(anyhow::Error::msg)
}

pub(super) fn resolve_mcp_dev(
    path_arg: Option<PathBuf>,
    context: &AssetCommandContext,
) -> anyhow::Result<panels::mcp::McpDevSession> {
    let config_root = &context.directories.mcp;
    let target = asset_dir_target(resolve_optional_target(path_arg, context)?);
    let project = choose_mcp_project(&target, config_root)?;
    Ok(panels::mcp::McpDevSession {
        name: project.name,
        description: project.description,
        rel: project.rel,
        path: project.path,
        root: asset_root_for_dir(config_root, &target),
    })
}

pub(super) fn resolve_skill_dev(
    path_arg: Option<PathBuf>,
    context: &AssetCommandContext,
) -> anyhow::Result<panels::skill::SkillDevSession> {
    let config_root = &context.directories.skill;
    let target = resolve_optional_target(path_arg, context)?;
    let file = choose_skill_file(&target)?;
    let root = asset_root_for_file(config_root, &file);
    Ok(panels::skill::skill_dev_session_from_file(&root, &file))
}

#[derive(Debug, Clone)]
pub(super) struct FlowFile {
    pub(super) rel: String,
    pub(super) path: PathBuf,
}

pub(super) fn resolve_flow_file(
    path_arg: Option<PathBuf>,
    context: &AssetCommandContext,
) -> anyhow::Result<FlowFile> {
    let config_root = &context.directories.flow;
    let target = resolve_optional_target(path_arg, context)?;
    let file = choose_flow_file(&target)?;
    let root = asset_root_for_file(config_root, &file);
    let rel = rel_to_root(&root, &file);
    Ok(FlowFile { rel, path: file })
}

pub(crate) fn resolve_okf_dev(
    path_arg: Option<PathBuf>,
    context: &AssetCommandContext,
) -> anyhow::Result<panels::okf::OkfDevSession> {
    let default_root = &context.directories.okf;
    let target = match path_arg {
        Some(path) => resolve_asset_path(&path, context)?,
        None => {
            if panels::okf::okf_package_asset_from_dir(&context.workspace, &context.workspace)
                .is_some()
            {
                context.workspace.clone()
            } else {
                default_root.clone()
            }
        }
    };
    let target = asset_dir_target(target);
    let package = choose_okf_package(&target, default_root)?;
    Ok(panels::okf::OkfDevSession {
        name: package.name,
        description: package.description,
        rel: package.rel,
        path: package.path,
        root: asset_root_for_dir(default_root, &target),
    })
}

fn choose_agent_file(target: &Path) -> anyhow::Result<PathBuf> {
    if target.is_file() {
        return Ok(target.to_path_buf());
    }
    if let Some(entry) = panels::agent::agent_entry_file(target) {
        return Ok(entry);
    }
    let agents = panels::agent::list_agents(target);
    match agents.as_slice() {
        [agent] => Ok(agent.definition_path.clone()),
        [] => anyhow::bail!("no agent package found in {}", target.display()),
        _ => anyhow::bail!(
            "multiple agent packages found in {}; pass one package path",
            target.display()
        ),
    }
}

fn choose_mcp_project(
    target: &Path,
    config_root: &Path,
) -> anyhow::Result<panels::mcp::McpProject> {
    let root = if target.is_dir() {
        asset_root_for_dir(config_root, target)
    } else {
        anyhow::bail!("MCP path must be a directory: {}", target.display());
    };
    let projects = panels::mcp::list_mcp_projects(target);
    match projects.as_slice() {
        [project] => {
            let rel = rel_to_root(&root, &project.path);
            Ok(panels::mcp::McpProject {
                rel,
                path: project.path.clone(),
                name: project.name.clone(),
                description: project.description.clone(),
            })
        }
        [] => anyhow::bail!("no MCP asset found in {}", target.display()),
        _ => anyhow::bail!(
            "multiple MCP assets found in {}; pass one asset directory",
            target.display()
        ),
    }
}

fn choose_skill_file(target: &Path) -> anyhow::Result<PathBuf> {
    if target.is_file() {
        return Ok(target.to_path_buf());
    }
    let skills = panels::skill::list_skill_assets(target);
    match skills.as_slice() {
        [skill] => Ok(skill.path.clone()),
        [] => anyhow::bail!("no skill asset found in {}", target.display()),
        _ => anyhow::bail!(
            "multiple skill assets found in {}; pass one skill file or directory",
            target.display()
        ),
    }
}

fn choose_flow_file(target: &Path) -> anyhow::Result<PathBuf> {
    if target.is_file() {
        return Ok(target.to_path_buf());
    }
    let flows = panels::flow::list_flows(target);
    match flows.as_slice() {
        [flow] => Ok(target.join(flow)),
        [] => anyhow::bail!("no workflow JSON found in {}", target.display()),
        _ => anyhow::bail!(
            "multiple workflow JSON files found in {}; pass one file path",
            target.display()
        ),
    }
}

fn choose_okf_package(
    target: &Path,
    default_root: &Path,
) -> anyhow::Result<panels::okf::OkfPackageAsset> {
    let root = asset_root_for_dir(default_root, target);
    if target.is_dir() {
        if let Some(package) = panels::okf::okf_package_asset_from_dir(&root, target) {
            return Ok(package);
        }
    }
    let packages = panels::okf::list_okf_packages(target);
    match packages.as_slice() {
        [package] => {
            let rel = rel_to_root(&root, &package.path);
            Ok(panels::okf::OkfPackageAsset {
                rel,
                path: package.path.clone(),
                name: package.name.clone(),
                description: package.description.clone(),
            })
        }
        [] => anyhow::bail!("no OKF package found in {}", target.display()),
        _ => anyhow::bail!(
            "multiple OKF packages found in {}; pass one package directory",
            target.display()
        ),
    }
}

fn asset_dir_target(path: PathBuf) -> PathBuf {
    if path.is_file() {
        return path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.clone());
    }
    path
}

fn resolve_optional_target(
    path_arg: Option<PathBuf>,
    context: &AssetCommandContext,
) -> anyhow::Result<PathBuf> {
    match path_arg {
        Some(path) => resolve_asset_path(&path, context),
        None => Ok(context.workspace.clone()),
    }
}

fn resolve_asset_path(path: &Path, context: &AssetCommandContext) -> anyhow::Result<PathBuf> {
    let expanded = if let Ok(rest) = path.strip_prefix(Path::new("~")) {
        context
            .home
            .as_deref()
            .map(|home| home.join(rest))
            .unwrap_or_else(|| path.to_path_buf())
    } else {
        path.to_path_buf()
    };
    let path = if expanded.is_absolute() {
        expanded
    } else {
        context.workspace.join(expanded)
    };
    std::fs::canonicalize(&path)
        .map_err(|e| anyhow::anyhow!("could not resolve {}: {e}", path.display()))
}

fn asset_root_for_file(config_root: &Path, file: &Path) -> PathBuf {
    let dir = file.parent().unwrap_or_else(|| Path::new("."));
    asset_root_for_dir(config_root, dir)
}

fn asset_root_for_dir(config_root: &Path, dir: &Path) -> PathBuf {
    if let (Ok(config_root), Ok(dir)) = (
        std::fs::canonicalize(config_root),
        std::fs::canonicalize(dir),
    ) {
        if dir.starts_with(&config_root) {
            return config_root;
        }
    }
    dir.to_path_buf()
}

fn rel_to_root(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn read_flow_design(path: &Path) -> anyhow::Result<String> {
    let design = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("could not read {}: {e}", path.display()))?;
    serde_json::from_str::<serde_json::Value>(&design)
        .map_err(|e| anyhow::anyhow!("{} is not valid workflow JSON: {e}", path.display()))?;
    Ok(design)
}
