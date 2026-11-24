//! The Messages panel (#86): level chips with counts, the Source and
//! Document filters and a text filter, then the CLASS / DOCUMENT / SOURCE /
//! MESSAGE / TIME table. Copy and Clear sit in the dock title bar
//! (`super::title_actions`). DEBUG and TRACE rows show only when a verbose
//! log level is set. A row that names a sheet offers Show, which opens the
//! sheet and selects the object it names; a plain click does nothing
//! (mockup row 86).

use std::path::PathBuf;

use iced::widget::{Row, column, pick_list};
use signex_i18n::{I18n, MessageId, UserText, ids, t};
use signex_types::schematic::SelectedItem;
use signex_widgets::chips::{ChipMetrics, ChipStyle, ChipTone, chip, tag_chip};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::text_input;

use super::messages_row::{MessageSource, clock_time, document_name, named_documents};
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
    pub document: Option<PathBuf>,
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
    DocumentPicked(Option<PathBuf>),
    QueryEdited(String),
    /// A row's Show: open `document` and select `object` on it.
    ShowPressed {
        document: PathBuf,
        object: Option<SelectedItem>,
    },
    /// Copy in the title bar: the shown rows go to the clipboard.
    Copied,
    /// Clear in the title bar: every message so far is hidden.
    Cleared,
}

impl MessagesView {
    /// Apply a filter event. `newest` is the newest log entry's id, which
    /// Clear hides up to. Copy and Show are the app's (they write the
    /// clipboard, open a sheet).
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
            MessagesPanelMsg::Copied | MessagesPanelMsg::ShowPressed { .. } => {}
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
/// A document filter lets through only the entries that name that sheet.
pub(crate) fn shown<'e>(
    entries: &'e [DiagnosticEntry],
    view: &MessagesView,
    verbose: bool,
    i18n: &I18n,
) -> Vec<&'e DiagnosticEntry> {
    let query = i18n.fold_for_search(view.query.trim());
    visible(entries, view, verbose)
        .into_iter()
        .filter(|e| {
            view.level == MessageLevel::All || MessageLevel::of(e.level) == Some(view.level)
        })
        .filter(|e| {
            view.source
                .is_none_or(|s| MessageSource::of_code(&e.code) == s)
        })
        .filter(|e| {
            view.document
                .as_ref()
                .is_none_or(|d| e.document.as_ref() == Some(d))
        })
        .filter(|e| query.is_empty() || i18n.fold_for_search(&e.message).contains(&query))
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

/// The tone of a level: the row's CLASS pill and the level chip's tag.
fn level_tone(level: DiagnosticLevel) -> PillTone {
    match level {
        DiagnosticLevel::Error => PillTone::Err,
        DiagnosticLevel::Warning => PillTone::Warn,
        DiagnosticLevel::Info => PillTone::Info,
        DiagnosticLevel::Debug | DiagnosticLevel::Trace => PillTone::Neutral,
    }
}

/// Fill and ink of a level chip's tag, from the level's pill tone.
pub(crate) fn level_tag_tone(tokens: &ThemeTokens, level: DiagnosticLevel) -> ChipTone {
    let (fill, ink) = crate::styles::pill_colors(tokens, level_tone(level));
    ChipTone { fill, ink }
}

