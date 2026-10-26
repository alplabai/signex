//! Community distributor adapters (DigiKey, Mouser, LCSC, JLCPCB).
//!
//! Gated behind the `distributors-community` Cargo feature so the core
//! library crate stays free of `reqwest`/`oauth2`/`keyring` when consumers
//! don't need vendor lookups (e.g. CI builds of `signex-app` that ship
//! without distributor integrations).
//!
//! See `docs/internal/docs/v0.9-library-plan.md` → §14a Distributor Data Sources.

pub mod cache;
pub mod digikey;
mod http;
pub mod jlcpcb;
pub mod keyring;
pub mod lcsc;
pub mod mouser;
pub(crate) mod price;
mod throttle;

pub use cache::{CacheError, DEFAULT_TTL, DistributorCache};
pub use digikey::DigiKeyAdapter;
pub use jlcpcb::JlcpcbAdapter;
pub use keyring::{KeyringError, KeyringStore};
pub use lcsc::LcscAdapter;
pub use mouser::MouserAdapter;

/// Sources whose client returns prices ([`crate::DistributorAdapter::supports_pricing`]).
///
/// The UI decides from this whether Refresh All Pricing is worth offering
/// without building a client (which touches the OS keyring). A test below
/// keeps it equal to what each client reports.
pub const PRICING_SOURCES: &[crate::DistributorSource] = &[
    crate::DistributorSource::DigiKey,
    crate::DistributorSource::Mouser,
];

/// Whether the client for `source` returns prices; see [`PRICING_SOURCES`].
pub fn source_supports_pricing(source: crate::DistributorSource) -> bool {
    PRICING_SOURCES.contains(&source)
}

/// Dot-anchored domain match: `host` is `domain` itself or a subdomain of it.
/// A bare `ends_with` would also accept `evildigikey.com`.
pub(crate) fn host_in_domain(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|rest| rest.ends_with('.'))
}

#[cfg(test)]
mod host_tests {
    use super::host_in_domain;

    #[test]
    fn host_match_is_dot_anchored() {
        assert!(host_in_domain("digikey.com", "digikey.com"));
        assert!(host_in_domain("www.digikey.com", "digikey.com"));
        assert!(!host_in_domain("evildigikey.com", "digikey.com"));
        assert!(!host_in_domain("digikey.com.evil.io", "digikey.com"));
    }
}

#[cfg(test)]
mod pricing_sources_tests {
    use super::*;
    use crate::DistributorAdapter;

    /// `PRICING_SOURCES` is what the UI reads without building a client; it
    /// must say what each client says.
    #[test]
    fn pricing_sources_match_what_each_client_reports() {
        let clients: Vec<Box<dyn DistributorAdapter>> = vec![
            Box::new(
                DigiKeyAdapter::with_access_token("http://unused.invalid", "t", None)
                    .with_client_id("client-id"),
            ),
            Box::new(MouserAdapter::with_api_key(
                "http://unused.invalid",
                "k",
                None,
            )),
            Box::new(LcscAdapter::with_base_url("http://unused.invalid", None)),
            Box::new(JlcpcbAdapter::with_base_url("http://unused.invalid", None)),
        ];
        for c in &clients {
            assert_eq!(
                source_supports_pricing(c.source()),
                c.supports_pricing(),
                "{} disagrees with PRICING_SOURCES",
                c.name()
            );
        }
    }
}
