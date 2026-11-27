//! The Command Log's model (#2458): every catalog command this session ran,
//! from a menu, a key, the palette, a context menu, the CLI or the agent
//! socket, newest last. Plain data; the shell records a row after the command
//! ran and the panel reads it. Rules: `docs/COMMAND_LOG.md`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Local};
use pcb_geom::ItemRef;
use serde_json::{Map, Value};
use signex_types::schematic::SelectedItem;

/// The most rows the log keeps; the oldest goes first.
pub const MAX_ROWS: usize = 5000;

/// The client name the CLI gives on the agent socket (`docs/CLI.md`).
pub const CLI_CLIENT: &str = "signex-cli";

/// The argument a GUI row shows a finished operation's input under: its
/// offset, angle or values. A record like [`RECORDED_TARGETS`], not an operand.
pub const RECORDED_INPUT: &str = "recorded_input";

/// The argument a GUI row shows its targets under. It is a record, not an
/// operand: the CLI line and an operand run leave it out.
pub const RECORDED_TARGETS: &str = "recorded_targets";

/// What a command that acts on the selection acted on, by stable item id.
/// The row's `document` says where.
#[derive(Debug, Clone, PartialEq)]
pub enum RecordedTargets {
    Sheet(Vec<SelectedItem>),
    Board(Vec<ItemRef>),
}

impl RecordedTargets {
    /// Each target as text: its kind and id.
    pub fn names(&self) -> Vec<String> {
        match self {
            Self::Sheet(items) => items
                .iter()
                .map(|item| format!("{:?} {}", item.kind, item.uuid))
                .collect(),
            Self::Board(items) => items.iter().map(|item| format!("{item:?}")).collect(),
        }
    }

    /// The targets as the row's recorded argument.
    pub fn to_operand(&self) -> Value {
        Value::Array(self.names().into_iter().map(Value::String).collect())
    }
}

/// Who ran a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogSource {
    /// A menu, a key, the palette, a context menu or a tool. `window` names
    /// the document of the undocked window it came from; `None` is the main
    /// window.
    Gui { window: Option<String> },
    /// `signex-cli` working through this app.
    Cli { pid: Option<u32> },
    /// Any other client of the agent socket.
    Agent {
        client: Option<String>,
        pid: Option<u32>,
    },
}

impl LogSource {
    /// The source of an agent request from `client` (process `pid`): the CLI
    /// names itself `signex-cli`, every other client is an agent.
    pub fn of_agent(client: Option<String>, pid: Option<u32>) -> Self {
        match client {
            Some(name) if name == CLI_CLIENT => Self::Cli { pid },
            client => Self::Agent { client, pid },
        }
    }

    pub fn kind(&self) -> SourceKind {
        match self {
            Self::Gui { .. } => SourceKind::Gui,
            Self::Cli { .. } => SourceKind::Cli,
            Self::Agent { .. } => SourceKind::Agent,
        }
    }
}

/// The three source filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceKind {
    Gui,
    Cli,
    Agent,
}

impl SourceKind {
    pub const ALL: [SourceKind; 3] = [SourceKind::Gui, SourceKind::Cli, SourceKind::Agent];

    fn key(self) -> &'static str {
        match self {
            Self::Gui => "gui",
            Self::Cli => "cli",
            Self::Agent => "agent",
        }
    }
}

/// What a command came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogOutcome {
    /// It ran; the text the agent reply gave, `None` for a plain "Done".
    Ok(Option<String>),
    /// It ran with a warning, or its answer is still to come.
    Warn(String),
    /// It did not run, or did not succeed: the reason as returned.
    Failed(String),
}

impl LogOutcome {
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    /// The text after the result icon: `None` for a plain "Done".
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Ok(text) => text.as_deref(),
            Self::Warn(text) | Self::Failed(text) => Some(text),
        }
    }

    /// What the table's Result cell shows of a failed row: the error code,
    /// the part before the first `:`, or the whole reason when it has none.
    pub fn short_text(&self) -> Option<&str> {
        match self {
            Self::Failed(reason) => Some(failure_code(reason)),
            other => other.text(),
        }
    }

    fn key(&self) -> &'static str {
        match self {
            Self::Ok(_) => "ok",
            Self::Warn(_) => "warn",
            Self::Failed(_) => "failed",
        }
    }
}

