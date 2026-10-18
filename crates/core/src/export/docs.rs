//! One self-contained HTML page documenting a workspace, with no scripts or external assets,
//! so it can be opened anywhere.

use std::path::Path;

use super::effective_headers;
use crate::http_file::{Header, Request};
use crate::workspace::{self, TreeNode};
use crate::{Result, environments, examples, request_meta, requests};

const STYLE: &str = "\
:root{color-scheme:light dark;--bg:#fff;--fg:#1c1c22;--muted:#6b6b76;--line:#e4e4ea;--code:#f5f5f8;--accent:#0e7490}\
@media (prefers-color-scheme:dark){:root{--bg:#141418;--fg:#ececf1;--muted:#9a9aa6;--line:#2c2c34;--code:#1d1d23;--accent:#22d3ee}}\
*{box-sizing:border-box}body{margin:0;font:15px/1.6 system-ui,sans-serif;background:var(--bg);color:var(--fg)}\
nav{position:fixed;inset:0 auto 0 0;width:280px;overflow:auto;padding:24px 16px;border-right:1px solid var(--line);font-size:13px}\
nav a{display:block;padding:2px 6px;border-radius:4px;color:inherit;text-decoration:none}nav a:hover{background:var(--code)}\
nav .folder{margin-top:12px;font-weight:600;color:var(--muted)}main{margin-left:280px;padding:32px 48px;max-width:1000px}\
h1{margin-top:0}h2{margin-top:48px;border-bottom:1px solid var(--line);padding-bottom:6px}section{margin:32px 0}\
.method{display:inline-block;min-width:64px;font:700 12px ui-monospace,monospace;color:var(--accent)}\
.url{font:13px ui-monospace,monospace;word-break:break-all}pre,code{font:13px ui-monospace,monospace;background:var(--code);border-radius:6px}\
pre{padding:12px;overflow:auto;white-space:pre-wrap}code{padding:1px 4px}table{border-collapse:collapse;font-size:13px}\
td,th{border-bottom:1px solid var(--line);padding:4px 12px 4px 0;text-align:left;vertical-align:top}.muted{color:var(--muted)}";

/// The documentation page. With `environment`, its plain variables are listed (never secrets).
pub fn html(root: &Path, environment: Option<&str>) -> Result<String> {
    let title = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "API".into());
    let tree = workspace::tree(root)?;
    let mut nav = String::new();
    let mut body = String::new();
    write_nodes(root, &tree, 2, &mut nav, &mut body)?;

    let mut intro = String::new();
    if let Some(name) = environment {
        let env = environments::get(root, name)?;
        intro.push_str(&format!(
            "<h2 id=\"environment\">Environment {}</h2><table>",
            escape(&env.name)
        ));
        for (key, value) in &env.variables {
            intro.push_str(&format!(
                "<tr><td><code>{}</code></td><td class=\"url\">{}</td></tr>",
                escape(key),
                escape(value)
            ));
        }
        for secret in &env.secrets {
            intro.push_str(&format!(
                "<tr><td><code>{}</code></td><td class=\"muted\">secret</td></tr>",
                escape(&secret.name)
            ));
        }
        intro.push_str("</table>");
    }

    Ok(format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
         content=\"width=device-width,initial-scale=1\"><title>{t}</title><style>{STYLE}</style></head>\
         <body><nav><strong>{t}</strong>{nav}</nav><main><h1>{t}</h1>{intro}{body}</main></body></html>\n",
        t = escape(&title)
    ))
}

fn write_nodes(
    root: &Path,
    nodes: &[TreeNode],
    level: usize,
    nav: &mut String,
    body: &mut String,
) -> Result<()> {
    for node in nodes {
        match node {
            TreeNode::Folder {
                name,
                path,
                children,
            } => {
                let anchor = anchor(path);
                nav.push_str(&format!(
                    "<a class=\"folder\" href=\"#{anchor}\">{}</a>",
                    escape(name)
                ));
                let h = level.min(4);
                body.push_str(&format!("<h{h} id=\"{anchor}\">{}</h{h}>", escape(name)));
                write_nodes(root, children, level + 1, nav, body)?;
            }
            TreeNode::Request {
                id, name, method, ..
            } => {
                let anchor = anchor(id);
                nav.push_str(&format!(
                    "<a href=\"#{anchor}\"><span class=\"method\">{}</span>{}</a>",
                    escape(method),
                    escape(name)
                ));
                let request = requests::get(root, id)?;
                body.push_str(&request_section(root, id, name, &anchor, &request)?);
            }
        }
    }
    Ok(())
}

