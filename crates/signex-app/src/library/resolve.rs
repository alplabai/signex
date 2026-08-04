//! Reporting wrapper around [`signex_library::LibrarySet`] resolution.
//!
//! The resolvers return `Result<Option<T>, LibraryError>`: `Ok(None)`
//! is "the primitive is not there", `Err` is "the lookup itself
//! failed". UI state slots (`state.symbol`, `state.footprint`,
//! `state.sim`) can only hold an `Option`, so the two have to be
//! flattened somewhere — [`report_read_failure`] is the one place
//! that is allowed to do it, and it puts the error in front of the
//! user first.
//!
//! Without this the user is told a UUID is not in the mounted
//! libraries when the truth is that a `.snxsym` on disk is corrupt or
//! a remote library server answered 500 — a diagnosis that sends them
//! off to re-bind a perfectly good reference.

use signex_library::{LibraryError, PrimitiveRef};

/// What kind of primitive was being resolved. Used only for the log
/// record's wording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedKind {
    Symbol,
    Footprint,
    Sim,
}

impl ResolvedKind {
    fn label(self) -> &'static str {
        match self {
            ResolvedKind::Symbol => "symbol",
            ResolvedKind::Footprint => "footprint",
            ResolvedKind::Sim => "simulation model",
        }
    }
}

/// Flatten a resolution into the `Option` the UI holds, sending any
/// read failure to the Messages panel on the way past.
///
/// `context` names the gesture that triggered the lookup so the record
/// says which action came up empty.
pub(crate) fn report_read_failure<T>(
    resolved: Result<Option<T>, LibraryError>,
    kind: ResolvedKind,
    reference: &PrimitiveRef,
    context: &str,
) -> Option<T> {
    match resolved {
        Ok(found) => found,
        Err(e) => {
            // Field order is load-bearing: the Messages panel compacts
            // a record to 160 characters, so the error and the UUID
            // that identify the failure go first and the wider context
            // trails.
            tracing::error!(
                target: "signex::library",
                error = %e,
                uuid = %reference.uuid,
                kind = kind.label(),
                library_id = %reference.library_id,
                context = context,
                "library primitive could not be read"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{DiagnosticEntry, DiagnosticLevel};
    use uuid::Uuid;

    fn any_ref() -> PrimitiveRef {
        PrimitiveRef::new(Uuid::now_v7(), Uuid::now_v7())
    }

    /// Make sure the real in-app logger is installed, so
    /// `recent_entries()` shows what the Messages panel would show.
    /// Other tests in this binary install it too; whoever gets there
    /// first installs the same `SignexLogger` feeding the same sink,
    /// so a rejected second call changes nothing.
    fn messages_panel_capture() {
        use crate::ignore::IgnoreResult;
        crate::diagnostics::init_logging()
            .ignore("another test in this binary already installed the same logger");
    }

    /// Entries the Messages panel would be holding that mention
    /// `needle`. Tests key on a per-test UUID so they stay independent
    /// under libtest's default parallelism.
    fn entries_mentioning(needle: &str) -> Vec<DiagnosticEntry> {
        crate::diagnostics::recent_entries()
            .into_iter()
            .filter(|e| e.message.contains(needle))
            .collect()
    }

    #[test]
    fn a_resolved_primitive_passes_straight_through() {
        let got = report_read_failure(
            Ok(Some(7u32)),
            ResolvedKind::Symbol,
            &any_ref(),
            "preview tab",
        );
        assert_eq!(got, Some(7));
    }

    #[test]
    fn an_absent_primitive_stays_none_and_is_not_reported_as_a_failure() {
        messages_panel_capture();
        let reference = any_ref();
        let got = report_read_failure::<u32>(
            Ok(None),
            ResolvedKind::Footprint,
            &reference,
            "preview tab",
        );
        assert_eq!(got, None);
        assert!(
            entries_mentioning(&reference.uuid.to_string()).is_empty(),
            "an absent UUID is a normal outcome and must not raise an error"
        );
    }

    #[test]
    fn a_read_failure_reaches_the_messages_panel_before_collapsing_to_none() {
        messages_panel_capture();
        let reference = any_ref();
        let got = report_read_failure::<u32>(
            Err(LibraryError::Backend("library server returned 500".into())),
            ResolvedKind::Sim,
            &reference,
            "simulation tab",
        );
        assert_eq!(got, None, "the UI slot has nowhere to put an error");

        let reported = entries_mentioning(&reference.uuid.to_string());
        assert_eq!(
            reported.len(),
            1,
            "a swallowed read failure never reaches the user: {reported:?}"
        );
        assert_eq!(reported[0].level, DiagnosticLevel::Error);
        assert_eq!(reported[0].code, "SIGNEX-LIBRARY");
        assert!(
            reported[0].message.contains("library server returned 500"),
            "the record dropped the underlying error: {}",
            reported[0].message
        );
    }

    #[test]
    fn every_kind_has_its_own_wording() {
        assert_eq!(ResolvedKind::Symbol.label(), "symbol");
        assert_eq!(ResolvedKind::Footprint.label(), "footprint");
        assert_eq!(ResolvedKind::Sim.label(), "simulation model");
    }
}