/// The part of `reason` before its first `:`, trimmed; all of it when that
/// part is empty or there is no `:`.
pub fn failure_code(reason: &str) -> &str {
    match reason.split_once(':') {
        Some((code, _)) if !code.trim().is_empty() => code.trim(),
        _ => reason,
    }
}

/// One command that ran.
#[derive(Debug, Clone, PartialEq)]
pub struct LogRow {
    /// Its place in the session, from 1; never reused, not even after Clear.
    pub seq: u64,
    pub at: DateTime<Local>,
    pub source: LogSource,
    /// The catalog id (`place_wire`).
    pub command: String,
    /// The catalog label in the UI language when it ran; the id when the
    /// catalog has no such command.
    pub label: String,
    /// The operands it ran with, name to value as the CLI or the socket gave
    /// them; empty for a GUI command.
    pub operands: Map<String, Value>,
    pub outcome: LogOutcome,
    /// How long the synchronous part of the dispatch took.
    pub took: Duration,
    /// The document it acted on, when it names or has one.
    pub document: Option<PathBuf>,
    /// The project file of that document, for the CLI line.
    pub project: Option<PathBuf>,
    /// The catalog says its change is an undo step.
    pub undoable: bool,
    /// A view or selection command that changes no document.
    pub view_only: bool,
    /// When an undo of its document took it back.
    pub undone_at: Option<DateTime<Local>>,
}

impl LogRow {
    /// Whether an undo can take this row back: it changed something.
    fn can_be_undone(&self) -> bool {
        self.undoable
            && !self.outcome.is_failed()
            && !matches!(self.command.as_str(), "undo" | "redo")
    }

    /// Each operand as `name` and the text the CLI takes for it. A GUI row's
    /// recorded targets are not an operand.
    pub fn argument_pairs(&self) -> Vec<(&str, String)> {
        let is_gui = matches!(self.source, LogSource::Gui { .. });
        self.operands
            .iter()
            .filter(|(name, _)| {
                !(is_gui && matches!(name.as_str(), RECORDED_TARGETS | RECORDED_INPUT))
            })
            .map(|(name, value)| (name.as_str(), operand_text(value)))
            .collect()
    }

    /// Whether it ran with operands of its own, to run through their handler.
    pub fn has_given_operands(&self) -> bool {
        !self.argument_pairs().is_empty()
    }

    /// The Arguments cell: `name=value` pairs two spaces apart; empty when
    /// there are none.
    pub fn arguments_text(&self) -> String {
        self.argument_pairs()
            .into_iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("  ")
    }

    /// The operands as indented JSON; `None` when there are none.
    pub fn arguments_json(&self) -> Option<String> {
        if self.operands.is_empty() {
            return None;
        }
        serde_json::to_string_pretty(&Value::Object(self.operands.clone())).ok()
    }

    /// The `name=value` texts `CommandArgs::parse_operands` reads, to run the
    /// command again.
    pub fn operand_texts(&self) -> Vec<String> {
        self.argument_pairs()
            .into_iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect()
    }

    /// The command as one `signex-cli run` line (`docs/CLI.md`, Usage):
    /// `signex-cli run <project> <id> --arg name=value ...`. `project` is the
    /// text put for the project file when the row has none.
    pub fn cli_line(&self, project: &str) -> String {
        let project = self
            .project
            .as_ref()
            .map_or_else(|| project.to_string(), |p| p.display().to_string());
        let mut line = format!(
            "signex-cli run {} {}",
            shell_word(&project),
            shell_word(&self.command)
        );
        for (name, value) in self.argument_pairs() {
            line.push_str(" --arg ");
            line.push_str(&shell_word(&format!("{name}={value}")));
        }
        line
    }

    /// The object Jump to goes to: a designator, a target, a net, or the
    /// point `at`; `None` when the operands name none.
    pub fn jump_target(&self) -> Option<JumpTarget> {
        for name in ["designator", "target"] {
            if let Some(Value::String(text)) = self.operands.get(name)
                && !text.trim().is_empty()
            {
                return Some(JumpTarget::Designator(text.trim().to_string()));
            }
        }
        if let Some(Value::String(net)) = self.operands.get("net")
            && !net.trim().is_empty()
        {
            return Some(JumpTarget::Net(net.trim().to_string()));
        }
        self.operands
            .get("at")
            .and_then(point_of)
            .map(JumpTarget::Point)
    }