/// All, then ERROR, WARN and INFO as mono tags in their level's tone, each
/// with its count (mockup row 86).
fn level_chips<'a>(ctx: &'a PanelContext, visible: &[&DiagnosticEntry]) -> Element<'a, PanelMsg> {
    let i18n: &'a I18n = &ctx.i18n;
    let picked = ctx.validation.messages.level;
    let style = chip_style(&ctx.tokens);
    let metrics = ChipMetrics::from_text_size(TS.body);
    let [all, errors, warnings, infos] = level_counts(visible);
    let press = |level| Some(messages_msg(MessagesPanelMsg::LevelPicked(level)));
    let mut chips: Row<Element<'a, PanelMsg>> = Row::new().spacing(metrics.row_gap).push(chip(
        t!(i18n, ids::PANELS_MESSAGES_LEVEL_ALL),
        Some(all),
        picked == MessageLevel::All,
        style,
        metrics,
        i18n,
        press(MessageLevel::All),
    ));
    for (level, diagnostic, count) in [
        (MessageLevel::Error, DiagnosticLevel::Error, errors),
        (MessageLevel::Warning, DiagnosticLevel::Warning, warnings),
        (MessageLevel::Info, DiagnosticLevel::Info, infos),
    ] {
        chips = chips.push(tag_chip(
            t!(i18n, level_id(diagnostic)),
            level_tag_tone(&ctx.tokens, diagnostic),
            None,
            Some(count),
            picked == level,
            style,
            metrics,
            i18n,
            press(level),
        ));
    }
    chips.wrap().vertical_spacing(metrics.row_gap).boxed()
}

/// The filter field: the search glyph and a borderless input in one field
/// frame (mockup row 86).
fn filter_field<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let ink4 = r.ink4;
    let ink = crate::styles::ti(tokens.text);
    let accent = crate::styles::ti(tokens.accent);
    let glyph = iced::widget::svg(crate::icons::icon_chrome_search(ctx.theme_id))
        .width(TS.body)
        .height(TS.body)
        .style(move |_: &Theme, _| iced::widget::svg::Style {
            color: Some(ink4),
            ..iced::widget::svg::Style::default()
        });
    let input = text_input(
        t!(ctx.i18n, ids::PANELS_MESSAGES_FILTER_HINT),
        &ctx.validation.messages.query,
    )
    .on_input(|q| messages_msg(MessagesPanelMsg::QueryEdited(q)))
    .size(TS.body)
    .padding(iced::Padding::ZERO)
    .width(Length::Fill)
    .style(move |_: &Theme, _| iced::widget::text_input::Style {
        background: Background::Color(iced::Color::TRANSPARENT),
        border: iced::Border::default(),
        placeholder: ink4,
        value: ink,
        selection: iced::Color { a: 0.4, ..accent },
    });
    let field_height = crate::styles::ChromeMetrics::DEFAULT.field;
    container(
        row![glyph.boxed(), input.boxed()]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center),
    )
    .padding([0.0, SP.m])
    .width(Length::Fixed(TS.body * FILTER_FIELD_EM))
    .center_y(field_height)
    .style(crate::styles::field(tokens, false))
    .boxed()
}

/// One entry of the Source or Document dropdown; `None` is "all".
#[derive(Debug, Clone, PartialEq)]
enum MessagesChoice {
    Source(Option<MessageSource>),
    Document(Option<PathBuf>),
}

fn choice_label(i18n: &I18n, choice: &MessagesChoice) -> String {
    match choice {
        MessagesChoice::Source(None) => t!(i18n, ids::PANELS_MESSAGES_ALL_SOURCES).into_string(),
        MessagesChoice::Source(Some(s)) => t!(i18n, s.id()).into_string(),
        MessagesChoice::Document(None) => {
            t!(i18n, ids::PANELS_MESSAGES_ALL_DOCUMENTS).into_string()
        }
        MessagesChoice::Document(Some(d)) => document_name(d),
    }
}

