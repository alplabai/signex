//! Message catalogs and locale-aware formatting for the Signex UI.
//!
//! The only crate that imports Fluent or ICU4X; a backend change touches
//! this crate alone. It is a UI-layer crate: the domain crates cannot depend
//! on it (`scripts/check_arch_guard.sh`).
//!
//! An [`I18n`] is plain data held in the app state, not a mutable global; the
//! one shared instance, [`I18n::en_us`], is a process-wide immutable fallback
//! bundle with no app state. [`t!`]
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
pub use types::{Arg, LocalNumber, Localized, MessageId, UserText};

/// Every message id, defined together with the [`ids::ALL`] list so a new
/// id cannot be left out of the consistency tests. This is the only place
/// `MessageId::new` is called (it is crate-private).
macro_rules! define_ids {
    ($($name:ident = $id:literal;)+) => {
        /// The message ids; each lives in `locales/en-US/`.
        pub mod ids {
            use crate::MessageId;

            $(pub const $name: MessageId = MessageId::new($id);)+

            /// Every id above.
            #[cfg(test)]
            pub(crate) const ALL: &[MessageId] = &[$($name),+];
        }
    };
}

define_ids! {
    LANGUAGE_SYSTEM_DEFAULT = "language-system-default";
    MENU_FILE = "menu-file";
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
mod consistency_tests;
#[cfg(test)]
mod tests;
