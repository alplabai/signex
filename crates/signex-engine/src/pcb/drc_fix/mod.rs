//! DRC fixes, one per rule kind (#2940): clearance, track width, annular
//! ring, hole to hole, silkscreen to pad and solder mask sliver. A fix never
//! waives a finding and never edits a locked item. A finding it cannot fix,
//! or whose fix would make a new finding, is left as it is with the reason.
//! Pure: the board, its rules and its parts' 3D bodies go in, the commands
//! and what they do come out, and the caller runs the commands as one undo
//! step (the engine's lease check runs then). The checks are Run DRC's,
//! bodies included, so a fix judged clean here is clean to `check_drc`.
//! How each fix works: `docs/DRC_RULES.md`, "Fixing findings".

mod mask;
mod moves;
mod plan;
mod sizes;

use std::collections::{BTreeMap, BTreeSet};

use signex_drc::{BodyModels, DrcReport, DrcRules, Violation, ViolationKind, WaiverState};
use signex_types::object_id::IdSource;
use signex_types::pcb::PcbBoard;

use super::BoardCommand;
use plan::Plan;

/// How many times one fix is planned again on the board it left while its
/// finding is still there.
const MAX_ATTEMPTS: usize = 4;

/// A rule kind that has a fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FixRule {
    Clearance,
    TrackWidth,
    AnnularRing,
    HoleToHole,
    SilkToPad,
    MaskSliver,
}

impl FixRule {
    pub const ALL: [Self; 6] = [
        Self::Clearance,
        Self::TrackWidth,
        Self::AnnularRing,
        Self::HoleToHole,
        Self::SilkToPad,
        Self::MaskSliver,
    ];

    /// The kind of finding it fixes.
    pub const fn kind(self) -> ViolationKind {
        match self {
            Self::Clearance => ViolationKind::Clearance,
            Self::TrackWidth => ViolationKind::TrackWidth,
            Self::AnnularRing => ViolationKind::AnnularRing,
            Self::HoleToHole => ViolationKind::HoleToHole,
            Self::SilkToPad => ViolationKind::SilkToPad,
            Self::MaskSliver => ViolationKind::MaskSliver,
        }
    }

    /// The fix of findings of `kind`, if it has one.
    pub fn of(kind: ViolationKind) -> Option<Self> {
        Self::ALL.into_iter().find(|rule| rule.kind() == kind)
    }

    /// What `fix_drc rule=` takes: the kind's rule id without `builtin::`.
    pub fn token(self) -> &'static str {
        let id = self.kind().rule_id();
        id.strip_prefix("builtin::").unwrap_or(id)
    }

    /// The fix a token names, with or without `builtin::`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let token = text.strip_prefix("builtin::").unwrap_or(text);
        Self::ALL
            .into_iter()
            .find(|rule| rule.token().eq_ignore_ascii_case(token))
    }
}

/// Which findings a run fixes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixScope {
    /// The one finding with this waiver key.
    One(String),
    /// Every finding of one kind, or of every kind (`None`).
    Every(Option<FixRule>),
}

/// Why a finding was left as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeftReason {
    /// Its rule kind has no fix.
    NoFix,
    /// It is waived: a fix never touches an accepted finding.
    Waived,
    /// The fix would change these locked items (`track <uuid>`, ...).
    Locked(Vec<String>),
    /// Nothing the fix may change can clear it, and why.
    CannotChange(&'static str),
    /// Every way tried makes these new findings (their waiver keys).
    WouldMakeNew(Vec<String>),
    /// The fix was made and the finding is still there.
    NotCleared,
    /// The board refused the edit.
    Refused(String),
}

impl LeftReason {
    /// The stable name in an answer.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoFix => "no_fix",
            Self::Waived => "waived",
            Self::Locked(_) => "locked",
            Self::CannotChange(_) => "cannot_change",
            Self::WouldMakeNew(_) => "new_violation",
            Self::NotCleared => "not_cleared",
            Self::Refused(_) => "refused",
        }
    }

    /// One sentence for a person.
    pub fn message(&self) -> String {
        match self {
            Self::NoFix => "this rule has no fix; change the board by hand".into(),
            Self::Waived => "it is waived; unwaive it first to fix it".into(),
            Self::Locked(names) => format!("the fix would change {}, locked", names.join(", ")),
            Self::CannotChange(why) => (*why).to_string(),
            Self::WouldMakeNew(keys) => {
                format!("every fix tried makes a new finding ({})", keys.join(", "))
            }
            Self::NotCleared => "the fix did not clear it".into(),
            Self::Refused(why) => format!("the board refused the edit: {why}"),
        }
    }
}

