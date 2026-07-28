use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{bail, Context};
use serde_json::json;

use crate::cli::args::{CacheArgs, CacheCommand, OutputMode};
use crate::cli::context::InvocationContext;
use crate::cli::output::{coded_error, render_value, ExitClass};

pub(crate) fn run(args: CacheArgs, context: &InvocationContext) -> anyhow::Result<()> {
    match args.command {
        CacheCommand::Path => path(context),
        CacheCommand::Status => status(context),
        CacheCommand::Prune(args) => prune(args.dry_run, context),
        CacheCommand::Clean(args) => clean(args.dry_run, args.yes, context),
    }
}

fn path(context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let path = context.component_paths.cache_root.clone();
    render_value(output, "cache.path", json!({"path": path}), || {
        println!("{}", path.display());
    })
}

fn status(context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let path = context.component_paths.cache_root.clone();
    let usage = cache_usage(&path)?;
    render_value(
        output,
        "cache.status",
        json!({"path": path, "files": usage.files, "directories": usage.directories, "bytes": usage.bytes}),
        || {
            println!("path: {}", path.display());
            println!("files: {}", usage.files);
            println!("directories: {}", usage.directories);
            println!("bytes: {}", usage.bytes);
        },
    )
}

fn prune(dry_run: bool, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let root = context.component_paths.cache_root.clone();
    let candidates = prune_candidates(&root)?;
    let bytes = candidates
        .iter()
        .filter_map(|path| std::fs::metadata(path).ok())
        .map(|metadata| metadata.len())
        .sum::<u64>();
    if !dry_run {
        for candidate in &candidates {
            remove_cache_entry(candidate)?;
        }
    }
    render_value(
        output,
        "cache.prune",
        json!({"path": root, "dryRun": dry_run, "removedEntries": candidates.len(), "removedBytes": bytes}),
        || {
            println!(
                "{} {} unreferenced temporary cache entries ({} bytes)",
                if dry_run { "would remove" } else { "removed" },
                candidates.len(),
                bytes
            );
        },
    )
}

fn clean(dry_run: bool, yes: bool, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let root = context.component_paths.cache_root.clone();
    validate_cache_root(&root, context.home.as_deref())?;
    let usage = cache_usage(&root)?;
    if !dry_run && !yes {
        confirm("Remove all recreatable A3S cache content?", context)?;
    }
    if !dry_run && root.is_dir() {
        for entry in std::fs::read_dir(&root)? {
            remove_cache_entry(&entry?.path())?;
        }
    }
    render_value(
        output,
        "cache.clean",
        json!({"path": root, "dryRun": dry_run, "removedFiles": usage.files, "removedBytes": usage.bytes}),
        || {
            println!(
                "{} {} cache files ({} bytes)",
                if dry_run { "would remove" } else { "removed" },
                usage.files,
                usage.bytes
            );
        },
    )
}

#[derive(Default)]
struct CacheUsage {
    files: u64,
    directories: u64,
    bytes: u64,
}

fn cache_usage(root: &Path) -> anyhow::Result<CacheUsage> {
    let mut usage = CacheUsage::default();
    if !root.exists() {
        return Ok(usage);
    }
    walk_cache(root, &mut usage)?;
    Ok(usage)
}

fn walk_cache(path: &Path, usage: &mut CacheUsage) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            usage.directories += 1;
            walk_cache(&entry.path(), usage)?;
        } else if metadata.is_file() {
            usage.files += 1;
            usage.bytes = usage.bytes.saturating_add(metadata.len());
        }
    }
    Ok(())
}

fn prune_candidates(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut candidates = Vec::new();
    if !root.is_dir() {
        return Ok(candidates);
    }
    collect_prune_candidates(root, SystemTime::now(), &mut candidates)?;
    Ok(candidates)
}

fn collect_prune_candidates(
    directory: &Path,
    now: SystemTime,
    candidates: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        let old = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= Duration::from_secs(24 * 60 * 60));
        if old && (name.ends_with(".tmp") || name.starts_with(".staging-")) {
            candidates.push(path);
        } else if metadata.is_dir() {
            collect_prune_candidates(&path, now, candidates)?;
        }
    }
    Ok(())
}

fn remove_cache_entry(path: &Path) -> anyhow::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
    .with_context(|| format!("could not remove cache entry {}", path.display()))
}

fn validate_cache_root(root: &Path, home: Option<&Path>) -> anyhow::Result<()> {
    if root.parent().is_none() || root == Path::new("/") {
        bail!("refusing to clean an unsafe cache root {}", root.display());
    }
    if std::fs::symlink_metadata(root).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        bail!(
            "refusing to clean a symbolic-link cache root {}",
            root.display()
        );
    }
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    if home
        .map(Path::to_path_buf)
        .map(|home| home.canonicalize().unwrap_or(home))
        .is_some_and(|home| home == canonical_root)
    {
        bail!("refusing to clean the home directory as a cache root");
    }
    Ok(())
}

fn confirm(prompt: &str, context: &InvocationContext) -> anyhow::Result<()> {
    if context.output_mode() != OutputMode::Human
        || context.interaction.non_interactive
        || !context.terminal.stdin
        || !context.terminal.stderr
    {
        bail!("cache cleaning requires `--yes` in non-interactive mode");
    }
    eprint!("{prompt} [y/N] ");
    std::io::stderr().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    if matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        Ok(())
    } else {
        Err(coded_error(
            "operation.cancelled",
            "cache cleaning cancelled",
            ExitClass::Cancelled,
        ))
    }
}
