//! The NDJSON events of a `library --progress` run (contract §3.1) and the
//! summary that must be its last line (§3.2).

use std::path::PathBuf;

use serde::Deserialize;

use super::stream::Line;

/// How a run ended, as its summary's `exit` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitKind {
    Ok,
    Partial,
    Failed,
}

/// The summary's per-kind counts. Missing keys read as 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct Counts {
    pub symbols: u64,
    pub footprints: u64,
    pub parts: u64,
    pub models: u64,
    pub skipped: u64,
}

/// The run's last stdout line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub ok: bool,
    pub error: Option<String>,
    pub exit: ExitKind,
    /// The library folder written (`None` on failure or a dry run).
    pub library: Option<PathBuf>,
    pub report: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    pub counts: Counts,
    /// Aggregated non-`info` report items: message and count.
    pub warnings: Vec<(String, usize)>,
}

/// A progress event other than the summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressEvent {
    /// A new phase (`read`, `convert`, `write`, `models`) with its item
    /// count, when known.
    Phase { phase: String, total: Option<u64> },
    /// `done` of `total` in the current phase; `item` is display only.
    Progress {
        done: u64,
        total: Option<u64>,
        item: Option<String>,
    },
    /// An event a newer contract minor added; it still proves liveness.
    Other(String),
}

/// One parsed stdout line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Progress(ProgressEvent),
    Summary(Summary),
}

#[derive(Deserialize)]
struct RawLine {
    event: String,
    #[serde(default)]
    phase: Option<String>,
    #[serde(default)]
    total: Option<u64>,
    #[serde(default)]
    done: Option<u64>,
    #[serde(default)]
    item: Option<String>,
}

#[derive(Deserialize)]
struct RawSummary {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    exit: Option<String>,
    #[serde(default)]
    library: Option<PathBuf>,
    #[serde(default)]
    report: Option<PathBuf>,
    #[serde(default)]
    files: Vec<PathBuf>,
    #[serde(default)]
    counts: Counts,
    #[serde(default)]
    warnings: Vec<RawWarning>,
}

#[derive(Deserialize)]
struct RawWarning {
    message: String,
    #[serde(default)]
    count: Option<u64>,
}

/// Parse one line. `Err` is why it is not an event.
pub fn parse_event(line: &str) -> Result<Event, String> {
    let raw: RawLine = serde_json::from_str(line).map_err(|e| e.to_string())?;
    match raw.event.as_str() {
        "phase" => Ok(Event::Progress(ProgressEvent::Phase {
            phase: raw.phase.ok_or("phase event without a phase")?,
            total: raw.total,
        })),
        "progress" => Ok(Event::Progress(ProgressEvent::Progress {
            done: raw.done.ok_or("progress event without done")?,
            total: raw.total,
            item: raw.item,
        })),
        "summary" => parse_summary(line).map(Event::Summary),
        other => Ok(Event::Progress(ProgressEvent::Other(other.to_string()))),
    }
}

/// Parse a summary object.
pub fn parse_summary(line: &str) -> Result<Summary, String> {
    let raw: RawSummary = serde_json::from_str(line).map_err(|e| e.to_string())?;
    let exit = match raw.exit.as_deref() {
        Some("ok") => ExitKind::Ok,
        Some("partial") => ExitKind::Partial,
        Some("failed") => ExitKind::Failed,
        None if raw.ok => ExitKind::Ok,
        None => ExitKind::Failed,
        Some(other) => return Err(format!("unknown exit {other:?}")),
    };
    Ok(Summary {
        ok: raw.ok,
        error: raw.error,
        exit,
        library: raw.library,
        report: raw.report,
        files: raw.files,
        counts: raw.counts,
        warnings: raw
            .warnings
            .into_iter()
            .map(|w| (w.message, w.count.unwrap_or(1) as usize))
            .collect(),
    })
}

/// What a stream has said so far: the summary counts only while it is the
/// last line.
#[derive(Debug, Default)]
pub struct StreamFold {
    last_summary: Option<Summary>,
    /// Lines that were not events (malformed, oversized), for the log.
    pub rejected: usize,
}

impl StreamFold {
    /// Take one line. Returns the progress event it carried, if any, and
    /// whether it was an event at all (what the silence rule counts).
    pub fn take(&mut self, line: Line) -> (bool, Option<ProgressEvent>) {
        let text = match line {
            Line::Text(text) if text.trim().is_empty() => return (false, None),
            Line::Text(text) => text,
            Line::Oversized(len) => {
                log::warn!("library import: dropped a {len}-byte converter line");
                self.rejected += 1;
                self.last_summary = None;
                return (false, None);
            }
        };
        match parse_event(text.trim()) {
            Ok(Event::Summary(summary)) => {
                self.last_summary = Some(summary);
                (true, None)
            }
            Ok(Event::Progress(event)) => {
                self.last_summary = None;
                (true, Some(event))
            }
            Err(why) => {
                log::warn!("library import: unreadable converter line: {why}");
                self.rejected += 1;
                self.last_summary = None;
                (false, None)
            }
        }
    }

