//! Stub of the upstream `iced_fonts` crate.
//!
//! The upstream crate embeds a dozen icon fonts behind feature flags. Signex
//! uses none of them: `iced_aw` only re-exports this crate (`pub use
//! iced_fonts;`) and enables `advanced_text`, and nothing else in the workspace
//! names `iced_fonts`. The font-embedding modules, features and font files were
//! therefore removed (see `vendor/README.md`). The crate stays so that
//! `iced_aw`'s dependency and the `[patch.crates-io]` entry keep resolving.
//!
//! `iced_aw`'s own icon font (`vendor/iced_aw/font.ttf`) is unrelated to this
//! crate.
