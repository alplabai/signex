//! Signex EDA — AI-first electronics design automation.
//!
//! Thin binary entrypoint. The real implementation lives in the
//! library face (`lib.rs`) so integration tests can drive
//! dispatchers without spinning up the iced runtime.

fn main() -> iced::Result {
    // The Community binary installs no extension (docs/EXTENSION_API.md).
    signex_app::launch::run(Vec::new())
}
