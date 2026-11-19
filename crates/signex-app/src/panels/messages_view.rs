//! The Messages panel (#86): level chips with counts, the Source and
//! Document filters and a text filter, then the CLASS / DOCUMENT / SOURCE /
//! MESSAGE / TIME table. Copy and Clear sit in the dock title bar
//! (`super::title_actions`). DEBUG and TRACE rows show only when a verbose
//! log level is set. A row does not cross-probe (open decision on #86).

use iced::widget::pick_list;
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_widgets::chips::{ChipMetrics, ChipStyle, FilterSet, filter_chips};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::Label;

use super::messages_row::{MessageSource, clock_time};
use super::*;
use crate::diagnostics::{DiagnosticEntry, DiagnosticLevel};
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Average advance of a glyph in em, for sizing a column to its longest value.
const GLYPH_ADVANCE_EM: f32 = 0.62;
/// Width of the filter field, in em of the body size.
const FILTER_FIELD_EM: f32 = 20.0;
/// Clock time cells are `HH:MM:SS`.
const TIME_CHARS: usize = 8;

/// A level chip: every visible message, or one level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessageLevel {
    #[default]
    All,
    Error,
    Warning,
    Info,
}

impl MessageLevel {
    fn of(level: DiagnosticLevel) -> Option<Self> {
        match level {
            DiagnosticLevel::Error => Some(Self::Error),
            DiagnosticLevel::Warning => Some(Self::Warning),
            DiagnosticLevel::Info => Some(Self::Info),
            DiagnosticLevel::Debug | DiagnosticLevel::Trace => None,
        }
    }
}

/// What the Messages panel shows: view state, never the log itself.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MessagesView {
    pub level: MessageLevel,
    /// One source, or every source.
    pub source: Option<MessageSource>,
    /// One document, or every document.
    pub document: Option<String>,
    /// Text a shown message must contain (any case).
    pub query: String,
    /// Clear hides every entry up to this id; the log keeps them.
    pub cleared_through: Option<u64>,
}

/// What the user did in the Messages panel.
#[derive(Debug, Clone, PartialEq)]
pub enum MessagesPanelMsg {
    LevelPicked(MessageLevel),
    SourcePicked(Option<MessageSource>),
    DocumentPicked(Option<String>),
    QueryEdited(String),
    /// Copy in the title bar: the shown rows go to the clipboard.
    Copied,
    /// Clear in the title bar: every message so far is hidden.
    Cleared,
}

impl MessagesView {
    /// Apply a filter event. `newest` is the newest log entry's id, which
    /// Clear hides up to. Copy is the app's (it writes the clipboard).
    pub fn apply(&mut self, msg: MessagesPanelMsg, newest: Option<u64>) {
        match msg {
            MessagesPanelMsg::LevelPicked(level) => self.level = level,
            MessagesPanelMsg::SourcePicked(source) => self.source = source,
            MessagesPanelMsg::DocumentPicked(document) => self.document = document,
            MessagesPanelMsg::QueryEdited(query) => self.query = query,
            MessagesPanelMsg::Cleared => {
                if newest.is_some() {
                    self.cleared_through = newest;
                }
            }
            MessagesPanelMsg::Copied => {}
        }
    }
}

/// Whether the log level is verbose enough to show DEBUG and TRACE rows.
pub(crate) fn is_verbose(level_label: &str) -> bool {
    matches!(level_label, "debug" | "trace")
}

/// The entries left after Clear and the verbosity rule, newest first.
pub(crate) fn visible<'e>(
    entries: &'e [DiagnosticEntry],
    view: &MessagesView,
    verbose: bool,
) -> Vec<&'e DiagnosticEntry> {
    entries
        .iter()
        .rev()
        .filter(|e| view.cleared_through.is_none_or(|id| e.id > id))
        .filter(|e| verbose || MessageLevel::of(e.level).is_some())
        .collect()
}

/// The visible entries the chips, dropdowns and filter text let through.
/// A log entry names no document, so a document filter lets none through.
pub(crate) fn shown<'e>(
    entries: &'e [DiagnosticEntry],
    view: &MessagesView,
    verbose: bool,
) -> Vec<&'e DiagnosticEntry> {
    let query = view.query.trim().to_lowercase();
    visible(entries, view, verbose)
        .into_iter()
        .filter(|e| {
            view.level == MessageLevel::All || MessageLevel::of(e.level) == Some(view.level)
        })
        .filter(|e| {
            view.source
                .is_none_or(|s| MessageSource::of_code(&e.code) == s)
        })
        .filter(|_| view.document.is_none())
        .filter(|e| query.is_empty() || e.message.to_lowercase().contains(&query))
        .collect()
}