fn request_section(
    root: &Path,
    id: &str,
    name: &str,
    anchor: &str,
    request: &Request,
) -> Result<String> {
    let mut out = format!(
        "<section id=\"{anchor}\"><h3>{}</h3><p><span class=\"method\">{}</span> <span class=\"url\">{}</span></p>",
        escape(name),
        escape(&request.method.to_uppercase()),
        escape(&request.url)
    );
    let docs = request_meta::docs(request);
    if !docs.is_empty() {
        out.push_str(&markdown(&docs));
    }
    out.push_str(&headers_table(&effective_headers(
        root,
        id,
        &request.headers,
    )?));
    if !request.body.is_empty() {
        out.push_str(&format!(
            "<h4>Body</h4><pre>{}</pre>",
            escape(&pretty(&request.body))
        ));
    }
    out.push_str(&assertions_list(&request.assertions));
    for example in examples::list(root, id)? {
        out.push_str(&format!(
            "<h4>Example: {} <span class=\"muted\">({})</span></h4><pre>{}</pre>",
            escape(&example.name),
            example.status,
            escape(&pretty(&example.body))
        ));
    }
    out.push_str(&format!(
        "<p class=\"muted\"><code>{}</code></p></section>",
        escape(id)
    ));
    Ok(out)
}

fn headers_table(headers: &[Header]) -> String {
    if headers.is_empty() {
        return String::new();
    }
    let rows: String = headers
        .iter()
        .map(|h| {
            format!(
                "<tr><td><code>{}</code></td><td class=\"url\">{}</td></tr>",
                escape(&h.name),
                escape(&h.value)
            )
        })
        .collect();
    format!("<h4>Headers</h4><table>{rows}</table>")
}

fn assertions_list(assertions: &[String]) -> String {
    if assertions.is_empty() {
        return String::new();
    }
    let items: String = assertions
        .iter()
        .map(|a| format!("<li><code>{}</code></li>", escape(a)))
        .collect();
    format!("<h4>Assertions</h4><ul>{items}</ul>")
}

/// A small, safe subset of Markdown: paragraphs, `- ` lists, `#` headings, `code` and
/// **bold**. Everything is escaped first, so docs can't inject HTML.
fn markdown(text: &str) -> String {
    let mut out = String::new();
    let mut in_list = false;
    for block in text.split("\n\n") {
        for line in block.lines() {
            let line = line.trim_end();
            if let Some(item) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
                if !in_list {
                    out.push_str("<ul>");
                    in_list = true;
                }
                out.push_str(&format!("<li>{}</li>", inline(item)));
                continue;
            }
            if in_list {
                out.push_str("</ul>");
                in_list = false;
            }
            if let Some(heading) = line.strip_prefix('#') {
                out.push_str(&format!(
                    "<h4>{}</h4>",
                    inline(heading.trim_start_matches('#').trim())
                ));
            } else if !line.is_empty() {
                out.push_str(&format!("<p>{}</p>", inline(line)));
            }
        }
    }
    if in_list {
        out.push_str("</ul>");
    }
    out
}

fn inline(text: &str) -> String {
    let escaped = escape(text);
    let code = wrap_pairs(&escaped, "`", "<code>", "</code>");
    wrap_pairs(&code, "**", "<strong>", "</strong>")
}

/// Replaces matched pairs of `marker` with open/close tags; an unmatched marker stays as is.
fn wrap_pairs(text: &str, marker: &str, open: &str, close: &str) -> String {
    let parts: Vec<&str> = text.split(marker).collect();
    if parts.len() < 3 {
        return text.to_string();
    }
    let mut out = String::new();
    let pairs = (parts.len() - 1) / 2 * 2;
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            match i <= pairs {
                true if i % 2 == 1 => out.push_str(open),
                true => out.push_str(close),
                false => out.push_str(marker),
            }
        }
        out.push_str(part);
    }
    out
}

fn pretty(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .and_then(|v| serde_json::to_string_pretty(&v))
        .unwrap_or_else(|_| body.to_string())
}

fn anchor(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

pub(crate) fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
