//! `erc_clean` and `drc_clean`: the project's ERC run and its board's DRC
//! run have no error finding (and no warning finding unless allowed). One
//! finding with the counts, not a copy of every ERC or DRC finding; the run's
//! own warnings go in the report's `warnings`.

use serde_json::json;
use signex_types::check::{CheckReport, Severity};

use crate::{Board, Miss};

/// `erc_clean`.
pub(crate) fn erc_clean(
    run: Option<&Result<CheckReport, String>>,
    allow_warnings: bool,
) -> Vec<Miss> {
    clean("ERC", run, allow_warnings, None)
}

/// `drc_clean`, filed under the board.
pub(crate) fn drc_clean(
    board: &Board,
    run: Option<&Result<CheckReport, String>>,
    allow_warnings: bool,
) -> Vec<Miss> {
    match board {
        Board::NotListed => vec![Miss::new(
            "The project lists no board, so there is no DRC to run",
        )],
        Board::Read { path, .. } | Board::Unreadable { path, .. } => {
            clean("DRC", run, allow_warnings, Some(path.as_str()))
        }
    }
}

fn clean(
    name: &str,
    run: Option<&Result<CheckReport, String>>,
    allow_warnings: bool,
    sheet: Option<&str>,
) -> Vec<Miss> {
    let file = |miss: Miss| match sheet {
        Some(sheet) => miss.on(sheet),
        None => miss,
    };
    let report = match run {
        None => return vec![file(Miss::new(format!("The {name} run was not prepared")))],
        Some(Err(why)) => {
            let miss = Miss::new(format!("The {name} run did not finish: {why}"));
            return vec![file(miss.with("reason", json!(why)))];
        }
        Some(Ok(report)) => report,
    };
    let count = |severity: Severity| {
        report
            .findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    };
    let (errors, warnings, info) = (
        count(Severity::Error),
        count(Severity::Warning),
        count(Severity::Info),
    );
    let failing = errors + if allow_warnings { 0 } else { warnings };
    if failing == 0 {
        return Vec::new();
    }
    let message = if allow_warnings {
        format!("{name} found {errors} error(s)")
    } else {
        format!("{name} found {errors} error(s) and {warnings} warning(s)")
    };
    let miss = Miss::new(message)
        .with("check", json!(report.check))
        .with("errors", json!(errors))
        .with("warnings", json!(warnings))
        .with("info", json!(info))
        .with("waived", json!(report.waived.len()))
        .with("run_warnings", json!(report.warnings.len()))
        .with("rule_set", report.rule_set.clone().unwrap_or_default());
    vec![file(miss)]
}
