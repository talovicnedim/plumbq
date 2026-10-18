//! One request of a run: the read-only and agent checks, the send, and its history entry.

use std::path::Path;

use super::{RunOptions, RunOutcome, RunOutcomeState, Selected};
use crate::send::{self, SendContext};
use crate::{Error, guard, history, requests};

pub(super) async fn run_one(
    root: &Path,
    item: &Selected,
    iteration: u32,
    options: &RunOptions,
    context: &SendContext,
) -> RunOutcome {
    let mut outcome = not_sent(item, iteration);
    let env = options.environment.as_deref();
    let request = match requests::get(root, &item.id) {
        Ok(request) => request,
        Err(err) => return failed(outcome, err),
    };
    // The request as it is now, not as selected: a GET edited into a DELETE during a long run
    // must still be held back in read-only mode.
    outcome.method = request.method.clone();
    if options.safe_methods_only {
        let method = match send::effective_method(root, &item.id, &request) {
            Ok(method) => method,
            Err(err) => return failed(outcome, err),
        };
        if !send::SAFE_METHODS.contains(&method.as_str()) {
            outcome.skipped = Some(format!("{method} isn't sent in read-only mode"));
            return outcome;
        }
    }
    let confirmed = match &options.agent {
        Some(approved) => {
            match guard::check_approved(root, &item.id, &request, env, approved.get(&item.id)) {
                Ok(confirmed) => confirmed,
                Err(err) => return failed(outcome, err),
            }
        }
        None => options.allow_production_writes,
    };
    let sent = send::send(
        root,
        &item.id,
        &request,
        env,
        &options.prompts,
        context,
        confirmed,
    )
    .await;
    match sent {
        Ok(response) => record(outcome, root, &item.id, env, context, response),
        Err(err) => failed(outcome, err),
    }
}

fn not_sent(item: &Selected, iteration: u32) -> RunOutcome {
    RunOutcome {
        id: item.id.clone(),
        name: item.name.clone(),
        method: item.method.clone(),
        iteration,
        status: None,
        duration_ms: 0,
        assertions: Vec::new(),
        error: None,
        skipped: None,
        needs_confirmation: false,
        prompts_needed: Vec::new(),
        history_id: None,
        warning: None,
        state: RunOutcomeState::Passed,
    }
}

fn record(
    mut outcome: RunOutcome,
    root: &Path,
    id: &str,
    env: Option<&str>,
    context: &SendContext,
    response: send::Response,
) -> RunOutcome {
    outcome.status = Some(response.status);
    outcome.duration_ms = response.duration_ms;
    outcome.assertions = response.assertions.clone();
    if let Some(dir) = &context.history_dir {
        match history::add(dir, root, id, env, response) {
            Ok(entry) => outcome.history_id = Some(entry.id),
            // The request itself went fine; a full disk or read-only HOME isn't its fault.
            Err(err) => outcome.warning = Some(format!("failed to record history: {err}")),
        }
    }
    outcome
}

/// A request that wasn't sent: production writes are skipped like in any run (also when the
/// guard stops an agent's request), anything else is an error the caller can read.
fn failed(mut outcome: RunOutcome, err: Error) -> RunOutcome {
    match err {
        Error::NeedsConfirmation(message) => {
            outcome.skipped = Some(format!("{message}; production writes are skipped in runs"));
            outcome.needs_confirmation = true;
        }
        Error::NeedsInput(names) => {
            outcome.error = Some(format!("needs values for {}", names.join(", ")));
            outcome.prompts_needed = names;
        }
        other => outcome.error = Some(other.to_string()),
    }
    outcome
}