    /// The summary, when the stream ended on one.
    pub fn into_summary(self) -> Option<Summary> {
        self.last_summary
    }
}

#[cfg(test)]
mod tests {
    use super::super::stream::LineSplitter;
    use super::*;

    const PROGRESS_OK: &str =
        include_str!("../../../../docs/fixtures/library-converter/progress-ok.ndjson");
    const SUMMARY_FAILED: &str =
        include_str!("../../../../docs/fixtures/library-converter/summary-failed.json");

    fn fold(bytes: &[u8], max: usize) -> (StreamFold, Vec<ProgressEvent>) {
        let mut splitter = LineSplitter::new(max);
        let mut lines = Vec::new();
        splitter.push(bytes, &mut lines);
        lines.extend(splitter.finish());
        let mut fold = StreamFold::default();
        let events = lines
            .into_iter()
            .filter_map(|line| fold.take(line).1)
            .collect();
        (fold, events)
    }

    #[test]
    fn the_progress_fixture_yields_events_and_its_summary() {
        let (fold, events) = fold(PROGRESS_OK.as_bytes(), 1 << 20);
        assert_eq!(events.len(), 6);
        assert_eq!(
            events[1],
            ProgressEvent::Progress {
                done: 1,
                total: Some(2),
                item: Some("Passives".into())
            }
        );
        assert_eq!(
            events[5],
            ProgressEvent::Phase {
                phase: "models".into(),
                total: None
            }
        );
        let summary = fold.into_summary().unwrap();
        assert_eq!(summary.exit, ExitKind::Partial);
        assert_eq!(summary.library, Some(PathBuf::from("/abs/out/Passives")));
        assert_eq!(summary.counts.parts, 3);
        assert_eq!(summary.counts.skipped, 1);
        assert_eq!(summary.warnings[0].1, 1);
    }

    #[test]
    fn the_failed_summary_fixture_parses() {
        let summary = parse_summary(SUMMARY_FAILED.trim()).unwrap();
        assert!(!summary.ok);
        assert_eq!(summary.exit, ExitKind::Failed);
        assert!(summary.error.unwrap().contains("not a compound document"));
        assert_eq!(summary.library, None);
    }

    #[test]
    fn a_partial_last_line_without_newline_still_counts() {
        let trimmed = PROGRESS_OK.trim_end_matches('\n');
        assert!(fold(trimmed.as_bytes(), 1 << 20).0.into_summary().is_some());
    }

    #[test]
    fn a_truncated_stream_has_no_summary() {
        let cut = &PROGRESS_OK[..PROGRESS_OK.len() - 40];
        let (fold, events) = fold(cut.as_bytes(), 1 << 20);
        assert_eq!(events.len(), 6, "the complete lines still count");
        assert_eq!(fold.rejected, 1);
        assert!(fold.into_summary().is_none());
    }

    #[test]
    fn a_summary_followed_by_more_lines_is_not_the_summary() {
        let text = format!(
            "{}{{\"event\":\"progress\",\"done\":1,\"total\":null}}\n",
            PROGRESS_OK
        );
        assert!(fold(text.as_bytes(), 1 << 20).0.into_summary().is_none());
    }

    #[test]
    fn malformed_and_oversized_lines_are_skipped() {
        let mut text = String::from("not json\n{\"event\":\"phase\"}\n");
        text.push_str(&format!("{{\"event\":\"progress\",\"item\":\"{}\"}}\n", "x".repeat(300)));
        text.push_str("{\"event\":\"future\",\"x\":1}\n");
        text.push_str(SUMMARY_FAILED);
        let (fold, events) = fold(text.as_bytes(), 256);
        assert_eq!(events, [ProgressEvent::Other("future".into())]);
        assert_eq!(fold.rejected, 3);
        assert!(fold.into_summary().is_some());
    }

    #[test]
    fn an_oversized_summary_is_missing() {
        let (fold, _) = fold(PROGRESS_OK.as_bytes(), 64);
        assert!(fold.into_summary().is_none());
    }

    #[test]
    fn an_unknown_exit_is_refused() {
        assert!(parse_summary(r#"{"ok":true,"exit":"maybe"}"#).is_err());
        assert_eq!(
            parse_summary(r#"{"ok":true}"#).unwrap().exit,
            ExitKind::Ok,
            "a summary without exit reads ok from ok"
        );
    }
}
