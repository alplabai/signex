//! What a checklist item links to and what that link says now: a check (the
//! project's ERC or DRC run, its title block, one of its requirements) or a
//! design object, which has no result of its own. The text form is the one a
//! command takes: `erc`, `drc`, `title_block`, `requirement:<id>`,
//! `object:<what>`.

use serde::{Deserialize, Serialize};

use crate::check::CheckReport;
use crate::project::ProjectData;

/// The project parameters a filled title block has, each not blank.
pub const TITLE_BLOCK_REQUIRED: [&str; 4] = ["title", "document_number", "rev", "date"];

/// The requirement id the checklist's own ERC check runs under.
const ERC_RUN_ID: &str = "checklist::erc";
/// The requirement id the checklist's own DRC check runs under.
const DRC_RUN_ID: &str = "checklist::drc";

/// What an item links to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChecklistLink {
    /// The project's ERC run: it passes with no error finding.
    Erc,
    /// The DRC run of the project's board: it passes with no error finding.
    Drc,
    /// The project parameters of [`TITLE_BLOCK_REQUIRED`] are all set.
    TitleBlock,
    /// One of the project's requirements, by id.
    Requirement { id: String },
    /// A part, pin, net, sheet or board object, as typed; no check.
    Object { target: String },
}

/// What a link says now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkResult {
    /// The check passes.
    Passes,
    /// The check fails with this many findings.
    Fails(usize),
    /// The check has not run, or the run did not check it.
    NotRun,
    /// The link is to a design object: there is nothing to run.
    NoCheck,
}

impl LinkResult {
    /// The result's name in a command's answer.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Passes => "passes",
            Self::Fails(_) => "fails",
            Self::NotRun => "not_run",
            Self::NoCheck => "no_check",
        }
    }
}

impl ChecklistLink {
    /// The link `text` names; `None` for an empty text, which clears it.
    pub fn parse(text: &str) -> Result<Option<Self>, String> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(None);
        }
        let link = match text.split_once(':') {
            None => match text {
                "erc" => Self::Erc,
                "drc" => Self::Drc,
                "title_block" => Self::TitleBlock,
                _ => return Err(text.to_string()),
            },
            Some((kind, rest)) => {
                let rest = rest.trim();
                match kind.trim() {
                    "requirement" if !rest.is_empty() => Self::Requirement { id: rest.into() },
                    "object" if !rest.is_empty() => Self::Object {
                        target: rest.into(),
                    },
                    _ => return Err(text.to_string()),
                }
            }
        };
        Ok(Some(link))
    }

    /// The text [`Self::parse`] reads back as this link.
    pub fn to_text(&self) -> String {
        match self {
            Self::Erc => "erc".into(),
            Self::Drc => "drc".into(),
            Self::TitleBlock => "title_block".into(),
            Self::Requirement { id } => format!("requirement:{id}"),
            Self::Object { target } => format!("object:{target}"),
        }
    }

    /// The requirement id a run checks this link under: the project's own
    /// for a requirement, the checklist's own for ERC and DRC; `None` for a
    /// link that runs nothing.
    pub fn run_id(&self) -> Option<&str> {
        match self {
            Self::Erc => Some(ERC_RUN_ID),
            Self::Drc => Some(DRC_RUN_ID),
            Self::Requirement { id } => Some(id),
            Self::TitleBlock | Self::Object { .. } => None,
        }
    }

    /// What the link says over `data` and the last run's `report`. The title
    /// block is read from `data` at once; a link with a run id reads the
    /// rule the requirement checks file it under (`req::<id>`, as
    /// `signex_req::entry_rule` names an entry with that id).
    pub fn result(&self, data: &ProjectData, report: Option<&CheckReport>) -> LinkResult {
        if *self == Self::TitleBlock {
            let missing = title_block_missing(data).len();
            return if missing == 0 {
                LinkResult::Passes
            } else {
                LinkResult::Fails(missing)
            };
        }
        let Some(id) = self.run_id() else {
            return LinkResult::NoCheck;
        };
        let Some(report) = report else {
            return LinkResult::NotRun;
        };
        let rule = format!("req::{id}");
        let failed = report.findings.iter().filter(|f| f.rule == rule).count();
        if failed > 0 {
            LinkResult::Fails(failed)
        } else if report.passed.contains(&rule) {
            LinkResult::Passes
        } else {
            LinkResult::NotRun
        }
    }
}

/// The report after a run of only the links whose run ids are `run_ids`
/// (one item's Run check): `ran`, plus what `previous` said of every other
/// link, so a check run alone does not forget the others' results.
pub fn merge_link_run(
    previous: Option<&CheckReport>,
    mut ran: CheckReport,
    run_ids: &[String],
) -> CheckReport {
    let Some(previous) = previous else {
        return ran;
    };
    let rules: Vec<String> = run_ids.iter().map(|id| format!("req::{id}")).collect();
    let kept = previous
        .findings
        .iter()
        .filter(|finding| !rules.contains(&finding.rule))
        .cloned();
    ran.findings.extend(kept);
    let passed = previous
        .passed
        .iter()
        .filter(|rule| !rules.contains(rule))
        .cloned();
    ran.passed.extend(passed);
    ran
}

/// The [`TITLE_BLOCK_REQUIRED`] parameters `data` leaves blank.
pub fn title_block_missing(data: &ProjectData) -> Vec<&'static str> {
    TITLE_BLOCK_REQUIRED
        .into_iter()
        .filter(|key| {
            data.parameters
                .get(*key)
                .is_none_or(|value| value.trim().is_empty())
        })
        .collect()
}