/// The chip counts: all visible entries, then errors, warnings and infos.
pub(crate) fn level_counts(visible: &[&DiagnosticEntry]) -> [u32; 4] {
    let count = |level| {
        visible
            .iter()
            .filter(|e| MessageLevel::of(e.level) == Some(level))
            .count() as u32
    };
    [
        visible.len() as u32,
        count(MessageLevel::Error),
        count(MessageLevel::Warning),
        count(MessageLevel::Info),
    ]
}

fn level_id(level: DiagnosticLevel) -> MessageId {
    match level {
        DiagnosticLevel::Error => ids::PANELS_MESSAGES_LEVEL_ERROR,
        DiagnosticLevel::Warning => ids::PANELS_MESSAGES_LEVEL_WARNING,
        DiagnosticLevel::Info => ids::PANELS_MESSAGES_LEVEL_INFO,
        DiagnosticLevel::Debug => ids::PANELS_MESSAGES_LEVEL_DEBUG,
        DiagnosticLevel::Trace => ids::PANELS_MESSAGES_LEVEL_TRACE,
    }
}

/// The shown rows as tab-separated text, for Copy.
pub(crate) fn copy_text(rows: &[&DiagnosticEntry], i18n: &I18n) -> String {
    rows.iter()
        .map(|e| {
            format!(
                "{}\t{}\t{}\t{}",
                t!(i18n, level_id(e.level)).into_string(),
                t!(i18n, MessageSource::of_code(&e.code).id()).into_string(),
                clock_time(e.at),
                e.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn messages_msg(msg: MessagesPanelMsg) -> PanelMsg {
    PanelMsg::MessagesView(msg)
}

fn chip_style(tokens: &ThemeTokens) -> ChipStyle {
    let r = resolve(tokens);
    ChipStyle {
        fill: r.field,
        edge: theme_ext::border_color(tokens),
        hover: theme_ext::hover_color(tokens),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    }
}

/// One entry of the Source or Document dropdown; `None` is "all".
#[derive(Debug, Clone, PartialEq)]
enum MessagesChoice {
    Source(Option<MessageSource>),
    Document(Option<String>),
}

fn choice_label(i18n: &I18n, choice: &MessagesChoice) -> String {
    match choice {
        MessagesChoice::Source(None) => t!(i18n, ids::PANELS_MESSAGES_ALL_SOURCES).into_string(),
        MessagesChoice::Source(Some(s)) => t!(i18n, s.id()).into_string(),
        MessagesChoice::Document(None) => {
            t!(i18n, ids::PANELS_MESSAGES_ALL_DOCUMENTS).into_string()
        }
        MessagesChoice::Document(Some(d)) => d.clone(),
    }
}

/// The toolbar: level chips, Source and Document, and the filter field.
fn toolbar<'a>(ctx: &'a PanelContext, visible: &[&DiagnosticEntry]) -> Element<'a, PanelMsg> {
    let i18n: &'a I18n = &ctx.i18n;
    let view = &ctx.validation.messages;
    let [all, errors, warnings, infos] = level_counts(visible);
    let chips = filter_chips(
        [
            (
                MessageLevel::All,
                Label::from(t!(i18n, ids::PANELS_MESSAGES_LEVEL_ALL)),
                Some(all),
            ),
            (
                MessageLevel::Error,
                Label::from(t!(i18n, ids::PANELS_MESSAGES_LEVEL_ERROR)),
                Some(errors),
            ),
            (
                MessageLevel::Warning,
                Label::from(t!(i18n, ids::PANELS_MESSAGES_LEVEL_WARNING)),
                Some(warnings),
            ),
            (
                MessageLevel::Info,
                Label::from(t!(i18n, ids::PANELS_MESSAGES_LEVEL_INFO)),
                Some(infos),
            ),
        ],
        &FilterSet::new([view.level]),
        chip_style(&ctx.tokens),
        ChipMetrics::from_text_size(TS.body),
        i18n,
        |level| messages_msg(MessagesPanelMsg::LevelPicked(level)),
    );
    let mut sources = vec![MessagesChoice::Source(None)];
    let mut present: Vec<MessageSource> = visible
        .iter()
        .map(|e| MessageSource::of_code(&e.code))
        .collect();
    present.sort();
    present.dedup();
    sources.extend(present.into_iter().map(|s| MessagesChoice::Source(Some(s))));
    let label = move |choice: &MessagesChoice| choice_label(i18n, choice);
    let dropdown = |selected: MessagesChoice, options: Vec<MessagesChoice>| {
        pick_list(Some(selected), options, label)
            .graphite()
            .on_select(|choice: MessagesChoice| match choice {
                MessagesChoice::Source(s) => messages_msg(MessagesPanelMsg::SourcePicked(s)),
                MessagesChoice::Document(d) => messages_msg(MessagesPanelMsg::DocumentPicked(d)),
            })
            .boxed()
    };
    let filter = text_input(t!(i18n, ids::PANELS_MESSAGES_FILTER_HINT), &view.query)
        .on_input(|q| messages_msg(MessagesPanelMsg::QueryEdited(q)))
        .size(TS.body)
        .padding(SP.s)
        .style(super::form_look::text_input_style)
        .width(Length::Fixed(TS.body * FILTER_FIELD_EM));
    let line = resolve(&ctx.tokens).line_strong;
    let rule = container(Space::new())
        .width(Length::Fixed(crate::styles::HAIRLINE))
        .height(Length::Fixed(TS.body * crate::styles::LINE_HEIGHT))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        });
    row![
        chips,
        rule,
        dropdown(MessagesChoice::Source(view.source), sources),
        dropdown(
            MessagesChoice::Document(view.document.clone()),
            vec![MessagesChoice::Document(None)],
        ),
        Space::new().width(Length::Fill).height(Length::Shrink),
        filter,
    ]
    .spacing(SP.s)
    .padding([SP.s, SP.m])
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// Width of a column whose longest value is `chars` long at `size`.
fn column_width(chars: usize, size: f32, pad: f32) -> f32 {
    (chars as f32 * size * GLYPH_ADVANCE_EM + 2.0 * pad).ceil()
}

/// Fixed column widths: CLASS, DOCUMENT, SOURCE, TIME, each sized to its
/// longest value or its header.
fn column_widths(i18n: &I18n, rows: &[&DiagnosticEntry]) -> [f32; 4] {
    let chars = |id: MessageId| t!(i18n, id).into_string().chars().count();
    let pad = SP.m;
    let class = rows
        .iter()
        .map(|e| chars(level_id(e.level)) + 2)
        .chain([chars(ids::PANELS_MESSAGES_COLUMN_CLASS)])
        .max()
        .unwrap_or_default();
    let document =
        chars(ids::PANELS_MESSAGES_COLUMN_DOCUMENT).max(chars(ids::PANELS_MESSAGES_NO_VALUE));
    let source = rows
        .iter()
        .map(|e| chars(MessageSource::of_code(&e.code).id()))
        .chain([chars(ids::PANELS_MESSAGES_COLUMN_SOURCE)])
        .max()
        .unwrap_or_default();
    let time = TIME_CHARS.max(chars(ids::PANELS_MESSAGES_COLUMN_TIME));
    [
        column_width(class, TS.caption, pad),
        column_width(document, TS.body, pad),
        column_width(source, TS.body, pad),
        column_width(time, TS.body, pad),
    ]
}

fn level_pill<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    level: DiagnosticLevel,
) -> Element<'a, PanelMsg> {
    let tone = match level {
        DiagnosticLevel::Error => PillTone::Err,
        DiagnosticLevel::Warning => PillTone::Warn,
        DiagnosticLevel::Info => PillTone::Info,
        DiagnosticLevel::Debug | DiagnosticLevel::Trace => PillTone::Neutral,
    };
    container(text(t!(i18n, level_id(level))).size(TS.caption))
        .padding([0.0, SP.s])
        .style(crate::styles::pill(tokens, tone))
        .boxed()
}

