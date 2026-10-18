//! Command-line arguments, parsed by hand: the surface is small and stable.

use std::collections::BTreeMap;
use std::path::PathBuf;

use plumbq_core::runner;

pub const USAGE: &str = "\
usage: plumbq run <workspace | folder | file.http> [options]
       plumbq docs <workspace> [--env NAME] [--output docs.html]

run options:
  --env, -e NAME               environment to resolve {{variables}} with
  --bail                       stop at the first failing request
  --reporter pretty|junit|json output format (default: pretty)
  --output, -o FILE            write the report to FILE instead of stdout
  --prompt NAME=VALUE          answer a {{$prompt NAME}} variable (repeatable)
  --var NAME=VALUE             set a {{variable}}, winning over environments and --data rows
                               (repeatable; masked like a secret when NAME looks like one,
                               e.g. API_TOKEN)
  --root DIR                   the workspace folder, when it can't be found from the target
  --allow-production-writes    send POST/PUT/PATCH/DELETE… to production environments
  --no-history                 don't keep responses in the app's history or cookie jar
                               (chaining and cookies still work within the run)
  --no-cookies                 don't send or store cookies
  --tag NAME                   only requests tagged `# @tag NAME` (repeatable)
  --data FILE                  CSV (with a header row) or JSON array: run once per row, with
                               the row's columns as {{variables}}
  --iterations N               run the selection N times (without --data)
  --delay MS                   wait MS milliseconds between requests

docs writes a self-contained HTML page documenting every request (stdout without --output).

A request fails when an assertion fails, when it has no assertions and the status is 400 or
higher, or when it can't be sent. Exit code: 0 all passed, 1 something failed, 2 usage error
(including an unknown --env).

Secrets: without a keychain (CI), a secret NAME is read from the environment variable
PLUMBQ_SECRET_NAME (upper-cased, other characters as _).";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reporter {
    Pretty,
    Junit,
    Json,
}

#[derive(Debug, PartialEq)]
pub struct RunArgs {
    pub target: PathBuf,
    pub environment: Option<String>,
    pub bail: bool,
    pub reporter: Reporter,
    pub output: Option<PathBuf>,
    pub prompts: BTreeMap<String, String>,
    pub vars: BTreeMap<String, String>,
    pub root: Option<PathBuf>,
    pub allow_production_writes: bool,
    pub history: bool,
    pub cookies: bool,
    pub tags: Vec<String>,
    pub data: Option<PathBuf>,
    pub iterations: u32,
    pub delay_ms: u32,
}

#[derive(Debug, PartialEq)]
pub struct DocsArgs {
    pub target: PathBuf,
    pub environment: Option<String>,
    pub output: Option<PathBuf>,
}

#[derive(Debug, PartialEq)]
pub enum Command {
    Run(RunArgs),
    Docs(DocsArgs),
    Help,
    Version,
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some("run") => parse_run(args).map(Command::Run),
        Some("docs") => parse_docs(args).map(Command::Docs),
        Some("--help" | "-h" | "help") | None => Ok(Command::Help),
        Some("--version" | "-V") => Ok(Command::Version),
        Some(other) => Err(format!("unknown command {other}")),
    }
}

fn parse_run(mut args: impl Iterator<Item = String>) -> Result<RunArgs, String> {
    let mut run = RunArgs {
        target: PathBuf::new(),
        environment: None,
        bail: false,
        reporter: Reporter::Pretty,
        output: None,
        prompts: BTreeMap::new(),
        vars: BTreeMap::new(),
        root: None,
        allow_production_writes: false,
        history: true,
        cookies: true,
        tags: Vec::new(),
        data: None,
        iterations: 1,
        delay_ms: 0,
    };
    let mut target = None;
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--env" | "-e" => run.environment = Some(value()?),
            "--bail" => run.bail = true,
            "--reporter" => run.reporter = reporter(&value()?)?,
            "--output" | "-o" => run.output = Some(PathBuf::from(value()?)),
            "--prompt" => {
                let (name, answer) = pair(&arg, &value()?)?;
                run.prompts.insert(name, answer);
            }
            "--var" => {
                let (name, val) = pair(&arg, &value()?)?;
                run.vars.insert(name, val);
            }
            "--root" => run.root = Some(PathBuf::from(value()?)),
            "--allow-production-writes" => run.allow_production_writes = true,
            "--no-history" => run.history = false,
            "--no-cookies" => run.cookies = false,
            "--tag" => run.tags.push(value()?),
            "--data" => run.data = Some(PathBuf::from(value()?)),
            "--iterations" => run.iterations = iterations(&arg, &value()?)?,
            "--delay" => run.delay_ms = number(&arg, &value()?)?,
            flag if flag.starts_with('-') => return Err(format!("unknown option {flag}")),
            path if target.is_none() => target = Some(PathBuf::from(path)),
            extra => return Err(format!("unexpected argument {extra}")),
        }
    }
    run.target = target.ok_or("run needs a workspace, folder or .http file")?;
    Ok(run)
}