/// A finding fixed.
#[derive(Debug, Clone, PartialEq)]
pub struct Fixed {
    pub key: String,
    pub violation: Violation,
    /// The items its fix changed: a uuid, or `<footprint>/<pad>` for a pad.
    pub changed: Vec<String>,
    /// The key of the earlier fix of this run that cleared it, when its own
    /// fix was not needed.
    pub cleared_by: Option<String>,
}

/// A finding left as it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Left {
    pub key: String,
    pub violation: Violation,
    pub reason: LeftReason,
}

/// What a run fixed, what it left, and the board it leaves.
#[derive(Debug, Clone)]
pub struct FixRun {
    /// Every fix's commands, in order: one undo step.
    pub commands: Vec<BoardCommand>,
    pub fixed: Vec<Fixed>,
    pub left: Vec<Left>,
    /// The board with every fix made, and its check.
    pub board: PcbBoard,
    pub report: DrcReport,
}

/// Why a run could not start.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FixScopeError {
    #[error("the board has no open DRC finding `{0}`; run check_drc for the current keys")]
    NoSuchFinding(String),
}

/// Fix the findings `scope` names on `board`, checked with `rules` and the
/// parts' 3D `models` (empty when no rule measures a body). Each fix is
/// tried on a copy and kept only when its finding is gone and no new one
/// appeared; nothing here changes the board. Pure and blocking: run it off
/// the UI thread.
pub fn fix_findings(
    board: &PcbBoard,
    rules: &DrcRules,
    models: &BodyModels,
    scope: &FixScope,
    new_ids: &mut dyn IdSource,
) -> Result<FixRun, FixScopeError> {
    let board = board.clone();
    let report = signex_drc::run_with_bodies(&board, rules, models);
    let targets = targets(&report, &board, scope)?;
    let mut session = Session {
        rules,
        models,
        board,
        report,
        commands: Vec::new(),
        cleared: BTreeMap::new(),
    };
    let (mut fixed, mut left) = (Vec::new(), Vec::new());
    for (key, violation, waived) in targets {
        let reason = if waived {
            LeftReason::Waived
        } else if let Some(rule) = FixRule::of(violation.kind) {
            match session.open(&key) {
                None => {
                    let cleared_by = session.cleared.get(&key).cloned();
                    fixed.push(Fixed {
                        key,
                        violation,
                        changed: Vec::new(),
                        cleared_by,
                    });
                    continue;
                }
                Some(now) => match session.fix(rule, &key, &now, new_ids) {
                    Ok(changed) => {
                        fixed.push(Fixed {
                            key,
                            violation,
                            changed,
                            cleared_by: None,
                        });
                        continue;
                    }
                    Err(reason) => reason,
                },
            }
        } else {
            LeftReason::NoFix
        };
        left.push(Left {
            key,
            violation,
            reason,
        });
    }
    Ok(FixRun {
        commands: session.commands,
        fixed,
        left,
        board: session.board,
        report: session.report,
    })
}

/// The findings `scope` names, each with its key and whether it is waived;
/// one per key, in report order. A rule switched off reports nothing.
fn targets(
    report: &DrcReport,
    board: &PcbBoard,
    scope: &FixScope,
) -> Result<Vec<(String, Violation, bool)>, FixScopeError> {
    let waived = |v: &Violation| {
        matches!(
            signex_drc::waiver_state(&board.options.drc_waivers, v, board),
            WaiverState::Waived(_)
        )
    };
    let shown = report
        .violations
        .iter()
        .filter(|v| report.severity_of(v).is_some());
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for v in shown {
        let key = signex_drc::waiver_key(v);
        let wanted = match scope {
            FixScope::One(one) => *one == key,
            FixScope::Every(rule) => rule.is_none_or(|r| r.kind() == v.kind),
        };
        if wanted && seen.insert(key.clone()) {
            out.push((key, v.clone(), waived(v)));
        }
    }
    match scope {
        FixScope::One(key) if out.is_empty() => Err(FixScopeError::NoSuchFinding(key.clone())),
        _ => Ok(out),
    }
}

/// The board as the fixes so far left it.
struct Session<'a> {
    rules: &'a DrcRules,
    models: &'a BodyModels,
    board: PcbBoard,
    report: DrcReport,
    commands: Vec<BoardCommand>,
    /// A finding a fix cleared besides its own, and that fix's key.
    cleared: BTreeMap<String, String>,
}

/// One fix tried on a copy of the board.
struct Trial {
    board: PcbBoard,
    report: DrcReport,
    commands: Vec<BoardCommand>,
    changed: Vec<String>,
}

