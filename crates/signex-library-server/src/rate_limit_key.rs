//! Rate-limit key extraction for [`crate::with_rate_limit_trusted`].
//!
//! By default requests are bucketed by the TCP peer address. Behind a reverse
//! proxy every request shares the proxy's address, so an operator may list the
//! proxy addresses as trusted; only then is the client address read from
//! `X-Forwarded-For` / `Forwarded`. Headers from any other peer are ignored,
//! since a direct client could otherwise forge them to dodge the limit.

use std::net::{IpAddr, SocketAddr};

use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request};
use tower_governor::{GovernorError, key_extractor::KeyExtractor};

#[derive(Debug, Clone, Default)]
pub struct TrustedProxyKeyExtractor {
    trusted: Vec<IpAddr>,
}

impl TrustedProxyKeyExtractor {
    pub fn new(trusted: Vec<IpAddr>) -> Self {
        Self { trusted }
    }

    fn is_trusted(&self, ip: &IpAddr) -> bool {
        self.trusted.contains(ip)
    }

    /// Walk the forwarded chain right-to-left (nearest hop first), skipping
    /// trusted proxies; the first untrusted hop is the real client. A hop that
    /// does not parse stops the walk (`None` -> caller falls back to the peer).
    fn client_from_headers(&self, headers: &HeaderMap) -> Option<IpAddr> {
        let hops: Vec<Option<IpAddr>> = if headers.contains_key("x-forwarded-for") {
            headers
                .get_all("x-forwarded-for")
                .iter()
                .filter_map(|v| v.to_str().ok())
                .flat_map(|s| s.split(','))
                .map(|s| s.trim().parse::<IpAddr>().ok())
                .collect()
        } else {
            headers
                .get_all(axum::http::header::FORWARDED)
                .iter()
                .filter_map(|v| v.to_str().ok())
                .flat_map(|s| s.split(','))
                .map(forwarded_for)
                .collect()
        };
        for hop in hops.into_iter().rev() {
            match hop {
                Some(ip) if self.is_trusted(&ip) => continue,
                other => return other,
            }
        }
        None
    }
}

/// `for=` element of one `Forwarded` entry (`for=192.0.2.1`, `for="[2001:db8::1]:80"`).
fn forwarded_for(entry: &str) -> Option<IpAddr> {
    let v = entry
        .split(';')
        .map(str::trim)
        .find_map(|p| {
            p.get(..4)
                .filter(|h| h.eq_ignore_ascii_case("for="))
                .map(|_| &p[4..])
        })?
        .trim_matches('"');
    v.parse::<SocketAddr>()
        .map(|s| s.ip())
        .or_else(|_| v.trim_matches(['[', ']']).parse::<IpAddr>())
        .ok()
}

impl KeyExtractor for TrustedProxyKeyExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        let peer = req
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0.ip())
            .ok_or(GovernorError::UnableToExtractKey)?;
        if self.is_trusted(&peer) {
            Ok(self.client_from_headers(req.headers()).unwrap_or(peer))
        } else {
            Ok(peer)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(peer: &str, hdr: Option<(&str, &str)>) -> Request<()> {
        let mut b = Request::builder();
        if let Some((k, v)) = hdr {
            b = b.header(k, v);
        }
        let mut r = b.body(()).unwrap();
        r.extensions_mut().insert(ConnectInfo(
            format!("{peer}:1234").parse::<SocketAddr>().unwrap(),
        ));
        r
    }

    fn ex() -> TrustedProxyKeyExtractor {
        TrustedProxyKeyExtractor::new(vec!["10.0.0.1".parse().unwrap()])
    }

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn untrusted_peer_cannot_spoof_headers() {
        let r = req("203.0.113.9", Some(("x-forwarded-for", "1.2.3.4")));
        assert_eq!(ex().extract(&r).unwrap(), ip("203.0.113.9"));
    }

    #[test]
    fn trusted_peer_uses_forwarded_client() {
        let r = req("10.0.0.1", Some(("x-forwarded-for", "198.51.100.7")));
        assert_eq!(ex().extract(&r).unwrap(), ip("198.51.100.7"));
        let r = req(
            "10.0.0.1",
            Some(("forwarded", "for=\"[2001:db8::1]:80\";proto=https")),
        );
        assert_eq!(ex().extract(&r).unwrap(), ip("2001:db8::1"));
    }

    #[test]
    fn client_supplied_leftmost_hop_is_ignored() {
        // The client sent "6.6.6.6"; the proxy appended the real peer.
        let r = req(
            "10.0.0.1",
            Some(("x-forwarded-for", "6.6.6.6, 198.51.100.7, 10.0.0.1")),
        );
        assert_eq!(ex().extract(&r).unwrap(), ip("198.51.100.7"));
    }

    #[test]
    fn trusted_peer_without_headers_or_garbage_falls_back_to_peer() {
        assert_eq!(
            ex().extract(&req("10.0.0.1", None)).unwrap(),
            ip("10.0.0.1")
        );
        let r = req("10.0.0.1", Some(("x-forwarded-for", "not-an-ip")));
        assert_eq!(ex().extract(&r).unwrap(), ip("10.0.0.1"));
    }

    #[test]
    fn empty_trusted_list_keys_by_peer() {
        let r = req("10.0.0.1", Some(("x-forwarded-for", "1.2.3.4")));
        let e = TrustedProxyKeyExtractor::default();
        assert_eq!(e.extract(&r).unwrap(), ip("10.0.0.1"));
    }

    #[test]
    fn missing_connect_info_is_an_error() {
        let r = Request::builder().body(()).unwrap();
        assert!(ex().extract(&r).is_err());
    }
}