    /// The text of the source filter's query: label, id and arguments.
    fn haystack(&self) -> String {
        // A search key, never shown, so no language case rules apply.
        // ast-grep-ignore: no-case-in-view
        format!("{} {} {}", self.label, self.command, self.arguments_text()).to_lowercase()
    }

    /// One JSON Lines record of the saved log.
    fn to_json(&self) -> Value {
        let mut object = Map::new();
        object.insert("seq".into(), self.seq.into());
        object.insert("time".into(), Value::String(time_text(&self.at)));
        object.insert("source".into(), self.source.kind().key().into());
        match &self.source {
            LogSource::Gui { window } => {
                if let Some(window) = window {
                    object.insert("window".into(), window.clone().into());
                }
            }
            LogSource::Cli { pid } => {
                object.insert("client".into(), CLI_CLIENT.into());
                if let Some(pid) = pid {
                    object.insert("pid".into(), (*pid).into());
                }
            }
            LogSource::Agent { client, pid } => {
                if let Some(client) = client {
                    object.insert("client".into(), client.clone().into());
                }
                if let Some(pid) = pid {
                    object.insert("pid".into(), (*pid).into());
                }
            }
        }
        object.insert("id".into(), self.command.clone().into());
        object.insert("args".into(), Value::Object(self.operands.clone()));
        object.insert("result".into(), self.outcome.key().into());
        if let Some(text) = self.outcome.text() {
            object.insert("text".into(), text.into());
        }
        let took_ms = u64::try_from(self.took.as_millis()).unwrap_or(u64::MAX);
        object.insert("took_ms".into(), took_ms.into());
        if let Some(document) = &self.document {
            object.insert("document".into(), document.display().to_string().into());
        }
        object.insert("undoable".into(), self.undoable.into());
        if let Some(at) = &self.undone_at {
            object.insert("undone_at".into(), Value::String(time_text(at)));
        }
        Value::Object(object)
    }
}

/// What Jump to goes to.
#[derive(Debug, Clone, PartialEq)]
pub enum JumpTarget {
    /// A part by its designator.
    Designator(String),
    /// A net by its name.
    Net(String),
    /// A point of the sheet, in mm.
    Point((f64, f64)),
}

/// `[x, y]` of two numbers, or the `x,y` text a CLI operand gives.
fn point_of(value: &Value) -> Option<(f64, f64)> {
    match value {
        Value::Array(items) => match items.as_slice() {
            [x, y] => Some((x.as_f64()?, y.as_f64()?)),
            _ => None,
        },
        Value::String(text) => {
            let (x, y) = text.split_once(',')?;
            Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
        }
        _ => None,
    }
}

/// An operand value as the text the CLI's `--arg` takes: a string as it is,
/// a number or flag as written, a point as `x,y`, anything else as JSON. The
/// same forms the socket turns its JSON into (`agent_edits::wire::operand_text`).
pub fn operand_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => match items.as_slice() {
            [Value::Number(x), Value::Number(y)] => format!("{x},{y}"),
            _ => value.to_string(),
        },
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// `text` as one shell word: as it is when it holds only safe characters,
/// else in single quotes.
fn shell_word(text: &str) -> String {
    let safe = |c: char| c.is_ascii_alphanumeric() || "_-./:,=@+%".contains(c);
    if !text.is_empty() && text.chars().all(safe) {
        return text.to_string();
    }
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// A time with milliseconds and the UTC offset, for the saved log.
fn time_text(at: &DateTime<Local>) -> String {
    at.to_rfc3339_opts(chrono::SecondsFormat::Millis, false)
}

/// Which results the list shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ResultFilter {
    #[default]
    All,
    Failed,
    Undone,
}

/// The panel's filters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFilters {
    /// The sources shown; all three at first.
    pub sources: BTreeSet<SourceKind>,
    pub result: ResultFilter,
    /// Matched case-insensitively against label, id and arguments.
    pub query: String,
}

impl Default for LogFilters {
    fn default() -> Self {
        Self {
            sources: SourceKind::ALL.into_iter().collect(),
            result: ResultFilter::All,
            query: String::new(),
        }
    }
}

