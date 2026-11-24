//! Message catalogs and locale-aware formatting for the Signex UI.
//!
//! The only crate that imports Fluent or ICU4X; a backend change touches
//! this crate alone. It is a UI-layer crate: the domain crates cannot depend
//! on it (`scripts/check_arch_guard.sh`). The ids and the values that carry
//! them live in the UI-free `signex-i18n-ids` and are re-exported here under
//! the same paths.
//!
//! An [`I18n`] is plain data held in the app state, not a mutable global; the
//! one shared instance, [`I18n::en_us`], is a process-wide immutable fallback
//! bundle with no app state. [`t!`]
//! renders a message to [`Localized`], which is for display only; state and
//! files keep a [`MessageId`] and its arguments.

mod bundle;
mod catalog;
mod format;
mod i18n;
mod locale;
mod localized;
mod numbers;
mod render;
mod text;

pub use format::{DateLength, ListKind};
pub use i18n::{I18n, LanguageOption};
/// The locale type [`I18n::format_locale`] returns, for callers that keep one.
pub use icu_locale_core::Locale;
pub use locale::{LanguagePref, resolved_locale};
pub use localized::Localized;
pub use render::RenderTranslatable;
pub use signex_i18n_ids::{
    Arg, LocalNumber, MessageId, MessageRenderer, Translatable, UserText, command_ids, ids,
    key_ids, menu_ids, shortcut_ids,
};

/// Render a message: `t!(i18n, ids::MENU_FILE)` or, with arguments,
/// `t!(i18n, ID, "count" => 3_i64, "name" => "R1")`.
///
/// A bare float is not an argument; wrap it in [`LocalNumber`]:
///
/// ```compile_fail,E0277
/// use signex_i18n::{I18n, ids, t};
/// let i18n = I18n::en_us();
/// let _ = t!(i18n, ids::MENU_FILE, "value" => 1.27_f64);
/// ```
///
/// ```compile_fail,E0599
/// let _ = signex_i18n::Arg::Number(1.27);
/// ```
///
/// The same setup compiles once the number is wrapped, so the two failures
/// above are not caused by a renamed id or constructor:
///
/// ```
/// use signex_i18n::{I18n, LocalNumber, ids, t};
/// let i18n = I18n::en_us();
/// let _ = t!(i18n, ids::MENU_FILE, "value" => LocalNumber(1.27));
/// ```
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
mod en_us_copy_tests;
#[cfg(test)]
mod format_tests;
#[cfg(test)]
mod tests;
