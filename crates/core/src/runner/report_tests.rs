use crate::assertions::AssertionResult;
use crate::runner::{KEEP_OUTCOMES, RunOutcomeState, keep};

use super::*;

fn outcome(name: &str, status: Option<u16>) -> RunOutcome {
    RunOutcome {
        id: format!("users/{name}.http"),
        name: name.into(),
        method: "GET".into(),
        iteration: 1,
        status,
        duration_ms: 12,
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

fn report() -> RunReport {
    let mut failing = outcome("create", Some(500));
    failing.assertions.push(AssertionResult {
        expression: "status == 201".into(),
        passed: false,
        message: "expected status == 201, got 500".into(),
    });
    let mut skipped = outcome("delete", None);
    skipped.skipped = Some("production write".into());
    let mut report = empty_report();
    for mut item in [outcome("list", Some(200)), failing, skipped] {
        item.state = item.judge();
        keep(&mut report, item);
    }
    report
}

fn empty_report() -> RunReport {
    RunReport {
        root: "/ws".into(),
        environment: Some("staging".into()),
        outcomes: Vec::new(),
        passed: 0,
        failed: 0,
        skipped: 0,
        dropped: 0,
        duration_ms: 1500,
    }
}

#[test]
fn long_runs_keep_the_first_outcomes_and_count_all() {
    let mut report = empty_report();
    for _ in 0..KEEP_OUTCOMES {
        keep(&mut report, outcome("ok", Some(200)));
    }
    let mut broken = outcome("broken", Some(500));
    broken.state = broken.judge();
    keep(&mut report, broken);

    assert_eq!(report.outcomes.len(), KEEP_OUTCOMES);
    let kept = u32::try_from(KEEP_OUTCOMES).unwrap();
    assert_eq!((report.passed, report.failed, report.dropped), (kept, 1, 1));
    assert!(pretty(&report).contains("… and 1 more not listed here\n"));
    let all = KEEP_OUTCOMES + 1;
    assert!(junit(&report).contains(&format!("tests=\"{all}\" failures=\"1\"")));
    let json: serde_json::Value = serde_json::from_str(&json(&report).unwrap()).unwrap();
    assert_eq!(json["dropped"], 1);
}

#[test]
fn outcomes_are_judged_by_assertions_errors_and_status() {
    assert_eq!(outcome("a", Some(200)).judge(), RunOutcomeState::Passed);
    assert_eq!(outcome("a", Some(404)).judge(), RunOutcomeState::Failed);
    let mut expected_404 = outcome("a", Some(404));
    expected_404.assertions.push(AssertionResult {
        expression: "status == 404".into(),
        passed: true,
        message: String::new(),
    });
    assert_eq!(expected_404.judge(), RunOutcomeState::Passed);
    let mut refused = outcome("a", None);
    refused.error = Some("refused".into());
    assert_eq!(refused.judge(), RunOutcomeState::Failed);
    let mut skipped = outcome("a", Some(500));
    skipped.skipped = Some("production".into());
    assert_eq!(skipped.judge(), RunOutcomeState::Skipped);
}

#[test]
fn pretty_lists_results_and_failed_assertions() {
    let text = pretty(&report());
    assert!(text.contains("✓ GET     list"));
    assert!(text.contains("✗ GET     create  500  12 ms"));
    assert!(text.contains("      ✗ status == 201  (expected status == 201, got 500)"));
    assert!(text.contains("- GET     delete  skipped: production write"));
    assert!(text.contains("3 requests: 1 passed, 1 failed, 1 skipped  (1500 ms)"));
}

#[test]
fn pretty_and_junit_show_iterations() {
    let mut report = report();
    report.outcomes[2].iteration = 2;
    assert!(pretty(&report).contains("- #2   GET     delete"));
    assert!(junit(&report).contains("name=\"GET delete #2\""));
}

#[test]
fn junit_marks_failures_and_skips() {
    let xml = junit(&report());
    assert!(xml.contains("tests=\"3\" failures=\"1\" skipped=\"1\" time=\"1.500\""));
    assert!(xml.contains("<testcase classname=\"users\" name=\"GET list\" time=\"0.012\"/>"));
    assert!(xml.contains("<failure message=\"status == 201: expected status == 201, got 500\">"));
    assert!(xml.contains("<skipped message=\"production write\"/>"));
}

#[test]
fn junit_drops_characters_xml_forbids() {
    let mut report = report();
    report.outcomes[1].assertions[0].message = "got \u{1b}[31mred\u{0} <b>".into();
    let xml = junit(&report);
    assert!(
        xml.contains("got \u{FFFD}[31mred\u{FFFD} &lt;b&gt;"),
        "{xml}"
    );
    assert!(
        !xml.chars()
            .any(|c| c < ' ' && !matches!(c, '\n' | '\t' | '\r'))
    );
}

#[test]
fn warnings_show_but_dont_fail() {
    let mut report = report();
    report.outcomes[0].warning = Some("failed to record history: disk full".into());
    assert!(!report.outcomes[0].failed());
    assert!(pretty(&report).contains("      ! failed to record history: disk full"));
}

#[test]
fn json_has_counts_and_requests() {
    let value: serde_json::Value = serde_json::from_str(&json(&report()).unwrap()).unwrap();
    assert_eq!(value["failed"], 1);
    assert_eq!(value["requests"][0]["status"], 200);
}

#[test]
fn status_without_assertions_decides() {
    assert!(!outcome("a", Some(204)).failed());
    assert!(outcome("a", Some(404)).failed());
}

#[test]
fn saved_reports_take_the_format_of_their_file_name() {
    let report = report();
    let text = |name: &str| report_text(&report, std::path::Path::new(name));

    assert!(text("results.XML").unwrap().starts_with("<?xml"));
    assert!(text("results.json").unwrap().starts_with('{'));
    let err = text("results.txt").unwrap_err();
    assert_eq!(err.kind(), "invalid");
    assert!(err.to_string().contains(".xml (JUnit) or .json"));
}
