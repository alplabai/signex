//! What a volume case probe reported (#1447); `state/project_case.rs` runs the
//! probe and reads the answer.

/// What the off-thread volume check reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseProbeOutcome {
    /// The volume answered: `true` when it ignores case.
    Decided(bool),
    /// The probe ran but could not decide (empty, read-only or unreadable
    /// volume).
    Undecided,
    /// The probe could not stat the entry it needed (deleted meanwhile,
    /// permission denied); carries the reason.
    StatFailed(String),
    /// The worker itself failed; carries the reason.
    Failed(String),
}