fn cell<'a>(content: Element<'a, PanelMsg>, width: Length) -> Element<'a, PanelMsg> {
    container(content)
        .padding([SP.xs, SP.m])
        .width(width)
        .clip(true)
        .boxed()
}

/// The empty state: centred title, the hint, and the variables in mono.
fn empty_state<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let r = resolve(&ctx.tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    container(
        column![
            text(t!(i18n, ids::PANELS_MESSAGES_EMPTY))
                .size(TS.body)
                .font(iced::Font {
                    weight: iced::font::Weight::Medium,
                    ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
                })
                .color(theme_ext::text_primary(&ctx.tokens)),
            text(t!(i18n, ids::PANELS_MESSAGES_VERBOSE_HINT))
                .size(TS.caption)
                .color(r.ink3),
            // Environment variables: code, the same in every language.
            text(UserText::new("RUST_LOG=debug  ·  SIGNEX_LOG=debug"))
                .size(TS.caption)
                .font(mono)
                .color(r.ink3),
        ]
        .spacing(SP.xs)
        .align_x(iced::Alignment::Center),
    )
    .center_x(Length::Fill)
    .padding(SP.xl)
    .boxed()
}

/// The Graphite Messages table under its toolbar.
pub fn view_messages<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let tokens = &ctx.tokens;
    let view = &ctx.validation.messages;
    let verbose = is_verbose(&ctx.diagnostics_level);
    let visible = visible(&ctx.diagnostics, view, verbose);
    if visible.is_empty() {
        return empty_state(ctx);
    }
    let rows = shown(&ctx.diagnostics, view, verbose);
    let r = resolve(tokens);
    let (sunken, line, ink3, ink4, err_soft) = (r.sunken, r.line_soft, r.ink3, r.ink4, r.err_soft);
    let ink = theme_ext::text_primary(tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let [class_w, document_w, source_w, time_w] = column_widths(i18n, &rows);
    let rule = move || -> Element<'a, PanelMsg> {
        container(Space::new())
            .height(Length::Fixed(crate::styles::HAIRLINE))
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(line)),
                ..container::Style::default()
            })
            .boxed()
    };
    let caps = move |label: MessageId| -> Element<'a, PanelMsg> {
        text(t!(i18n, label)).size(TS.section).color(ink3).boxed()
    };
    let mut col: Column<Element<'a, PanelMsg>> = Column::new()
        .width(Length::Fill)
        .push(toolbar(ctx, &visible))
        .push(rule());
    col = col.push(
        container(
            row![
                cell(
                    caps(ids::PANELS_MESSAGES_COLUMN_CLASS),
                    Length::Fixed(class_w)
                ),
                cell(
                    caps(ids::PANELS_MESSAGES_COLUMN_DOCUMENT),
                    Length::Fixed(document_w)
                ),
                cell(
                    caps(ids::PANELS_MESSAGES_COLUMN_SOURCE),
                    Length::Fixed(source_w)
                ),
                cell(caps(ids::PANELS_MESSAGES_COLUMN_MESSAGE), Length::Fill),
                cell(
                    caps(ids::PANELS_MESSAGES_COLUMN_TIME),
                    Length::Fixed(time_w)
                ),
            ]
            .align_y(iced::Alignment::Center)
            .width(Length::Fill),
        )
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            ..container::Style::default()
        })
        .boxed(),
    );
    col = col.push(rule());
    if rows.is_empty() {
        col = col.push(
            container(
                text(t!(i18n, ids::PANELS_MESSAGES_NONE_MATCH))
                    .size(TS.body)
                    .color(ink3),
            )
            .padding([SP.s, SP.m])
            .boxed(),
        );
        return col.boxed();
    }
    for entry in rows {
        let fill = (entry.level == DiagnosticLevel::Error).then_some(err_soft);
        col = col.push(
            container(
                row![
                    cell(
                        level_pill(tokens, i18n, entry.level),
                        Length::Fixed(class_w)
                    ),
                    cell(
                        text(t!(i18n, ids::PANELS_MESSAGES_NO_VALUE))
                            .size(TS.body)
                            .color(ink4)
                            .boxed(),
                        Length::Fixed(document_w),
                    ),
                    cell(
                        text(t!(i18n, MessageSource::of_code(&entry.code).id()))
                            .size(TS.body)
                            .color(ink3)
                            .wrapping(iced::widget::text::Wrapping::None)
                            .boxed(),
                        Length::Fixed(source_w),
                    ),
                    cell(
                        text(UserText::new(entry.message.as_str()))
                            .size(TS.body)
                            .color(ink)
                            .boxed(),
                        Length::Fill,
                    ),
                    cell(
                        text(UserText::new(clock_time(entry.at)))
                            .size(TS.body)
                            .font(mono)
                            .color(ink3)
                            .wrapping(iced::widget::text::Wrapping::None)
                            .boxed(),
                        Length::Fixed(time_w),
                    ),
                ]
                .align_y(iced::Alignment::Start)
                .width(Length::Fill),
            )
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: fill.map(Background::Color),
                ..container::Style::default()
            })
            .boxed(),
        );
        col = col.push(rule());
    }
    col.boxed()
}

#[cfg(test)]
#[path = "messages_view_tests.rs"]
mod tests;
