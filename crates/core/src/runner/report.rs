//! Turns a run into text: a readable summary, JUnit XML for CI, or JSON.

use std::path::Path;

use super::{RunOutcome, RunReport};
use crate::{Error, Result};

pub fn pretty(report: &RunReport) -> String {
    let mut out = format!("Workspace {}", report.root);
    if let Some(env) = &report.environment {
        out.push_str(&format!(" · environment {env}"));
    }
    out.push_str("\n\n");
    let iterations = report.outcomes.iter().any(|o| o.iteration > 1);
    let width = report
        .outcomes
        .iter()
        .map(|o| o.name.chars().count())
        .max()
        .unwrap_or(0);
    for outcome in &report.outcomes {
        out.push_str(&pretty_outcome(outcome, iterations, width));
    }
    if report.dropped > 0 {
        out.push_str(&format!("… and {} more not listed here\n", report.dropped));
    }
    let RunReport {
        passed,
        failed,
        skipped,
        ..
    } = report;
    out.push_str(&format!(
        "\n{} requests: {passed} passed, {failed} failed, {skipped} skipped  ({} ms)\n",
        total(report),
        report.duration_ms
    ));
    out
}

/// One line per request, then its failed assertions and warning.
fn pretty_outcome(outcome: &RunOutcome, iterations: bool, width: usize) -> String {
    let mark = match (outcome.skipped.is_some(), outcome.failed()) {
        (true, _) => "-",
        (false, true) => "✗",
        (false, false) => "✓",
    };
    let result = match (&outcome.skipped, &outcome.error, outcome.status) {
        (Some(reason), _, _) => format!("skipped: {reason}"),
        (_, Some(error), None) => format!("error: {error}"),
        (_, error, Some(status)) => {
            let note = error
                .as_ref()
                .map(|e| format!("  ({e})"))
                .unwrap_or_default();
            format!("{status}  {} ms{note}", outcome.duration_ms)
        }
        (None, None, None) => String::new(),
    };
    let round = match iterations {
        true => format!("#{:<3} ", outcome.iteration),
        false => String::new(),
    };
    let mut out = format!(
        "{mark} {round}{:<7} {:<width$}  {result}\n",
        outcome.method, outcome.name
    );
    for assertion in outcome.assertions.iter().filter(|a| !a.passed) {
        out.push_str(&format!(
            "      ✗ {}  ({})\n",
            assertion.expression, assertion.message
        ));
    }
    if let Some(warning) = &outcome.warning {
        out.push_str(&format!("      ! {warning}\n"));
    }
    out
}

/// JUnit XML, one test case per request, understood by GitHub Actions, GitLab, Jenkins, ….
pub fn junit(report: &RunReport) -> String {
    let (failed, skipped) = (report.failed, report.skipped);
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<testsuites name=\"plumbq\" tests=\"{}\" \
         failures=\"{failed}\" skipped=\"{skipped}\" time=\"{}\">\n  <testsuite name=\"{}\" \
         tests=\"{}\" failures=\"{failed}\" skipped=\"{skipped}\">\n",
        total(report),
        seconds(report.duration_ms),
        xml(&report.root),
        total(report)
    );
    let iterations = report.outcomes.iter().any(|o| o.iteration > 1);
    for outcome in &report.outcomes {
        let folder = outcome.id.rsplit_once('/').map(|(f, _)| f).unwrap_or("");
        let mut name = format!("{} {}", outcome.method, outcome.name);
        if iterations {
            name.push_str(&format!(" #{}", outcome.iteration));
        }
        out.push_str(&format!(
            "    <testcase classname=\"{}\" name=\"{}\" time=\"{}\"",
            xml(folder),
            xml(&name),
            seconds(outcome.duration_ms)
        ));
        if let Some(reason) = &outcome.skipped {
            out.push_str(&format!(
                ">\n      <skipped message=\"{}\"/>\n    </testcase>\n",
                xml(reason)
            ));
        } else if outcome.failed() {
            let message = failure_message(outcome);
            out.push_str(&format!(
                ">\n      <failure message=\"{}\">{}</failure>\n    </testcase>\n",
                xml(&message),
                xml(&message)
            ));
        } else {
            out.push_str("/>\n");
        }
    }
    out.push_str("  </testsuite>\n</testsuites>\n");
    out
}

/// The report as the file at `path` should hold it: JUnit XML for `.xml`, JSON for `.json`.
pub fn report_text(report: &RunReport, path: &Path) -> Result<String> {
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "xml" => Ok(junit(report)),
        "json" => json(report),
        _ => Err(Error::Invalid(
            "save the report as .xml (JUnit) or .json".into(),
        )),
    }
}

pub fn json(report: &RunReport) -> Result<String> {
    let value = serde_json::json!({
        "workspace": report.root,
        "environment": report.environment,
        "passed": report.passed,
        "failed": report.failed,
        "skipped": report.skipped,
        "dropped": report.dropped,
        "durationMs": report.duration_ms,
        "requests": report.outcomes,
    });
    Ok(serde_json::to_string_pretty(&value)?)
}

fn failure_message(outcome: &RunOutcome) -> String {
    if let Some(error) = &outcome.error {
        return error.clone();
    }
    let failed: Vec<String> = outcome
        .assertions
        .iter()
        .filter(|a| !a.passed)
        .map(|a| format!("{}: {}", a.expression, a.message))
        .collect();
    if failed.is_empty() {
        format!("status {}", outcome.status.unwrap_or_default())
    } else {
        failed.join("; ")
    }
}

/// Every request of the run, also those not kept in `outcomes`.
fn total(report: &RunReport) -> u64 {
    u64::from(report.passed) + u64::from(report.failed) + u64::from(report.skipped)
}

fn seconds(ms: u32) -> String {
    format!("{:.3}", f64::from(ms) / 1000.0)
}

/// Escaped for XML attributes and text. Characters XML 1.0 forbids even escaped (control
/// characters from response bodies, ANSI codes, U+FFFE/U+FFFF) become U+FFFD.
fn xml(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\t' | '\n' | '\r' => c,
            c if c < ' ' || c == '\u{FFFE}' || c == '\u{FFFF}' => '\u{FFFD}',
            c => c,
        })
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
