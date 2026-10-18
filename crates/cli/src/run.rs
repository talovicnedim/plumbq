//! `plumbq run`: finds the workspace and the requests to run, then hands them to the shared
//! runner in core (the same one the app's Runner uses).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use plumbq_core::runner::{self, RunOptions, RunReport};
use plumbq_core::send::SendContext;
use plumbq_core::{environments, secret_names, workspace};

use crate::args::RunArgs;

/// Runs every request selected by `args.target`. Errors are for problems before anything is
/// sent (no workspace, nothing to run, unreadable data file); per-request problems end up in
/// the outcomes.
pub async fn run(args: &RunArgs) -> Result<RunReport, String> {
    let target = workspace::canonical_root(&args.target)
        .map_err(|_| format!("{} not found", args.target.display()))?;
    let root = match &args.root {
        Some(root) => workspace::canonical_root(root)
            .map_err(|_| format!("workspace {} not found", root.display()))?,
        None => find_root(&target),
    };
    let rel = relative(&root, &target)?;
    check_environment(&root, args.environment.as_deref())?;
    let selection = runner::select(&root, &rel, &args.tags).map_err(|err| err.to_string())?;
    if selection.is_empty() {
        return Err(format!("no requests to run in {}", target.display()));
    }
    let data = match &args.data {
        Some(path) => runner::load_data(path).map_err(|err| err.to_string())?,
        None => Vec::new(),
    };

    // Without history, responses and cookies still go to a throwaway folder for this run, so
    // chained requests (`{{login.response…}}`) and session cookies work within it.
    let scratch = match args.history {
        true => None,
        false => Some(scratch_dir()?),
    };
    let options = run_options(args, data, scratch.as_deref());
    let mut report = runner::run(&root, &selection, &options, |_| true).await;
    if let Some(dir) = scratch {
        // Leftovers in the temp dir are harmless; don't fail a run over cleanup.
        let _ = std::fs::remove_dir_all(dir);
    }
    add_flag_hints(&mut report);
    Ok(report)
}

fn check_environment(root: &Path, environment: Option<&str>) -> Result<(), String> {
    let Some(env) = environment else {
        return Ok(());
    };
    if environments::exists(root, env) {
        return Ok(());
    }
    let known = environments::names(root).map_err(|err| err.to_string())?;
    Err(format!(
        "unknown environment {env}{} in {}; environments: {}",
        environments::unknown_hint(env, &known),
        root.display(),
        if known.is_empty() {
            "none".to_string()
        } else {
            known.join(", ")
        }
    ))
}

fn run_options(
    args: &RunArgs,
    data: Vec<BTreeMap<String, String>>,
    scratch: Option<&Path>,
) -> RunOptions {
    RunOptions {
        environment: args.environment.clone(),
        bail: args.bail,
        prompts: args.prompts.clone(),
        tags: args.tags.clone(),
        allow_production_writes: args.allow_production_writes,
        safe_methods_only: false,
        delay_ms: args.delay_ms,
        iterations: args.iterations,
        data,
        context: SendContext {
            history_dir: match scratch {
                Some(dir) => Some(dir.join("history")),
                None => plumbq_core::app_dirs::history_dir(),
            },
            cookie_dir: match (scratch, args.cookies) {
                (_, false) => None,
                (Some(dir), true) => Some(dir.join("cookies")),
                (None, true) => plumbq_core::app_dirs::cookie_dir(),
            },
            // Masked in history and output like environment secrets.
            secrets: secret_looking(&args.vars, true),
            variables: secret_looking(&args.vars, false),
            agent: false,
        },
        agent: None,
    }
}

/// The `--var` values whose name looks like a credential, or (`secret = false`) the others.
fn secret_looking(vars: &BTreeMap<String, String>, secret: bool) -> BTreeMap<String, String> {
    vars.iter()
        .filter(|(name, _)| secret_names::looks_secret(name) == secret)
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

/// Tells the user which CLI flag would have sent a request that wasn't.
fn add_flag_hints(report: &mut RunReport) {
    for outcome in &mut report.outcomes {
        if !outcome.prompts_needed.is_empty() {
            let flags: Vec<String> = outcome
                .prompts_needed
                .iter()
                .map(|n| format!("--prompt {n}=…"))
                .collect();
            outcome.error = Some(format!("needs values: {}", flags.join(" ")));
        }
        if let Some(reason) = &outcome.skipped
            && outcome.needs_confirmation
        {
            outcome.skipped = Some(format!(
                "{reason}; pass --allow-production-writes to send it"
            ));
        }
    }
}

/// A new folder for this run that only this user can open. The name is random and an existing
/// folder is never used, so another user on the machine can't plant one to read the run's
/// responses.
fn scratch_dir() -> Result<PathBuf, String> {
    let name = format!("plumbq-run-{}", uuid::Uuid::new_v4().simple());
    let dir = std::env::temp_dir().join(name);
    create_private_dir(&dir).map_err(|err| format!("failed to create {}: {err}", dir.display()))?;
    Ok(dir)
}

/// Fails when `dir` exists (not recursive, unlike `create_dir_all`).
#[cfg(unix)]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(dir)
}

#[cfg(not(unix))]
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir(dir)
}

/// The target as a workspace-relative path (`""` for the root), with `/` separators.
fn relative(root: &Path, target: &Path) -> Result<String, String> {
    Ok(target
        .strip_prefix(root)
        .map_err(|_| format!("{} is outside the workspace", target.display()))?
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/"))
}

/// The workspace root for a target: the highest folder at or above it that looks like a
/// workspace (has a `folder.toml`, or an `environments/` folder with `.toml` files), without
/// leaving the git repository it's in. Otherwise the target's own folder.
pub fn find_root(target: &Path) -> PathBuf {
    let start = if target.is_dir() {
        target.to_path_buf()
    } else {
        target.parent().map(Path::to_path_buf).unwrap_or_default()
    };
    let mut root = start.clone();
    for dir in start.ancestors() {
        if looks_like_workspace(dir) {
            root = dir.to_path_buf();
        }
        // A repository's root is as high as a workspace goes.
        if dir.join(".git").exists() {
            break;
        }
    }
    root
}

fn looks_like_workspace(dir: &Path) -> bool {
    if dir.join(workspace::FOLDER_FILE).is_file() {
        return true;
    }
    // Other tools have `environments/` folders too (e.g. Terraform); ours hold `.toml` files.
    std::fs::read_dir(dir.join(workspace::ENVIRONMENTS_DIR))
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
        })
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
