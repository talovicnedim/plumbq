//! `plumbq`: runs the requests and assertions of a workspace from a terminal or CI, and
//! generates HTML docs.
//!
//! ```text
//! plumbq run api/ --env staging --reporter junit --output results.xml
//! plumbq docs api/ --env production --output docs.html
//! ```

mod args;
mod run;

use std::io::Write;
use std::process::ExitCode;

use plumbq_core::{export, fs_util, runner, workspace};

use crate::args::{Command, DocsArgs, Reporter, RunArgs};

#[tokio::main]
async fn main() -> ExitCode {
    plumbq_core::http_client::install_crypto();
    let command = match args::parse(std::env::args().skip(1)) {
        Ok(command) => command,
        Err(err) => return usage_error(&err),
    };
    match command {
        Command::Help => print(&format!("{}\n", args::USAGE)),
        Command::Version => print(&format!("plumbq {}\n", env!("CARGO_PKG_VERSION"))),
        Command::Run(args) => run_command(&args).await,
        Command::Docs(args) => docs_command(&args),
    }
}

async fn run_command(args: &RunArgs) -> ExitCode {
    let report = match run::run(args).await {
        Ok(report) => report,
        Err(err) => return usage_error(&err),
    };
    let text = match args.reporter {
        Reporter::Pretty => Ok(runner::pretty(&report)),
        Reporter::Junit => Ok(runner::junit(&report)),
        Reporter::Json => runner::json(&report).map_err(|err| err.to_string()),
    };
    let text = match text {
        Ok(text) => text,
        Err(err) => return usage_error(&err),
    };
    if let Some(path) = &args.output {
        if let Err(err) = fs_util::write_atomic(path, &text) {
            return usage_error(&format!("failed to write {}: {err}", path.display()));
        }
        // With --output, the terminal still gets the readable summary.
        print(&runner::pretty(&report));
    } else {
        print(&text);
    }

    if report.failed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn docs_command(args: &DocsArgs) -> ExitCode {
    let html = workspace::canonical_root(&args.target)
        .map_err(|_| format!("{} not found", args.target.display()))
        .and_then(|root| {
            export::docs::html(&root, args.environment.as_deref()).map_err(|err| err.to_string())
        });
    let html = match html {
        Ok(html) => html,
        Err(err) => return usage_error(&err),
    };
    match &args.output {
        Some(path) => match fs_util::write_atomic(path, html) {
            Ok(()) => print(&format!("Wrote {}\n", path.display())),
            Err(err) => usage_error(&format!("failed to write {}: {err}", path.display())),
        },
        None => print(&html),
    }
}

fn print(text: &str) -> ExitCode {
    let mut stdout = std::io::stdout().lock();
    // A closed pipe (e.g. `| head`) isn't worth failing over.
    let _ = stdout.write_all(text.as_bytes());
    ExitCode::SUCCESS
}

fn usage_error(message: &str) -> ExitCode {
    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "error: {message}\n\n{}", args::USAGE);
    ExitCode::from(2)
}
