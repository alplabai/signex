//! One explicit form for "this `Result` is deliberately not actionable".
//!
//! The alternative shapes all have a defect:
//!
//! * `let _ = expr;` — indistinguishable from an oversight, and the thing
//!   `clippy::let_underscore_must_use` exists to flag. Suppressing it
//!   per site costs a five-line `#[expect(..., reason = "...")]` block.
//! * `drop(expr);` — silences every lint with no reason recorded, and
//!   `drop` has legitimate uses all over the tree (lock guards), so it
//!   cannot be gated. #577 recorded this as the blind spot with no
//!   clippy answer.
//!
//! [`IgnoreResult::ignore`] is one line, records the reason, and — the
//! part that matters — **is implemented only for [`Result`]**. An
//! `iced::Task` is `#[must_use]` but is not a `Result`, so
//! `self.handle_x().ignore("…")` does not compile. The escape hatch
//! `drop(task)` opens stays shut here by construction, which is why this
//! is a trait on `Result` rather than a free `fn ignore<T>(_: T)`.
//!
//! It is also greppable in a way an attribute is not: `.ignore("` finds
//! every deliberate discard in the crate, each with its reason attached.
//!
//! Not for a failure the user should hear about — that is a bug, and
//! #533 exists because two of them shipped. Use this only where the
//! failure genuinely has no consequence and no caller can act on it.

/// Consume a `Result` whose failure is deliberately not actionable.
pub(crate) trait IgnoreResult {
    /// Discard the result, recording why that is correct.
    ///
    /// `reason` is documentation the signature forces you to write. It
    /// is `&'static str` so it cannot carry runtime state, and it is
    /// never read at runtime.
    fn ignore(self, reason: &'static str);
}

impl<T, E> IgnoreResult for Result<T, E> {
    #[inline]
    fn ignore(self, _reason: &'static str) {}
}

#[cfg(test)]
mod tests {
    use super::IgnoreResult;

    /// Both variants are accepted — `ignore` records the caller's
    /// intent, which does not depend on which variant turned up.
    #[test]
    fn accepts_both_variants() {
        let ok: Result<u8, &str> = Ok(1);
        ok.ignore("test");
        let err: Result<u8, &str> = Err("boom");
        err.ignore("test");
    }

    /// Taken by value, so a non-`Copy` payload is moved in and dropped
    /// here — nothing leaks and no borrow is left behind.
    #[test]
    fn consumes_a_non_copy_payload() {
        let owned: Result<String, std::io::Error> = Ok("payload".to_string());
        owned.ignore("test");
    }
}