/// The counts the filters show beside their labels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LogCounts {
    pub gui: usize,
    pub cli: usize,
    pub agent: usize,
    pub failed: usize,
    pub undone: usize,
    /// Every row the view-command switch leaves in.
    pub total: usize,
}

impl LogCounts {
    pub fn of_source(&self, kind: SourceKind) -> usize {
        match kind {
            SourceKind::Gui => self.gui,
            SourceKind::Cli => self.cli,
            SourceKind::Agent => self.agent,
        }
    }
}

/// The session's log.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommandLog {
    rows: VecDeque<Arc<LogRow>>,
    next_seq: u64,
    /// While paused nothing is recorded.
    pub paused: bool,
    pub filters: LogFilters,
    /// View and selection commands are listed too.
    pub show_view_commands: bool,
    /// Agent requests whose reply waits for reads: `(pid, id, seq)`.
    awaiting: VecDeque<(Option<u32>, String, u64)>,
    /// What each row that acted on a selection acted on, by its number.
    targets: BTreeMap<u64, Arc<RecordedTargets>>,
}

impl CommandLog {
    /// Every row, oldest first.
    pub fn rows(&self) -> impl DoubleEndedIterator<Item = &LogRow> + ExactSizeIterator {
        self.rows.iter().map(Arc::as_ref)
    }

