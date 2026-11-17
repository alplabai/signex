//! The Rules tab's search (#2398): a typed needle against rule names, scopes,
//! layers, messages and scope picks. Matching folds case on data the user
//! typed or the board holds; nothing here is shown.

/// The needle the search box holds, trimmed and folded.
pub(super) fn needle(typed: &str) -> String {
    typed.trim().to_lowercase()
}

/// Whether `hay` holds `needle` (from [`needle`]); every text holds an empty
/// one.
pub(super) fn holds(hay: &str, needle: &str) -> bool {
    needle.is_empty() || hay.to_lowercase().contains(needle)
}
