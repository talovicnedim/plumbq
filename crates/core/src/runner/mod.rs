//! Runs many requests in sidebar order, for the app's Runner, the CLI and MCP `folders_run`.
//! When a request fails: `docs/formats.md` (Runner).

mod data;
mod report;
mod request;
mod select;

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

pub use data::{load_data, parse_data};
pub use report::{json, junit, pretty, report_text};
pub use select::{RunPreview, Selected, preview, prompts_needed, select};

use crate::assertions::AssertionResult;
use crate::send::SendContext;
use crate::{Error, Result, guard};
use request::run_one;

#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    pub environment: Option<String>,
    /// Stop at the first failing request.
    pub bail: bool,
    /// Answers for `{{$prompt name}}` variables.
    pub prompts: BTreeMap<String, String>,
    /// Only requests with at least one of these tags (`# @tag smoke`). Empty runs everything.
    pub tags: Vec<String>,
    /// Send POST, PUT, … to production environments instead of skipping them.
    pub allow_production_writes: bool,
    /// Skip everything but GET, HEAD and OPTIONS, also when a method override header asks for
    /// another method (read-only MCP mode).
    pub safe_methods_only: bool,
    /// Pause between requests.
    pub delay_ms: u32,
    /// Runs of the whole selection when there's no data file; at least 1, at most
    /// [`MAX_ITERATIONS`] (see [`check_iterations`]).
    pub iterations: u32,
    /// Data-file rows: one run of the selection per row, columns as variables.
    pub data: Vec<BTreeMap<String, String>>,
    /// History (recorded, and used for chaining) and cookie jars. Its `variables` and `secrets`
    /// (e.g. the CLI's `--var`) win over a data row's columns.
    pub context: SendContext,
    /// Set when an AI agent started the run (MCP): each request as the user approved it, by
    /// id. A request that changed since, or wasn't approved, is refused instead of sent.
    pub agent: Option<BTreeMap<String, guard::SendAssessment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RunOutcome {
    pub id: String,
    pub name: String,
    pub method: String,
    /// 1-based; above 1 only with iterations or a data file.
    pub iteration: u32,
    pub status: Option<u16>,
    pub duration_ms: u32,
    pub assertions: Vec<AssertionResult>,
    /// Why it couldn't be sent.
    pub error: Option<String>,
    /// Why it wasn't sent on purpose (e.g. a production write).
    pub skipped: Option<String>,
    /// Skipped because it writes to production; sending it needs the user's confirmation.
    #[serde(default)]
    pub needs_confirmation: bool,
    /// `{{$prompt}}` names that had no answer.
    pub prompts_needed: Vec<String>,
    /// The recorded history entry, to show the full response.
    pub history_id: Option<String>,
    /// Something went wrong around the request without failing it, e.g. history couldn't be
    /// recorded.
    #[serde(default)]
    pub warning: Option<String>,
    /// The verdict, from [`RunOutcome::judge`] once the request is done.
    pub state: RunOutcomeState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RunOutcomeState {
    Passed,
    Failed,
    Skipped,
}

impl RunOutcome {
    pub fn judge(&self) -> RunOutcomeState {
        match (self.skipped.is_some(), self.failed()) {
            (true, _) => RunOutcomeState::Skipped,
            (false, true) => RunOutcomeState::Failed,
            (false, false) => RunOutcomeState::Passed,
        }
    }

    /// Failed assertions, an error, or a 4xx/5xx status when there are no assertions to judge by.
    pub fn failed(&self) -> bool {
        if self.skipped.is_some() {
            return false;
        }
        if self.error.is_some() || self.assertions.iter().any(|a| !a.passed) {
            return true;
        }
        self.assertions.is_empty() && self.status.is_some_and(|s| s >= 400)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RunReport {
    pub root: String,
    pub environment: Option<String>,
    /// The first [`KEEP_OUTCOMES`] outcomes, in run order.
    pub outcomes: Vec<RunOutcome>,
    /// Counts of every outcome, also the ones left out of `outcomes`.
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    /// Outcomes counted but not kept in `outcomes`.
    pub dropped: u32,
    pub duration_ms: u32,
}

/// Outcomes a report keeps: a run of many iterations must not hold them all in memory. Later
/// ones are still counted and passed to `on_outcome` as they happen.
pub(crate) const KEEP_OUTCOMES: usize = 10_000;

/// More iterations than this are refused: a typo like `--iterations 4000000000` would
/// otherwise run for days.
pub(crate) const MAX_ITERATIONS: u32 = 1_000_000;

pub fn check_iterations(iterations: u32) -> Result<()> {
    if iterations > MAX_ITERATIONS {
        return Err(Error::Invalid(format!(
            "{iterations} iterations is more than the limit of {MAX_ITERATIONS}"
        )));
    }
    Ok(())
}

/// Runs `selection` and reports each outcome to `on_outcome` as it happens; returning `false`
/// from it stops the run (the app's Stop button).
pub async fn run(
    root: &Path,
    selection: &[Selected],
    options: &RunOptions,
    mut on_outcome: impl FnMut(&RunOutcome) -> bool,
) -> RunReport {
    let started = Instant::now();
    // One row at a time: a huge `--iterations` must not allocate a row per iteration upfront.
    let runs = match options.data.is_empty() {
        true => options.iterations.max(1) as usize,
        false => options.data.len(),
    };
    let mut report = RunReport {
        root: root.display().to_string(),
        environment: options.environment.clone(),
        outcomes: Vec::new(),
        passed: 0,
        failed: 0,
        skipped: 0,
        dropped: 0,
        duration_ms: 0,
    };
    let mut first = true;
    'rows: for index in 0..runs {
        let mut variables = options.data.get(index).cloned().unwrap_or_default();
        variables.extend(options.context.variables.clone());
        let context = SendContext {
            variables,
            ..options.context.clone()
        };
        for item in selection {
            if !first && options.delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(u64::from(options.delay_ms))).await;
            }
            first = false;
            let iteration = u32::try_from(index + 1).unwrap_or(u32::MAX);
            let mut outcome = run_one(root, item, iteration, options, &context).await;
            outcome.state = outcome.judge();
            let keep_going = on_outcome(&outcome);
            let stop = !keep_going || (options.bail && outcome.failed());
            keep(&mut report, outcome);
            if stop {
                break 'rows;
            }
        }
    }
    report.duration_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
    report
}

/// Counts a judged outcome, and keeps it while the report has room.
fn keep(report: &mut RunReport, outcome: RunOutcome) {
    let count = match outcome.state {
        RunOutcomeState::Passed => &mut report.passed,
        RunOutcomeState::Failed => &mut report.failed,
        RunOutcomeState::Skipped => &mut report.skipped,
    };
    *count = count.saturating_add(1);
    match report.outcomes.len() < KEEP_OUTCOMES {
        true => report.outcomes.push(outcome),
        false => report.dropped = report.dropped.saturating_add(1),
    }
}

#[cfg(test)]
mod agent_tests;

#[cfg(test)]
mod iteration_tests;

#[cfg(test)]
mod preview_tests;

#[cfg(test)]
mod tests;
