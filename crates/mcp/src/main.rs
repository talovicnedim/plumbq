//! MCP server for a Plumbq workspace: lets AI assistants (Claude, Cursor, VS Code, …)
//! read, organize and send the requests in a folder of `.http` files.
//!
//! ```text
//! plumbq-mcp --url http://127.0.0.1:7459/mcp/<token>
//! plumbq-mcp --workspace ~/projects/my-api [--read-only] [--allow-env] [--import-dir DIR]
//! ```
//!
//! With `--url` it is a stdio bridge to the server running inside the app: what Claude Desktop
//! and Codex start (AI → AI apps), so they work on the workspace open in the app.
//!
//! With `--workspace` it is a standalone server for one folder, for headless use (e.g. a CI
//! box). It works on the files directly; the app picks up every change through its file
//! watcher. What agents may do follows the permissions saved in the app's MCP settings. Anything
//! that needs approval is asked in the running app and denied when nobody answers.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::{Context, bail};
use plumbq_mcp::{Approver, Bridge, McpSettings, PlumbqMcp, Workspace, settings};
use rmcp::ServiceExt;
use rmcp::transport::stdio;

const USAGE: &str = "usage: plumbq-mcp --url <address>
       plumbq-mcp --workspace <folder> [--read-only] [--allow-env] [--import-dir <folder>]

  --url ADDRESS      forward to the MCP server running in the app (AI → AI apps shows the
                     address); approvals pop up there
  --workspace DIR    serve one folder without the app's server (headless use)
  --read-only        only read files and send GET, HEAD and OPTIONS; never ask for approval
  --allow-env        resolve {{$processEnv NAME}} / {{$env.NAME}} (off unless allowed in the
                     app's MCP settings: they read this process's environment)
  --import-dir DIR   besides the workspace, the folder import_file may read from";

/// Parsed command line.
#[derive(Debug, PartialEq)]
enum Mode {
    /// A stdio bridge to the app's HTTP server.
    Bridge {
        url: String,
    },
    Standalone(Args),
}

#[derive(Debug, Default, PartialEq)]
struct Args {
    root: PathBuf,
    read_only: bool,
    allow_env: bool,
    import_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // stdout carries the protocol; logs go to stderr.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
    plumbq_core::http_client::install_crypto();

    match parse_args(std::env::args().skip(1))? {
        Mode::Bridge { url } => bridge(url).await,
        Mode::Standalone(args) => standalone(args).await,
    }
}

async fn bridge(url: String) -> anyhow::Result<()> {
    tracing::info!("forwarding to the app's mcp server");
    let service = Bridge::new(url)
        .serve(stdio())
        .await
        .context("failed to start the mcp bridge")?;
    service.waiting().await?;
    Ok(())
}

async fn standalone(args: Args) -> anyhow::Result<()> {
    let root = plumbq_core::workspace::canonical_root(&args.root)
        .with_context(|| format!("workspace {} not found", args.root.display()))?;
    let server = standalone_server(&args, &root, plumbq_core::app_dirs::data_dir())?;
    tracing::info!(
        workspace = %root.display(),
        read_only = args.read_only,
        allow_env = args.allow_env,
        tools = server.tool_names().len(),
        "starting mcp server"
    );
    let service = server
        .serve(stdio())
        .await
        .context("failed to start the mcp server")?;
    service.waiting().await?;
    Ok(())
}

/// A server on one folder that shares history, cookies, settings, approvals and the activity
/// log with the app through the app data dir.
fn standalone_server(
    args: &Args,
    root: &Path,
    data_dir: Option<PathBuf>,
) -> anyhow::Result<PlumbqMcp> {
    let import_dir = match &args.import_dir {
        Some(dir) => Some(
            plumbq_core::workspace::canonical_root(dir)
                .with_context(|| format!("import folder {} not found", dir.display()))?,
        ),
        None => None,
    };
    let settings_file = data_dir.as_deref().map(settings::path);
    let saved = match &settings_file {
        Some(file) => settings::load(file)
            .with_context(|| format!("failed to read MCP settings {}", file.display()))?,
        None => McpSettings::default(),
    };
    let approver = match &data_dir {
        Some(dir) => Approver::files(dir.clone()),
        None => Approver::Nobody,
    };
    let server = PlumbqMcp::new(
        Workspace::Fixed(root.to_path_buf()),
        Arc::new(RwLock::new(saved)),
        approver,
    )
    .with_history_dir(data_dir.as_ref().map(|dir| dir.join("history")))
    .with_cookie_dir(data_dir.as_ref().map(|dir| dir.join("cookies")))
    .with_import_dir(import_dir)
    .with_activity_dir(data_dir.clone())
    .with_settings_file(settings_file)
    .with_process_env(args.allow_env);
    Ok(if args.read_only {
        server.read_only()
    } else {
        server
    })
}

