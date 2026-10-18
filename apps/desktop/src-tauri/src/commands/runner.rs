use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

use plumbq_core::fs_util;
use plumbq_core::runner::{self, RunOptions, RunOutcome, RunPreview, RunReport};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};

use crate::dialogs;
use crate::error::{AppError, blocking};
use crate::state::AppState;

/// One run at a time; the Stop button flips this and the run ends after the current request.
static STOP: AtomicBool = AtomicBool::new(false);
static RUNNING: AtomicBool = AtomicBool::new(false);

/// What to run, from the Runner panel.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RunnerRequest {
    /// Folder path, request id, or `""` for the whole workspace.
    pub target: String,
    pub environment: Option<String>,
    pub bail: bool,
    pub tags: Vec<String>,
    pub delay_ms: u32,
    pub iterations: u32,
    /// CSV or JSON data file: one run per row.
    pub data_file: Option<String>,
    pub prompts: BTreeMap<String, String>,
    /// The user typed the production environment's name for the writes the preview counted.
    pub allow_production_writes: bool,
}

/// What a run would send and need first: request count, `{{$prompt}}` names and writes to
/// production.
#[tauri::command(async)]
#[specta::specta]
pub fn runner_preview(
    state: State<'_, AppState>,
    target: String,
    tags: Vec<String>,
    environment: Option<String>,
) -> Result<RunPreview, AppError> {
    let root = state.root()?;
    Ok(runner::preview(
        &root,
        &target,
        &tags,
        environment.as_deref(),
    )?)
}

/// Runs requests in sidebar order, streaming each request's outcome through `outcomes`;
/// resolves with the full report when done or stopped.
#[tauri::command]
#[specta::specta]
pub async fn runner_run(
    state: State<'_, AppState>,
    run: RunnerRequest,
    outcomes: Channel<RunOutcome>,
) -> Result<RunReport, AppError> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err(AppError {
            kind: "busy".into(),
            message: "a run is already in progress".into(),
        });
    }
    STOP.store(false, Ordering::SeqCst);
    let result = run_inner(&state, run, &outcomes).await;
    RUNNING.store(false, Ordering::SeqCst);
    result
}

async fn run_inner(
    state: &AppState,
    run: RunnerRequest,
    outcomes: &Channel<RunOutcome>,
) -> Result<RunReport, AppError> {
    let root = state.root()?;
    runner::check_iterations(run.iterations)?;
    let data_file = run
        .data_file
        .as_ref()
        .map(|p| state.granted(p))
        .transpose()?;
    let (selection, data) = {
        let (root, target, tags) = (root.clone(), run.target.clone(), run.tags.clone());
        blocking(move || {
            let selection = runner::select(&root, &target, &tags)?;
            let data = match data_file {
                Some(path) => runner::load_data(&path)?,
                None => Vec::new(),
            };
            Ok((selection, data))
        })
        .await?
    };
    let options = RunOptions {
        environment: run.environment,
        bail: run.bail,
        prompts: run.prompts,
        tags: run.tags,
        allow_production_writes: run.allow_production_writes,
        safe_methods_only: false,
        delay_ms: run.delay_ms,
        iterations: run.iterations,
        data,
        context: state.send_context(),
        agent: None,
    };
    let report = runner::run(&root, &selection, &options, |outcome| {
        if let Err(err) = outcomes.send(outcome.clone()) {
            tracing::warn!("failed to stream a run outcome: {err}");
        }
        !STOP.load(Ordering::SeqCst)
    })
    .await;
    Ok(report)
}

/// Stops the current run after the request in flight.
#[tauri::command]
#[specta::specta]
pub fn runner_stop() {
    STOP.store(true, Ordering::SeqCst);
}

/// Asks where to save a run's results and writes them: JUnit XML for a `.xml` file, JSON for
/// `.json`. Returns the path, or `None` when cancelled.
#[tauri::command]
#[specta::specta]
pub async fn runner_export_report(
    app: AppHandle,
    report: RunReport,
    file_name: String,
) -> Result<Option<String>, AppError> {
    let filter = ("JUnit XML or JSON", &["xml", "json"][..]);
    let Some(path) = dialogs::save(&app, "Save run results", &file_name, filter).await else {
        return Ok(None);
    };
    let text = runner::report_text(&report, &path)?;
    blocking(move || {
        fs_util::write_atomic(&path, text).map_err(|err| AppError {
            kind: "io".into(),
            message: format!("failed to write {}: {err}", path.display()),
        })?;
        Ok(Some(path.display().to_string()))
    })
    .await
}
