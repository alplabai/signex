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
mod throttle;

pub use cache::{CacheError, DEFAULT_TTL, DistributorCache};
pub use digikey::DigiKeyAdapter;
pub use jlcpcb::JlcpcbAdapter;
pub use keyring::{KeyringError, KeyringStore};
pub use lcsc::LcscAdapter;
pub use mouser::MouserAdapter;

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