fn parse_args(args: impl Iterator<Item = String>) -> anyhow::Result<Mode> {
    let mut root = None;
    let mut url = None;
    let mut parsed = Args::default();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--url" => {
                let value = args
                    .next()
                    .with_context(|| format!("--url needs the server address\n{USAGE}"))?;
                url = Some(value);
            }
            "--workspace" | "-w" => root = args.next().map(PathBuf::from),
            "--read-only" => parsed.read_only = true,
            "--allow-env" => parsed.allow_env = true,
            "--import-dir" => {
                let dir = args
                    .next()
                    .with_context(|| format!("--import-dir needs a folder\n{USAGE}"))?;
                parsed.import_dir = Some(PathBuf::from(dir));
            }
            "--help" | "-h" => bail!(USAGE),
            other => bail!("unknown argument {other}\n{USAGE}"),
        }
    }
    match (url, root) {
        (Some(url), None) if parsed == Args::default() => {
            if !url.starts_with("http://") && !url.starts_with("https://") {
                bail!("--url must be an http(s) address\n{USAGE}");
            }
            Ok(Mode::Bridge { url })
        }
        (Some(_), _) => bail!("--url can't be combined with other options\n{USAGE}"),
        (None, root) => {
            parsed.root = root.with_context(|| format!("missing --workspace or --url\n{USAGE}"))?;
            Ok(Mode::Standalone(parsed))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn parses_workspace_and_flags() {
        let parsed = parse_args(args(&[
            "--workspace",
            "/ws",
            "--read-only",
            "--allow-env",
            "--import-dir",
            "/dl",
        ]))
        .unwrap();
        assert_eq!(
            parsed,
            Mode::Standalone(Args {
                root: PathBuf::from("/ws"),
                read_only: true,
                allow_env: true,
                import_dir: Some(PathBuf::from("/dl")),
            })
        );
        assert_eq!(
            parse_args(args(&["-w", "/ws"])).unwrap(),
            Mode::Standalone(Args {
                root: PathBuf::from("/ws"),
                ..Args::default()
            })
        );
    }

    #[test]
    fn url_starts_the_bridge_alone() {
        let url = "http://127.0.0.1:7459/mcp/abc";
        assert_eq!(
            parse_args(args(&["--url", url])).unwrap(),
            Mode::Bridge { url: url.into() }
        );
        assert!(parse_args(args(&["--url"])).is_err());
        assert!(parse_args(args(&["--url", "ftp://x"])).is_err());
        assert!(parse_args(args(&["--url", url, "-w", "/ws"])).is_err());
        assert!(parse_args(args(&["--url", url, "--read-only"])).is_err());
    }

    #[test]
    fn allow_env_keeps_following_the_apps_saved_settings() {
        let data = std::env::temp_dir().join(format!("ac-mcp-allow-env-{}", std::process::id()));
        std::fs::create_dir_all(&data).unwrap();
        let file = settings::path(&data);
        let args = Args {
            allow_env: true,
            ..Args::default()
        };
        let server = standalone_server(&args, Path::new("/ws"), Some(data.clone())).unwrap();
        assert!(server.permissions().allow_process_env);

        let token = settings::new_token();
        let saved = format!(r#"{{"enabled":true,"port":7459,"token":"{token}","readOnly":true}}"#);
        std::fs::write(&file, saved).unwrap();
        assert_eq!(
            server.permissions(),
            plumbq_core::permissions::AgentPermissions::read_only()
        );
        std::fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn requires_workspace() {
        assert!(parse_args(args(&["--read-only"])).is_err());
        assert!(parse_args(args(&["--bogus"])).is_err());
        assert!(parse_args(args(&["-w", "/ws", "--import-dir"])).is_err());
    }
}
