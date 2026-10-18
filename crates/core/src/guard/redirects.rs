//! Where an agent's redirects and name lookups may lead. The agent can't pick a redirect target
//! or what a name resolves to, but the server and DNS it reaches can, so both are checked.

use std::collections::{BTreeMap, BTreeSet};
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::path::Path;
use std::sync::{Arc, Mutex};

use reqwest::Url;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use super::hosts::{
    is_private_host, is_private_ip, is_reserved_name, known_hosts, normal_host, production_hosts,
    url_host,
};
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRedirects {
    /// The first URL's host, as reqwest reads it.
    first: String,
    production: BTreeSet<String>,
    /// Hosts an environment or the workspace defaults point at. They are the user's own, so
    /// they may resolve to a private address: dev domains like `api.myapp.test` through
    /// dnsmasq. An approval only shows a name, so it doesn't make one of these.
    known: BTreeSet<String>,
    /// Downloads: a hop may only reach the first host or a known one.
    only_known: bool,
    /// Test seam: lookups answered from here instead of DNS.
    fixed: BTreeMap<String, IpAddr>,
}

impl AgentRedirects {
    /// For a send to `url`: never on to this machine, the local network or a production host.
    /// A URL without a host the app can check is refused, so nothing goes unchecked.
    pub fn for_sends(root: &Path, url: &str) -> Result<Self> {
        let Some(first) = url_host(url) else {
            return Err(Error::Invalid(
                "an AI agent's request needs an http(s) URL with a host the app can check".into(),
            ));
        };
        Ok(Self {
            first,
            production: production_hosts(root)?,
            known: known_hosts(root)?,
            only_known: false,
            fixed: BTreeMap::new(),
        })
    }

    /// For a download from `url`: also only on to hosts it could reach without asking.
    pub fn for_downloads(root: &Path, url: &str) -> Result<Self> {
        Ok(Self {
            only_known: true,
            ..Self::for_sends(root, url)?
        })
    }

    /// Whether a redirect to host `to` may be followed.
    pub fn allows(&self, to: &str) -> bool {
        let to = normal_host(to);
        if to == self.first {
            return true;
        }
        if is_private_host(&to) || self.production.contains(&to) {
            return false;
        }
        !self.only_known || self.known.contains(&to)
    }

    /// The name lookups of one client. Only the hosts the request reaches are checked: the
    /// first one and each redirect hop it follows. Other lookups, like a proxy's name, pass.
    /// An approval doesn't exempt a host: the user saw its name, not where it leads.
    pub fn resolver(&self) -> AgentResolver {
        let resolver = AgentResolver {
            rules: self.clone(),
            checked: Arc::default(),
        };
        // `myapp.test` through Valet or Herd leads to this machine, and nobody else can make a
        // name like that lead anywhere. A redirect to one is still checked.
        if !is_reserved_name(&self.first) {
            resolver.check(&self.first);
        }
        resolver
    }

    #[cfg(test)]
    pub(crate) fn with_fixed(mut self, name: &str, ip: IpAddr) -> Self {
        self.fixed.insert(name.to_string(), ip);
        self
    }
}

/// Refuses a host the request reaches when it looks public but resolves to this machine or a
/// private network. Clones share the hosts to check, so the client's redirect policy and its
/// resolver use one list.
#[derive(Debug, Clone)]
pub struct AgentResolver {
    rules: AgentRedirects,
    checked: Arc<Mutex<BTreeSet<String>>>,
}

impl AgentResolver {
    /// Whether a redirect to `to` may be followed; if so, its host's lookups are checked from
    /// now on. One without a host the app can check never passes.
    pub fn follows(&self, to: &Url) -> bool {
        let Some(host) = url_host(to.as_str()) else {
            return false;
        };
        if !self.rules.allows(&host) {
            return false;
        }
        self.check(&host);
        true
    }

    /// Names that look private show what they are, and known hosts are the user's own.
    fn check(&self, host: &str) {
        let host = normal_host(host);
        if is_private_host(&host) || self.rules.known.contains(&host) {
            return;
        }
        self.checked
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(host);
    }

    fn is_checked(&self, host: &str) -> bool {
        self.checked
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(host)
    }

    async fn lookup(&self, name: &str) -> std::io::Result<Vec<IpAddr>> {
        if let Some(ip) = self.rules.fixed.get(&normal_host(name)) {
            return Ok(vec![*ip]);
        }
        let name = name.to_string();
        tokio::task::spawn_blocking(move || {
            let addrs = (name.as_str(), 0).to_socket_addrs()?;
            Ok(addrs.map(|addr| addr.ip()).collect())
        })
        .await
        .map_err(std::io::Error::other)?
    }
}

impl Resolve for AgentResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let resolver = self.clone();
        let name = name.as_str().to_string();
        Box::pin(async move {
            let ips = resolver.lookup(&name).await?;
            let host = normal_host(&name);
            if resolver.is_checked(&host) && ips.iter().any(|ip| is_private_ip(*ip)) {
                return Err(format!(
                    "{host} leads to an address on this machine or a private network, which an \
                     AI agent may not reach"
                )
                .into());
            }
            let addrs: Addrs = Box::new(ips.into_iter().map(|ip| SocketAddr::new(ip, 0)));
            Ok(addrs)
        })
    }
}
