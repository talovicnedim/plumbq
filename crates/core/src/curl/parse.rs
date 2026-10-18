use base64::Engine;

use super::form;
use super::shell::tokenize;
use crate::http_file::{Header, Request};
use crate::{Error, Result};

/// Parses a `curl …` command (as copied from browser dev tools or docs) into a request.
/// Flags that only change curl's own output (`-s`, `-v`, `-L`, …) are ignored.
pub fn parse(command: &str) -> Result<Request> {
    let tokens = tokenize(command)?;
    let mut args = tokens.iter().map(String::as_str);
    match args.next() {
        Some("curl") => {}
        _ => return Err(Error::Invalid("not a curl command".into())),
    }
    let CurlArgs {
        mut request,
        data,
        form,
        get_with_data,
    } = read_args(args)?;
    if request.url.is_empty() {
        return Err(Error::Invalid("curl command has no url".into()));
    }
    if form.is_empty() {
        add_data(&mut request, data, get_with_data);
    } else {
        add_form(&mut request, &form);
    }
    if request.method.is_empty() {
        request.method = "GET".into();
    }
    Ok(request)
}

/// What the flags say, before the body is built from `data` or `form`.
struct CurlArgs {
    request: Request,
    data: Vec<String>,
    form: Vec<form::FormField>,
    get_with_data: bool,
}

fn read_args<'a>(mut args: impl Iterator<Item = &'a str>) -> Result<CurlArgs> {
    let mut out = CurlArgs {
        request: Request::new("", "", ""),
        data: Vec::new(),
        form: Vec::new(),
        get_with_data: false,
    };
    let request = &mut out.request;
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .map(str::to_string)
                .ok_or_else(|| Error::Invalid(format!("{arg} needs a value")))
        };
        match arg {
            "-X" | "--request" => request.method = value()?.to_uppercase(),
            "-H" | "--header" => {
                if let Some((name, val)) = value()?.split_once(':') {
                    request.headers.push(header(name.trim(), val.trim()));
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-ascii" => {
                out.data.push(value()?)
            }
            "--data-urlencode" => out.data.push(form::urlencode(&value()?)),
            "-F" | "--form" | "--form-string" => {
                let spec = value()?;
                let field = form::field(&spec, arg == "--form-string")
                    .ok_or_else(|| Error::Invalid(format!("{arg} needs name=value, got {spec}")))?;
                out.form.push(field);
            }
            "--json" => {
                out.data.push(value()?);
                set_default_header(request, "Content-Type", "application/json");
                set_default_header(request, "Accept", "application/json");
            }
            "-u" | "--user" => {
                let encoded = base64::engine::general_purpose::STANDARD.encode(value()?);
                request
                    .headers
                    .push(header("Authorization", &format!("Basic {encoded}")));
            }
            "-A" | "--user-agent" => request.headers.push(header("User-Agent", &value()?)),
            "-b" | "--cookie" => request.headers.push(header("Cookie", &value()?)),
            "-e" | "--referer" => request.headers.push(header("Referer", &value()?)),
            "-G" | "--get" => out.get_with_data = true,
            "-I" | "--head" => request.method = "HEAD".into(),
            "--url" => request.url = value()?,
            // Options with a value we don't use.
            "-o" | "--output" | "-m" | "--max-time" | "--connect-timeout" | "-x" | "--proxy" => {
                value()?;
            }
            flag if flag.starts_with('-') => {}
            url => request.url = url.to_string(),
        }
    }
    Ok(out)
}

fn add_form(request: &mut Request, form: &[form::FormField]) {
    let (content_type, body, absolute) = form::multipart(form);
    request
        .headers
        .retain(|h| !h.name.eq_ignore_ascii_case("content-type"));
    request.headers.push(header("Content-Type", &content_type));
    request.body = body;
    if absolute {
        request
            .comments
            .push("# file path from curl, move the file into the workspace".into());
    }
    if request.method.is_empty() {
        request.method = "POST".into();
    }
}

/// `-G` moves the data into the query string; otherwise it becomes a form body.
fn add_data(request: &mut Request, data: Vec<String>, get_with_data: bool) {
    let body = data.join("&");
    if body.is_empty() {
        return;
    }
    if get_with_data {
        let separator = if request.url.contains('?') { '&' } else { '?' };
        request.url = format!("{}{separator}{body}", request.url);
        return;
    }
    request.body = body;
    if request.method.is_empty() {
        request.method = "POST".into();
    }
    set_default_header(request, "Content-Type", "application/x-www-form-urlencoded");
}

pub(super) fn header(name: &str, value: &str) -> Header {
    Header {
        name: name.to_string(),
        value: value.to_string(),
    }
}

fn set_default_header(request: &mut Request, name: &str, value: &str) {
    if !request
        .headers
        .iter()
        .any(|h| h.name.eq_ignore_ascii_case(name))
    {
        request.headers.push(header(name, value));
    }
}