fn iterations(flag: &str, value: &str) -> Result<u32, String> {
    let iterations = number(flag, value)?.max(1);
    runner::check_iterations(iterations).map_err(|err| err.to_string())?;
    Ok(iterations)
}

fn reporter(name: &str) -> Result<Reporter, String> {
    match name {
        "pretty" => Ok(Reporter::Pretty),
        "junit" => Ok(Reporter::Junit),
        "json" => Ok(Reporter::Json),
        other => Err(format!(
            "unknown reporter {other}; use pretty, junit or json"
        )),
    }
}

fn parse_docs(mut args: impl Iterator<Item = String>) -> Result<DocsArgs, String> {
    let mut docs = DocsArgs {
        target: PathBuf::new(),
        environment: None,
        output: None,
    };
    let mut target = None;
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--env" | "-e" => docs.environment = Some(value()?),
            "--output" | "-o" => docs.output = Some(PathBuf::from(value()?)),
            flag if flag.starts_with('-') => return Err(format!("unknown option {flag}")),
            path if target.is_none() => target = Some(PathBuf::from(path)),
            extra => return Err(format!("unexpected argument {extra}")),
        }
    }
    docs.target = target.ok_or("docs needs a workspace folder")?;
    Ok(docs)
}

/// `NAME=VALUE`; the value may contain `=`.
fn pair(flag: &str, text: &str) -> Result<(String, String), String> {
    let (name, value) = text
        .split_once('=')
        .ok_or_else(|| format!("{flag} needs NAME=VALUE, got {text}"))?;
    Ok((name.trim().to_string(), value.to_string()))
}

fn number(flag: &str, text: &str) -> Result<u32, String> {
    text.parse()
        .map_err(|_| format!("{flag} needs a whole number, got {text}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_run_with_options() {
        let Ok(Command::Run(run)) = parse(args(&[
            "run",
            "api/users",
            "--env",
            "staging",
            "--bail",
            "--reporter",
            "junit",
            "-o",
            "out.xml",
            "--prompt",
            "otp=12=3",
            "--no-history",
            "--no-cookies",
            "--var",
            "userId=7",
            "--root",
            "api",
        ])) else {
            panic!("expected run");
        };
        assert_eq!(run.target, PathBuf::from("api/users"));
        assert_eq!(run.environment.as_deref(), Some("staging"));
        assert!(run.bail);
        assert_eq!(run.reporter, Reporter::Junit);
        assert_eq!(run.output, Some(PathBuf::from("out.xml")));
        assert_eq!(run.prompts["otp"], "12=3");
        assert!(!run.history);
        assert!(!run.cookies);
        assert!(!run.allow_production_writes);
        assert_eq!(run.vars["userId"], "7");
        assert_eq!(run.root, Some(PathBuf::from("api")));
    }

    #[test]
    fn parses_data_driven_options_and_docs() {
        let Ok(Command::Run(run)) = parse(args(&[
            "run",
            "api",
            "--tag",
            "smoke",
            "--tag",
            "auth",
            "--data",
            "users.csv",
            "--delay",
            "250",
            "--iterations",
            "0",
        ])) else {
            panic!("expected run");
        };
        assert_eq!(run.tags, ["smoke", "auth"]);
        assert_eq!(run.data, Some(PathBuf::from("users.csv")));
        assert_eq!(run.delay_ms, 250);
        assert_eq!(run.iterations, 1);
        assert!(parse(args(&["run", "x", "--iterations", "4000000000"])).is_err());
        assert!(parse(args(&["run", "api", "--delay", "soon"])).is_err());

        let Ok(Command::Docs(docs)) = parse(args(&["docs", "api", "-e", "prod", "-o", "d.html"]))
        else {
            panic!("expected docs");
        };
        assert_eq!(docs.environment.as_deref(), Some("prod"));
        assert_eq!(docs.output, Some(PathBuf::from("d.html")));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(args(&["run"])).is_err());
        assert!(parse(args(&["run", "a", "b"])).is_err());
        assert!(parse(args(&["run", "a", "--reporter", "xml"])).is_err());
        assert!(parse(args(&["run", "a", "--prompt", "novalue"])).is_err());
        assert!(parse(args(&["run", "a", "--var", "novalue"])).is_err());
        assert!(parse(args(&["deploy"])).is_err());
    }

    #[test]
    fn help_and_version() {
        assert_eq!(parse(args(&[])), Ok(Command::Help));
        assert_eq!(parse(args(&["--version"])), Ok(Command::Version));
    }
}
