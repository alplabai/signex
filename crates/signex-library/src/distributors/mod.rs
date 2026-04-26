//! Community distributor adapters (DigiKey, Mouser, LCSC, JLCPCB).
//!
//! Gated behind the `distributors-community` Cargo feature so the core
//! library crate stays free of `reqwest`/`oauth2`/`keyring` when consumers
//! don't need vendor lookups (e.g. CI builds of `signex-app` that ship
//! without distributor integrations).
//!
//! See `.claude/PRPs/v0.9-library-plan.md` → "WS-C: Distributor adapters".

pub mod cache;
pub mod digikey;
pub mod jlcpcb;
pub mod keyring;
pub mod lcsc;
pub mod mouser;

pub use cache::{CacheError, DistributorCache, DEFAULT_TTL};
pub use digikey::DigiKeyAdapter;
pub use jlcpcb::JlcpcbAdapter;
pub use keyring::{KeyringError, KeyringStore};
pub use lcsc::LcscAdapter;
pub use mouser::MouserAdapter;