/// The toolbar: level chips, Source and Document, and the filter field.
fn toolbar<'a>(ctx: &'a PanelContext, visible: &[&DiagnosticEntry]) -> Element<'a, PanelMsg> {
    let i18n: &'a I18n = &ctx.i18n;
    let view = &ctx.validation.messages;
    let mut sources = vec![MessagesChoice::Source(None)];
    let mut present: Vec<MessageSource> = visible
        .iter()
        .map(|e| MessageSource::of_code(&e.code))
        .collect();
    present.sort();
    present.dedup();
    sources.extend(present.into_iter().map(|s| MessagesChoice::Source(Some(s))));
    let mut documents = vec![MessagesChoice::Document(None)];
    documents.extend(
        named_documents(visible)
            .into_iter()
            .map(|d| MessagesChoice::Document(Some(d))),
    );
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
    let line = resolve(&ctx.tokens).line_strong;
    let rule = container(Space::new())
        .width(Length::Fixed(crate::styles::HAIRLINE))
        .height(Length::Fixed(TS.body * crate::styles::LINE_HEIGHT))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        });
    row![
        level_chips(ctx, visible),
        rule,
        dropdown(MessagesChoice::Source(view.source), sources),
        dropdown(MessagesChoice::Document(view.document.clone()), documents),
        Space::new().width(Length::Fill).height(Length::Shrink),
        filter_field(ctx),
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
    let document = rows
        .iter()
        .filter_map(|e| e.document.as_deref())
        .map(|d| document_name(d).chars().count())
        .chain([
            chars(ids::PANELS_MESSAGES_COLUMN_DOCUMENT),
            chars(ids::PANELS_MESSAGES_NO_VALUE),
        ])
        .max()
        .unwrap_or_default();
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
    container(text(t!(i18n, level_id(level))).size(TS.caption))
        .padding([0.0, SP.s])
        .style(crate::styles::pill(tokens, level_tone(level)))
        .boxed()
}

fn cell<'a>(content: Element<'a, PanelMsg>, width: Length) -> Element<'a, PanelMsg> {
    container(content)
        .padding([SP.xs, SP.m])
        .width(width)
        .clip(true)
        .boxed()
}

/// The Document cell: the file name of the sheet the row names, else a dash.
fn document_text<'a>(
    i18n: &I18n,
    entry: &DiagnosticEntry,
    ink: iced::Color,
    none_ink: iced::Color,
) -> Element<'a, PanelMsg> {
    let cell = match entry.document.as_deref() {
        Some(document) => text(UserText::new(document_name(document))).color(ink),
        None => text(t!(i18n, ids::PANELS_MESSAGES_NO_VALUE)).color(none_ink),
    };
    cell.size(TS.body)
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

/// What a row's Show sends: the sheet it names and the object on it.
fn show_message(entry: &DiagnosticEntry) -> Option<MessagesPanelMsg> {
    Some(MessagesPanelMsg::ShowPressed {
        document: entry.document.clone()?,
        object: entry.object,
    })
}

/// The Message cell of a Messages row: the text, then the link to the command
/// the row offers, if any (`messages_row::row_action`), and Show when the
/// row names a sheet.
fn message_text<'a>(
    tokens: &ThemeTokens,
    i18n: &I18n,
    entry: &'a DiagnosticEntry,
    ink: iced::Color,
) -> Element<'a, PanelMsg> {
    let line = text(UserText::new(entry.message.as_str()))
        .size(TS.body)
        .color(ink);
    let link = super::messages_row::row_action(entry)
        .map(|(link, label)| (PanelMsg::MessageLinkPressed(link), label));
    let show = show_message(entry).map(|msg| (messages_msg(msg), ids::PANELS_MESSAGES_SHOW));
    if link.is_none() && show.is_none() {
        return line.boxed();
    }
    let mut cell: Row<Element<'a, PanelMsg>> = Row::new()
        .push(line.boxed())
        .push(Space::new().width(6).height(Length::Shrink).boxed())
        .align_y(iced::Alignment::Center);
    for (press, label) in link.into_iter().chain(show) {
        cell = cell.push(
            signex_widgets::button(text(t!(i18n, label)).size(TS.body).color(ink))
                .padding([SP.xs, SP.m])
                .on_press(press)
                .style(crate::styles::menu_item(tokens))
                .boxed(),
        );
    }
    cell.boxed()
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
    let rows = shown(&ctx.diagnostics, view, verbose, i18n);
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
                        document_text(i18n, entry, ink3, ink4),
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
                    cell(message_text(tokens, i18n, entry, ink), Length::Fill),
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