    pub fn row(&self, seq: u64) -> Option<&LogRow> {
        self.rows.iter().map(Arc::as_ref).find(|row| row.seq == seq)
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Add `row` with the next sequence number, and apply what an `undo` or
    /// `redo` it records does to the rows before it. Returns its number, or
    /// `None` while paused.
    pub fn record(&mut self, mut row: LogRow) -> Option<u64> {
        if self.paused {
            return None;
        }
        self.next_seq += 1;
        row.seq = self.next_seq;
        row.undone_at = None;
        if !row.outcome.is_failed() {
            match row.command.as_str() {
                "undo" => self.mark_undone(row.document.as_ref(), row.at),
                "redo" => self.mark_redone(row.document.as_ref()),
                _ => {}
            }
        }
        self.rows.push_back(Arc::new(row));
        while self.rows.len() > MAX_ROWS {
            if let Some(gone) = self.rows.pop_front() {
                self.targets.remove(&gone.seq);
            }
        }
        Some(self.next_seq)
    }

    /// Keep what row `seq` acted on, for Replay.
    pub fn keep_targets(&mut self, seq: u64, targets: RecordedTargets) {
        if self.row(seq).is_some() {
            self.targets.insert(seq, Arc::new(targets));
        }
    }

    /// What row `seq` acted on, when it recorded targets.
    pub fn targets_of(&self, seq: u64) -> Option<&RecordedTargets> {
        self.targets.get(&seq).map(Arc::as_ref)
    }

    /// The newest row of `document` an undo can take back, not undone yet.
    fn mark_undone(&mut self, document: Option<&PathBuf>, at: DateTime<Local>) {
        let Some(document) = document else {
            return;
        };
        if let Some(row) = self.rows.iter_mut().rev().find(|row| {
            row.can_be_undone()
                && row.undone_at.is_none()
                && row.document.as_ref() == Some(document)
        }) {
            Arc::make_mut(row).undone_at = Some(at);
        }
    }

    /// The row of `document` undone last is done again.
    fn mark_redone(&mut self, document: Option<&PathBuf>) {
        let Some(document) = document else {
            return;
        };
        let latest = self
            .rows
            .iter_mut()
            .filter(|row| row.undone_at.is_some() && row.document.as_ref() == Some(document))
            .max_by_key(|row| (row.undone_at, row.seq));
        if let Some(row) = latest {
            Arc::make_mut(row).undone_at = None;
        }
    }

    /// Show `value` under the recorded argument `name` of row `seq`.
    pub fn set_recorded(&mut self, seq: u64, name: &str, value: Value) {
        if let Some(row) = self.rows.iter_mut().find(|row| row.seq == seq) {
            Arc::make_mut(row).operands.insert(name.to_string(), value);
        }
    }

    /// Row `seq` acted on `document`.
    pub fn set_document(&mut self, seq: u64, document: Option<PathBuf>) {
        if let Some(row) = self.rows.iter_mut().find(|row| row.seq == seq) {
            Arc::make_mut(row).document = document;
        }
    }

    /// Change the outcome of row `seq` (an agent request answered late).
    pub fn settle(&mut self, seq: u64, outcome: LogOutcome) {
        if let Some(row) = self.rows.iter_mut().find(|row| row.seq == seq) {
            Arc::make_mut(row).outcome = outcome;
        }
    }

    /// Row `seq` is an agent request of `pid` for `id` whose reply comes later.
    pub fn await_reply(&mut self, pid: Option<u32>, id: &str, seq: u64) {
        self.awaiting.push_back((pid, id.to_string(), seq));
        while self.awaiting.len() > MAX_ROWS {
            self.awaiting.pop_front();
        }
    }

    /// The oldest row waiting for the reply of `pid` to `id`, which is now
    /// sent.
    pub fn take_awaiting(&mut self, pid: Option<u32>, id: &str) -> Option<u64> {
        let at = self
            .awaiting
            .iter()
            .position(|(p, i, _)| *p == pid && i == id)?;
        self.awaiting.remove(at).map(|(_, _, seq)| seq)
    }

    /// Empty the log. Numbering goes on.
    pub fn clear(&mut self) {
        self.rows.clear();
        self.awaiting.clear();
        self.targets.clear();
    }

    /// Whether the view-command switch leaves `row` in.
    fn listed(&self, row: &LogRow) -> bool {
        self.show_view_commands || !row.view_only
    }

    /// The rows the filters leave, oldest first.
    pub fn visible(&self) -> Vec<&LogRow> {
        // A search key, never shown, so no language case rules apply.
        // ast-grep-ignore: no-case-in-view
        let query = self.filters.query.trim().to_lowercase();
        self.rows()
            .filter(|row| self.listed(row))
            .filter(|row| self.filters.sources.contains(&row.source.kind()))
            .filter(|row| match self.filters.result {
                ResultFilter::All => true,
                ResultFilter::Failed => row.outcome.is_failed(),
                ResultFilter::Undone => row.undone_at.is_some(),
            })
            .filter(|row| query.is_empty() || row.haystack().contains(&query))
            .collect()
    }

    /// The counts of the rows the view-command switch leaves in.
    pub fn counts(&self) -> LogCounts {
        let mut counts = LogCounts::default();
        for row in self.rows().filter(|row| self.listed(row)) {
            counts.total += 1;
            match row.source.kind() {
                SourceKind::Gui => counts.gui += 1,
                SourceKind::Cli => counts.cli += 1,
                SourceKind::Agent => counts.agent += 1,
            }
            counts.failed += usize::from(row.outcome.is_failed());
            counts.undone += usize::from(row.undone_at.is_some());
        }
        counts
    }

    /// Flip the source filter `kind`.
    pub fn toggle_source(&mut self, kind: SourceKind) {
        if !self.filters.sources.remove(&kind) {
            self.filters.sources.insert(kind);
        }
    }

    /// Every row as JSON Lines, oldest first, one object per line.
    pub fn to_jsonl(&self) -> String {
        let mut out = String::new();
        for row in self.rows() {
            out.push_str(&row.to_json().to_string());
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn at(second: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 11, 19, 14, 2, second)
            .single()
            .expect("a local time")
    }

    fn row(command: &str, source: LogSource) -> LogRow {
        LogRow {
            seq: 0,
            at: at(1),
            source,
            command: command.into(),
            label: command.replace('_', " "),
            operands: Map::new(),
            outcome: LogOutcome::Ok(None),
            took: Duration::from_millis(3),
            document: Some(PathBuf::from("/p/a.snxsch")),
            project: Some(PathBuf::from("/p/P.snxprj")),
            undoable: false,
            view_only: false,
            undone_at: None,
        }
    }

    fn gui(command: &str) -> LogRow {
        row(command, LogSource::Gui { window: None })
    }

    fn undoable(command: &str) -> LogRow {
        LogRow {
            undoable: true,
            ..gui(command)
        }
    }

    fn with_args(mut r: LogRow, args: Value) -> LogRow {
        r.operands = args.as_object().cloned().unwrap_or_default();
        r
    }

    fn ids(log: &CommandLog) -> Vec<&str> {
        log.visible().iter().map(|r| r.command.as_str()).collect()
    }

    #[test]
    fn rows_are_numbered_and_a_paused_log_records_nothing() {
        let mut log = CommandLog::default();
        assert_eq!(log.record(gui("place_wire")), Some(1));
        log.paused = true;
        assert_eq!(log.record(gui("zoom_in")), None);
        log.paused = false;
        assert_eq!(log.record(gui("rotate")), Some(2));
        assert_eq!(log.rows().len(), 2);
    }

    #[test]
    fn clear_empties_the_log_and_numbering_goes_on() {
        let mut log = CommandLog::default();
        log.record(gui("a"));
        log.clear();
        assert!(log.is_empty());
        assert_eq!(log.record(gui("b")), Some(2));
    }

    #[test]
    fn the_oldest_row_goes_past_the_cap() {
        let mut log = CommandLog::default();
        for _ in 0..MAX_ROWS + 3 {
            log.record(gui("a"));
        }
        assert_eq!(log.rows().len(), MAX_ROWS);
        assert_eq!(log.rows().next().map(|r| r.seq), Some(4));
        assert_eq!(log.rows().last().map(|r| r.seq), Some(MAX_ROWS as u64 + 3));
    }

    #[test]
    fn a_cli_client_is_the_cli_and_any_other_an_agent() {
        assert_eq!(
            LogSource::of_agent(Some("signex-cli".into()), Some(7)),
            LogSource::Cli { pid: Some(7) }
        );
        assert_eq!(
            LogSource::of_agent(Some("bringup-script".into()), None).kind(),
            SourceKind::Agent
        );
        assert_eq!(
            LogSource::of_agent(None, Some(9)),
            LogSource::Agent {
                client: None,
                pid: Some(9)
            }
        );
    }

    #[test]
    fn source_result_and_query_filters_combine() {
        let mut log = CommandLog::default();
        log.record(gui("place_wire"));
        log.record(with_args(
            row("set_param", LogSource::of_agent(Some("bot".into()), None)),
            json!({"target": "R5", "value": "31.6k"}),
        ));
        log.record(LogRow {
            outcome: LogOutcome::Failed("E_LOCKED: U3 is locked".into()),
            ..row(
                "move_component",
                LogSource::of_agent(Some(CLI_CLIENT.into()), None),
            )
        });
        assert_eq!(ids(&log), ["place_wire", "set_param", "move_component"]);

        log.toggle_source(SourceKind::Gui);
        assert_eq!(ids(&log), ["set_param", "move_component"]);
        log.toggle_source(SourceKind::Gui);

        log.filters.result = ResultFilter::Failed;
        assert_eq!(ids(&log), ["move_component"]);
        log.filters.result = ResultFilter::All;

        log.filters.query = "  31.6K ".into();
        assert_eq!(ids(&log), ["set_param"], "an argument value matches");
        log.filters.query = "place wire".into();
        assert_eq!(ids(&log), ["place_wire"], "the label matches");
        log.filters.query = "move_comp".into();
        assert_eq!(ids(&log), ["move_component"], "the id matches");
    }

    #[test]
    fn view_commands_are_hidden_until_switched_on_and_left_out_of_the_counts() {
        let mut log = CommandLog::default();
        log.record(LogRow {
            view_only: true,
            ..gui("zoom_in")
        });
        log.record(gui("place_wire"));
        assert_eq!(ids(&log), ["place_wire"]);
        assert_eq!(log.counts().total, 1);
        log.show_view_commands = true;
        assert_eq!(ids(&log), ["zoom_in", "place_wire"]);
        assert_eq!(log.counts().gui, 2);
    }

    #[test]
    fn counts_are_per_source_failed_and_undone() {
        let mut log = CommandLog::default();
        log.record(undoable("place_wire"));
        log.record(row("list_documents", LogSource::of_agent(None, None)));
        log.record(row(
            "check_erc",
            LogSource::of_agent(Some(CLI_CLIENT.into()), None),
        ));
        log.record(LogRow {
            outcome: LogOutcome::Failed("No schematic is open".into()),
            ..gui("place_bus")
        });
        log.record(gui("undo"));
        let counts = log.counts();
        assert_eq!((counts.gui, counts.cli, counts.agent), (3, 1, 1));
        assert_eq!((counts.failed, counts.undone, counts.total), (1, 1, 5));
    }

    #[test]
    fn an_undo_marks_the_newest_undoable_row_of_its_document_and_redo_clears_it() {
        let mut log = CommandLog::default();
        let first = log.record(undoable("place_wire")).unwrap();
        let second = log.record(undoable("rotate")).unwrap();
        let other = log
            .record(LogRow {
                document: Some(PathBuf::from("/p/b.snxsch")),
                ..undoable("move")
            })
            .unwrap();
        log.record(LogRow {
            outcome: LogOutcome::Failed("locked".into()),
            ..undoable("delete")
        });
        log.record(gui("zoom_in"));

        log.record(LogRow {
            at: at(9),
            ..gui("undo")
        });
        let undone = |log: &CommandLog, seq| log.row(seq).unwrap().undone_at;
        assert_eq!(undone(&log, second), Some(at(9)), "the newest of a.snxsch");
        assert_eq!(undone(&log, other), None, "another document");
        log.record(LogRow {
            at: at(10),
            ..gui("undo")
        });
        assert_eq!(undone(&log, first), Some(at(10)));
        log.filters.result = ResultFilter::Undone;
        assert_eq!(ids(&log), ["place_wire", "rotate"]);

        log.record(gui("redo"));
        assert_eq!(undone(&log, first), None, "redo takes the last undone back");
        assert_eq!(undone(&log, second), Some(at(9)));
    }

    #[test]
    fn a_failed_undo_marks_nothing() {
        let mut log = CommandLog::default();
        let placed = log.record(undoable("place_wire")).unwrap();
        log.record(LogRow {
            outcome: LogOutcome::Failed("Nothing to undo".into()),
            ..gui("undo")
        });
        assert_eq!(log.row(placed).unwrap().undone_at, None);
    }

    #[test]
    fn a_failed_row_shows_its_code_and_keeps_the_whole_reason() {
        let failed = LogOutcome::Failed("E_LOCKED: U3 is locked (Lock Component)".into());
        assert_eq!(failed.short_text(), Some("E_LOCKED"));
        assert_eq!(
            failed.text(),
            Some("E_LOCKED: U3 is locked (Lock Component)")
        );
        let plain = LogOutcome::Failed("No schematic is open".into());
        assert_eq!(plain.short_text(), Some("No schematic is open"));
        assert_eq!(
            failure_code(": starts with a colon"),
            ": starts with a colon"
        );
        assert_eq!(LogOutcome::Ok(None).short_text(), None);
    }

    #[test]
    fn arguments_read_as_cli_operands_and_pretty_json() {
        let r = with_args(
            gui("place_junction"),
            json!({"at": [5.08, 0], "document": "main.snxsch", "dry_run": true}),
        );
        assert_eq!(
            r.arguments_text(),
            "at=5.08,0  document=main.snxsch  dry_run=true"
        );
        assert_eq!(
            r.operand_texts(),
            ["at=5.08,0", "document=main.snxsch", "dry_run=true"]
        );
        assert!(r.arguments_json().unwrap().contains("\n  \"at\": ["));
        assert_eq!(gui("zoom_in").arguments_json(), None);
        assert_eq!(gui("zoom_in").arguments_text(), "");
    }

    #[test]
    fn the_cli_line_follows_the_cli_usage_and_quotes_what_the_shell_would_split() {
        let r = with_args(
            gui("set_param"),
            json!({"target": "R5", "value": "Lock it's"}),
        );
        assert_eq!(
            r.cli_line("<project.snxprj>"),
            "signex-cli run /p/P.snxprj set_param --arg target=R5 --arg 'value=Lock it'\\''s'"
        );
        let loose = LogRow {
            project: None,
            ..gui("undo")
        };
        assert_eq!(
            loose.cli_line("<project.snxprj>"),
            "signex-cli run '<project.snxprj>' undo"
        );
    }

    /// Caner (2026-11-22): the targets show in the row's arguments and its
    /// saved line, and never reach the CLI line or an operand run.
    #[test]
    fn recorded_targets_show_as_arguments_but_are_not_operands() {
        use signex_types::schematic::SelectedKind;
        let id = uuid::Uuid::from_u128(7);
        let targets = RecordedTargets::Sheet(vec![SelectedItem::new(id, SelectedKind::Symbol)]);
        let mut r = gui("delete_selection");
        r.operands
            .insert(RECORDED_TARGETS.to_string(), targets.to_operand());
        assert!(r.arguments_json().unwrap().contains(&id.to_string()));
        assert!(!r.has_given_operands());
        assert!(r.operand_texts().is_empty());
        assert!(!r.cli_line("<p>").contains(RECORDED_TARGETS));

        let agent = with_args(
            row("x", LogSource::of_agent(None, None)),
            json!({ RECORDED_TARGETS: "given" }),
        );
        assert!(agent.has_given_operands(), "an agent's operand is its own");
    }

    #[test]
    fn targets_go_with_their_row() {
        let mut log = CommandLog::default();
        let seq = log.record(gui("delete_selection")).unwrap();
        log.keep_targets(seq, RecordedTargets::Sheet(Vec::new()));
        assert!(log.targets_of(seq).is_some());
        log.keep_targets(seq + 1, RecordedTargets::Sheet(Vec::new()));
        assert!(log.targets_of(seq + 1).is_none(), "no such row");
        log.clear();
        assert!(log.targets_of(seq).is_none());

        let oldest = log.record(gui("delete_selection")).unwrap();
        log.keep_targets(oldest, RecordedTargets::Sheet(Vec::new()));
        for _ in 0..MAX_ROWS {
            log.record(gui("a"));
        }
        assert!(log.targets_of(oldest).is_none(), "its row was dropped");
    }

    #[test]
    fn jump_goes_to_a_designator_a_net_or_a_point() {
        let jump = |args: Value| with_args(gui("x"), args).jump_target();
        assert_eq!(
            jump(json!({"designator": "U3", "net": "VIN"})),
            Some(JumpTarget::Designator("U3".into()))
        );
        assert_eq!(
            jump(json!({"net": "VIN"})),
            Some(JumpTarget::Net("VIN".into()))
        );
        assert_eq!(
            jump(json!({"at": [1.5, 2]})),
            Some(JumpTarget::Point((1.5, 2.0)))
        );
        assert_eq!(
            jump(json!({"at": "1.5, 2"})),
            Some(JumpTarget::Point((1.5, 2.0)))
        );
        assert_eq!(jump(json!({"path": "/x"})), None);
    }

    #[test]
    fn a_late_reply_settles_the_waiting_row() {
        let mut log = CommandLog::default();
        let seq = log
            .record(LogRow {
                outcome: LogOutcome::Warn("waiting".into()),
                ..row("place_component", LogSource::of_agent(None, Some(4)))
            })
            .unwrap();
        log.await_reply(Some(4), "place_component", seq);
        assert_eq!(log.take_awaiting(Some(5), "place_component"), None);
        assert_eq!(log.take_awaiting(Some(4), "place_component"), Some(seq));
        log.settle(seq, LogOutcome::Ok(None));
        assert_eq!(log.row(seq).unwrap().outcome, LogOutcome::Ok(None));
        assert_eq!(log.take_awaiting(Some(4), "place_component"), None);
    }

    #[test]
    fn the_saved_log_is_one_json_object_per_row() {
        let mut log = CommandLog::default();
        log.record(with_args(undoable("place_wire"), json!({"at": [1, 2]})));
        log.record(LogRow {
            outcome: LogOutcome::Failed("E_LOCKED: U3".into()),
            ..row("move", LogSource::of_agent(Some("bot".into()), Some(42)))
        });
        let text = log.to_jsonl();
        let lines: Vec<Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.len(), 2);
        assert!(text.ends_with('\n'));
        assert_eq!(lines[0]["seq"], 1);
        assert_eq!(lines[0]["source"], "gui");
        assert_eq!(lines[0]["id"], "place_wire");
        assert_eq!(lines[0]["args"], json!({"at": [1, 2]}));
        assert_eq!(lines[0]["result"], "ok");
        assert_eq!(lines[0]["took_ms"], 3);
        assert_eq!(lines[0]["undoable"], true);
        assert!(lines[0]["time"].as_str().unwrap().contains("14:02:01.000"));
        assert_eq!(lines[1]["source"], "agent");
        assert_eq!(lines[1]["client"], "bot");
        assert_eq!(lines[1]["pid"], 42);
        assert_eq!(lines[1]["result"], "failed");
        assert_eq!(lines[1]["text"], "E_LOCKED: U3");
    }
}
