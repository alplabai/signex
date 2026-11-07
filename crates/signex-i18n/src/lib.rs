//! Message catalogs and locale-aware formatting for the Signex UI.
//!
//! The only crate that imports Fluent or ICU4X; a backend change touches
//! this crate alone. It is a UI-layer crate: the domain crates cannot depend
//! on it (`scripts/check_arch_guard.sh`).
//!
//! An [`I18n`] is plain data held in the app state, never a global. [`t!`]
//! renders a message to [`Localized`], which is for display only; state and
//! files keep a [`MessageId`] and its arguments.

mod bundle;
mod catalog;
mod i18n;
mod locale;
mod numbers;
mod types;

pub use i18n::{I18n, LanguageOption};
pub use locale::LanguagePref;
pub use types::{Arg, Localized, MessageId, UserText};

/// Message ids this crate renders itself.
pub mod ids {
    use crate::MessageId;

    pub const LANGUAGE_SYSTEM_DEFAULT: MessageId = MessageId::new("language-system-default");
    pub const MENU_FILE: MessageId = MessageId::new("menu-file");
}

/// Render a message: `t!(i18n, ids::MENU_FILE)` or, with arguments,
/// `t!(i18n, ID, "count" => 3_i64, "name" => "R1")`.
#[macro_export]
macro_rules! t {
    ($i18n:expr, $id:expr) => {
        $i18n.message($id, &[])
    };
    ($i18n:expr, $id:expr, $($key:literal => $value:expr),+ $(,)?) => {
        $i18n.message($id, &[$(($key, $crate::Arg::from($value))),+])
    };
}

#[cfg(test)]
mod tests;
