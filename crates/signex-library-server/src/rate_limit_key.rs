//! Rate-limit key extraction for [`crate::with_rate_limit_trusted`].
//!
//! By default requests are bucketed by the TCP peer address. Behind a reverse
//! proxy every request shares the proxy's address, so an operator may list the
//! proxy addresses as trusted and name the ONE header their proxy writes
//! ([`ProxyHeader`]); only then is the client address read, and only from that
//! header. The other header is ignored entirely (a client can forge it, and a
//! proxy that overwrites only one passes the other through). Headers from any
//! other peer are ignored too.

use std::net::{IpAddr, SocketAddr};

use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request};
use tower_governor::{GovernorError, key_extractor::KeyExtractor};

/// The single forwarding header the operator's proxy is known to write.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProxyHeader {
    #[default]
    XForwardedFor,
    Forwarded,
}

impl std::str::FromStr for ProxyHeader {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "x-forwarded-for" => Ok(Self::XForwardedFor),
            "forwarded" => Ok(Self::Forwarded),
            other => Err(format!("`{other}` is not `x-forwarded-for` or `forwarded`")),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrustedProxyKeyExtractor {
    trusted: Vec<IpAddr>,
    header: ProxyHeader,
}

impl TrustedProxyKeyExtractor {
    pub fn new(trusted: Vec<IpAddr>, header: ProxyHeader) -> Self {
        // Canonical form so `::ffff:10.0.0.1` (dual-stack listener) matches `10.0.0.1`.
        let trusted = trusted.into_iter().map(|ip| ip.to_canonical()).collect();
        Self { trusted, header }
    }

    fn is_trusted(&self, ip: &IpAddr) -> bool {
        self.trusted.contains(&ip.to_canonical())
    }

    /// Walk the forwarded chain right-to-left (nearest hop first), skipping
    /// trusted proxies; the first untrusted hop is the real client. A hop that
    /// does not parse stops the walk (`None` -> caller falls back to the peer).
    fn client_from_headers(&self, headers: &HeaderMap) -> Option<IpAddr> {
        let hops: Vec<Option<IpAddr>> = match self.header {
            ProxyHeader::XForwardedFor => headers
                .get_all("x-forwarded-for")
                .iter()
                .filter_map(|v| v.to_str().ok())
                .flat_map(|s| s.split(','))
                .map(|s| parse_hop(s.trim()))
                .collect(),
            ProxyHeader::Forwarded => headers
                .get_all(axum::http::header::FORWARDED)
                .iter()
                .filter_map(|v| v.to_str().ok())
                .flat_map(|s| s.split(','))
                .map(forwarded_for)
                .collect(),
        };
        for hop in hops.into_iter().rev() {
            match hop {
                Some(ip) if self.is_trusted(&ip) => continue,
                other => return other.map(|ip| ip.to_canonical()),
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
    parse_hop(v)
}

/// One hop: bare IP, `ip:port`, `[v6]` or `[v6]:port`.
fn parse_hop(v: &str) -> Option<IpAddr> {
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
            .map(|ci| ci.0.ip().to_canonical())
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
        ex_h(ProxyHeader::XForwardedFor)
    }

    fn ex_h(h: ProxyHeader) -> TrustedProxyKeyExtractor {
        TrustedProxyKeyExtractor::new(vec!["10.0.0.1".parse().unwrap()], h)
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
        assert_eq!(
            ex_h(ProxyHeader::Forwarded).extract(&r).unwrap(),
            ip("2001:db8::1")
        );
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

    fn req2(peer: &str, hdrs: &[(&str, &str)]) -> Request<()> {
        let mut b = Request::builder();
        for (k, v) in hdrs {
            b = b.header(*k, *v);
        }
        let mut r = b.body(()).unwrap();
        r.extensions_mut().insert(ConnectInfo(
            format!("{peer}:1234").parse::<SocketAddr>().unwrap(),
        ));
        r
    }

    #[test]
    fn only_the_configured_header_is_read() {
        let r = req2(
            "10.0.0.1",
            &[
                ("x-forwarded-for", "6.6.6.6"),
                ("forwarded", "for=198.51.100.7"),
            ],
        );
        assert_eq!(
            ex_h(ProxyHeader::Forwarded).extract(&r).unwrap(),
            ip("198.51.100.7")
        );
        let r = req2(
            "10.0.0.1",
            &[
                ("x-forwarded-for", "198.51.100.7"),
                ("forwarded", "for=6.6.6.6"),
            ],
        );
        assert_eq!(
            ex_h(ProxyHeader::XForwardedFor).extract(&r).unwrap(),
            ip("198.51.100.7")
        );
        // Configured header absent: the other is never consulted.
        let r = req("10.0.0.1", Some(("x-forwarded-for", "6.6.6.6")));
        assert_eq!(
            ex_h(ProxyHeader::Forwarded).extract(&r).unwrap(),
            ip("10.0.0.1")
        );
    }

    #[test]
    fn proxy_header_parses_or_errors() {
        assert_eq!("X-Forwarded-For".parse(), Ok(ProxyHeader::XForwardedFor));
        assert_eq!("forwarded".parse(), Ok(ProxyHeader::Forwarded));
        assert!("".parse::<ProxyHeader>().is_err());
        assert!("x-real-ip".parse::<ProxyHeader>().is_err());
    }

    #[test]
    fn ipv4_mapped_peer_matches_trusted_ipv4_proxy() {
        let r = req(
            "[::ffff:10.0.0.1]",
            Some(("x-forwarded-for", "198.51.100.7")),
        );
        assert_eq!(ex().extract(&r).unwrap(), ip("198.51.100.7"));
        let plain = req("203.0.113.9", None);
        let mapped = req("[::ffff:203.0.113.9]", None);
        assert_eq!(
            ex().extract(&plain).unwrap(),
            ex().extract(&mapped).unwrap()
        );
        let r = req("10.0.0.1", Some(("x-forwarded-for", "::ffff:198.51.100.7")));
        assert_eq!(ex().extract(&r).unwrap(), ip("198.51.100.7"));
    }

    #[test]
    fn hops_with_ports_parse() {
        let r = req("10.0.0.1", Some(("x-forwarded-for", "1.2.3.4:5678")));
        assert_eq!(ex().extract(&r).unwrap(), ip("1.2.3.4"));
        let r = req("10.0.0.1", Some(("x-forwarded-for", "[2001:db8::1]:4711")));
        assert_eq!(ex().extract(&r).unwrap(), ip("2001:db8::1"));
        let r = req("10.0.0.1", Some(("forwarded", "for=1.2.3.4:5678")));
        assert_eq!(
            ex_h(ProxyHeader::Forwarded).extract(&r).unwrap(),
            ip("1.2.3.4")
        );
    }

    #[test]
    fn missing_connect_info_is_an_error() {
        let r = Request::builder().body(()).unwrap();
        assert!(ex().extract(&r).is_err());
    }
}