impl Session<'_> {
    /// The open finding `key` on the board as it is now.
    fn open(&self, key: &str) -> Option<Violation> {
        open_findings(&self.report, &self.board)
            .find(|v| signex_drc::waiver_key(v) == key)
            .cloned()
    }

    /// Fix `violation` (its key `key`): try each way in turn and keep the
    /// first that clears it and makes nothing new.
    fn fix(
        &mut self,
        rule: FixRule,
        key: &str,
        violation: &Violation,
        new_ids: &mut dyn IdSource,
    ) -> Result<Vec<String>, LeftReason> {
        let plans = plan::plans(&self.board, rule, violation, new_ids)?;
        let mut last = LeftReason::NotCleared;
        for first in plans {
            match self.attempt(rule, key, first, new_ids) {
                Ok(trial) => {
                    let changed = trial.changed.clone();
                    self.accept(key, trial);
                    return Ok(changed);
                }
                Err(reason) => last = reason,
            }
        }
        Err(last)
    }

    /// Make `first` on a copy, and plan it again on the copy while the
    /// finding is still there.
    fn attempt(
        &self,
        rule: FixRule,
        key: &str,
        first: Plan,
        new_ids: &mut dyn IdSource,
    ) -> Result<Trial, LeftReason> {
        let baseline = identities(&self.report, &self.board);
        let subject = first.subject.clone();
        let mut trial = Trial {
            board: self.board.clone(),
            report: self.report.clone(),
            commands: Vec::new(),
            changed: Vec::new(),
        };
        let mut step = first;
        for _ in 0..MAX_ATTEMPTS {
            for command in &step.commands {
                command
                    .clone()
                    .apply(&mut trial.board)
                    .map_err(|e| LeftReason::Refused(e.to_string()))?;
            }
            trial.commands.extend(step.commands);
            for id in step.changed {
                if !trial.changed.contains(&id) {
                    trial.changed.push(id);
                }
            }
            trial.report = signex_drc::run_with_bodies(&trial.board, self.rules, self.models);
            let made = made_new(&baseline, &trial.report, &trial.board);
            if !made.is_empty() {
                return Err(LeftReason::WouldMakeNew(made));
            }
            let still = open_findings(&trial.report, &trial.board)
                .find(|v| signex_drc::waiver_key(v) == key)
                .cloned();
            let Some(still) = still else {
                return Ok(trial);
            };
            step = plan::plans(&trial.board, rule, &still, new_ids)?
                .into_iter()
                .find(|p| p.subject == subject)
                .ok_or(LeftReason::NotCleared)?;
        }
        Err(LeftReason::NotCleared)
    }

    /// Keep `trial`, the fix of `key`, and note what else it cleared.
    fn accept(&mut self, key: &str, trial: Trial) {
        let after: BTreeSet<String> = open_findings(&trial.report, &trial.board)
            .map(signex_drc::waiver_key)
            .collect();
        for gone in open_findings(&self.report, &self.board).map(signex_drc::waiver_key) {
            if gone != key && !after.contains(&gone) {
                self.cleared.entry(gone).or_insert_with(|| key.to_string());
            }
        }
        self.board = trial.board;
        self.report = trial.report;
        self.commands.extend(trial.commands);
    }
}

/// The findings of `report` a person sees on `board`: a rule switched on,
/// and not waived.
fn open_findings<'a>(
    report: &'a DrcReport,
    board: &'a PcbBoard,
) -> impl Iterator<Item = &'a Violation> + 'a {
    report.violations.iter().filter(move |v| {
        report.severity_of(v).is_some()
            && !matches!(
                signex_drc::waiver_state(&board.options.drc_waivers, v, board),
                WaiverState::Waived(_)
            )
    })
}

/// What tells one finding from another across an edit: its waiver key, or
/// for a finding that names no item (keyed by where it is) its rule alone.
fn identity(v: &Violation) -> String {
    if v.items.is_empty() && v.keepout.is_none() {
        format!("{}#unplaced", signex_drc::rule_id(v))
    } else {
        signex_drc::waiver_key(v)
    }
}

/// How many open findings of each identity `report` has.
fn identities(report: &DrcReport, board: &PcbBoard) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for v in open_findings(report, board) {
        *out.entry(identity(v)).or_insert(0) += 1;
    }
    out
}

/// The keys of the open findings of `report` that `baseline` lacks.
fn made_new(
    baseline: &BTreeMap<String, usize>,
    report: &DrcReport,
    board: &PcbBoard,
) -> Vec<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut made = Vec::new();
    for v in open_findings(report, board) {
        let id = identity(v);
        let seen = counts.entry(id.clone()).or_insert(0);
        *seen += 1;
        if *seen > baseline.get(&id).copied().unwrap_or(0) {
            made.push(signex_drc::waiver_key(v));
        }
    }
    made
}

#[cfg(test)]
mod move_tests;
#[cfg(test)]
mod tests;
