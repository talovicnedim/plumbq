//! The HTTP client for a send, built from [`ClientOptions`].
//! Settings: `docs/formats.md` (Workspace HTTP settings).

mod settings;

use std::fs;
use std::path::Path;

use crate::guard::{AgentRedirects, AgentResolver};
use crate::{Error, Result, client_pfx};

pub use settings::{
    ClientOptions, CrossOriginRedirects, HttpDefaults, HttpSettings, defaults, options,
    workspace_settings,
};

const MAX_REDIRECTS: usize = 10;

/// Makes ring rustls's crypto provider (reqwest is built without one, so aws-lc-rs stays out).
/// Binaries call it once at startup; client builders call it too, so tests and library users
/// never hit reqwest's "no provider" panic. Installing twice is a no-op.
pub fn install_crypto() {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
}

pub fn build(options: &ClientOptions) -> Result<reqwest::Client> {
    install_crypto();
    let agent = options
        .agent_redirects
        .as_ref()
        .map(AgentRedirects::resolver);
    let mut builder = reqwest::Client::builder()
        .timeout(options.timeout)
        .user_agent(concat!("plumbq/", env!("CARGO_PKG_VERSION")))
        .tls_danger_accept_invalid_certs(!options.verify_tls)
        .redirect(redirect_policy(options, agent.clone()));
    if let Some(timeout) = options.connect_timeout {
        builder = builder.connect_timeout(timeout);
    }
    if let Some(resolver) = agent {
        builder = builder.dns_resolver(resolver);
    }
    match options.proxy.as_str() {
        "" => {}
        "none" => builder = builder.no_proxy(),
        url => {
            let proxy = reqwest::Proxy::all(url)
                .map_err(|err| Error::Invalid(format!("invalid proxy {url}: {err}")))?;
            builder = builder.proxy(proxy);
        }
    }
    if let Some(path) = &options.ca_cert {
        for cert in ca_certificates(path)? {
            builder = builder.add_root_certificate(cert);
        }
    }
    if let Some(identity) = client_identity(options)? {
        builder = builder.identity(identity);
    }
    builder
        .build()
        .map_err(|err| Error::Http(format!("failed to create http client: {err}")))
}

fn ca_certificates(path: &Path) -> Result<Vec<reqwest::Certificate>> {
    reqwest::Certificate::from_pem_bundle(&read(path, "CA certificate")?)
        .map_err(|err| Error::Invalid(format!("invalid CA certificate {}: {err}", path.display())))
}

/// The client certificate from a PFX file or a PEM certificate (plus key), if one is set.
fn client_identity(options: &ClientOptions) -> Result<Option<reqwest::Identity>> {
    if options.client_cert.is_some() && options.client_pfx.is_some() {
        return Err(Error::Invalid(
            "set either a PEM client certificate or a PFX file, not both".into(),
        ));
    }
    if let Some(path) = &options.client_pfx {
        let pem = client_pfx::to_pem(
            &read(path, "PFX client certificate")?,
            &options.client_pfx_password,
        )
        .map_err(|err| Error::Invalid(format!("{}: {err}", path.display())))?;
        let identity = reqwest::Identity::from_pem(&pem).map_err(|err| {
            Error::Invalid(format!(
                "invalid PFX client certificate {}: {err}",
                path.display()
            ))
        })?;
        return Ok(Some(identity));
    }
    let Some(path) = &options.client_cert else {
        return Ok(None);
    };
    let mut pem = read(path, "client certificate")?;
    if let Some(key) = &options.client_key {
        pem.push(b'\n');
        pem.extend(read(key, "client key")?);
    }
    let identity = reqwest::Identity::from_pem(&pem).map_err(|err| {
        Error::Invalid(format!(
            "invalid client certificate {} (PEM certificate and private key): {err}",
            path.display()
        ))
    })?;
    Ok(Some(identity))
}

/// Follows up to [`MAX_REDIRECTS`], except the redirects to another origin that
/// `cross_origin_redirects` or the agent's rules stop.
fn redirect_policy(
    options: &ClientOptions,
    agent: Option<AgentResolver>,
) -> reqwest::redirect::Policy {
    if !options.follow_redirects {
        return reqwest::redirect::Policy::none();
    }
    let cross_origin = options.cross_origin_redirects;
    reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= MAX_REDIRECTS {
            return attempt.error("too many redirects");
        }
        let Some(first) = attempt.previous().first() else {
            return attempt.follow();
        };
        if same_origin(first, attempt.url()) {
            return attempt.follow();
        }
        let resends_body = matches!(attempt.status().as_u16(), 307 | 308);
        let stop = match cross_origin {
            CrossOriginRedirects::Follow => false,
            CrossOriginRedirects::StopBodyResends => resends_body,
            CrossOriginRedirects::Stop => true,
        } || agent
            .as_ref()
            .is_some_and(|resolver| !resolver.follows(attempt.url()));
        match stop {
            true => attempt.stop(),
            false => attempt.follow(),
        }
    })
}

/// The same origin, or the same host moving from `http` on port 80 to `https` on 443: secrets
/// stay with the same server, only safer.
pub(crate) fn same_origin(from: &reqwest::Url, to: &reqwest::Url) -> bool {
    if from.origin() == to.origin() {
        return true;
    }
    from.scheme() == "http"
        && to.scheme() == "https"
        && from.host_str() == to.host_str()
        && from.port_or_known_default() == Some(80)
        && to.port_or_known_default() == Some(443)
}

fn read(path: &Path, what: &str) -> Result<Vec<u8>> {
    fs::read(path)
        .map_err(|err| Error::Invalid(format!("failed to read {what} {}: {err}", path.display())))
}

#[cfg(test)]
mod tests;
