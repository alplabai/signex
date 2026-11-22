//! The Synchronize Sheet Entries and Ports dialog (#1276, mockup row 79):
//! which side wins as two option cards, a live drawing of the written side
//! (After / Before), then the change table. Nothing changes until Apply.
//! State and rules: `app/sheet_sync/`.

mod preview;
mod table;

use iced::widget::{Column, Row, Space, column, container, row, scrollable};
use iced::{Alignment, Border, Color, Element, Length, Theme, Widget as _};
use signex_engine::{SheetSyncDirection, SheetSyncEntry};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::property_illustration::{IllustrationHover, property_illustration};
use signex_widgets::{Label, segmented, text};

use super::board_output::kit::section_label;
use super::sizes;
use crate::app::sheet_sync::{
    SheetSyncMsg, SheetSyncPreviewMode, SheetSyncProblem, SheetSyncState,
};
use crate::app::{Message, Signex};
use crate::styles::{
    DialogButtonKind, HAIRLINE, PillTone, Radii, Spacing, TypeScale, dialog_button, pill_colors,
    ti, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the After / Before control, in body ems (140 px).
const MODE_EM: f32 = 35.0 / 3.0;
/// Side of a legend swatch, in captions.
const SWATCH_EM: f32 = 1.0;

fn ss(msg: SheetSyncMsg) -> Message {
    Message::from(msg)
}

/// Horizontal padding of the dialog body.
fn body_padding() -> f32 {
    SP.xl + SP.m
}

impl Signex {
    pub(in crate::app::view) fn view_sheet_sync_dialog(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(self.view_sheet_sync_card(), sizes::SHEET_SYNC)
    }

    fn view_sheet_sync_card(&self) -> Element<'_, Message> {
        let Some(state) = self.ui_state.sheet_sync.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let cancel = Some(ss(SheetSyncMsg::CancelRequested));
        let subtitle = t!(
            i18n,
            ids::SHEET_SYNC_SUBTITLE,
            "symbol" => state.symbol_name.as_str(),
            "file" => state.child_filename.as_str()
        )
        .into_string();
        let header = super::header::graphite_header_closing(
            t!(i18n, ids::SHEET_SYNC_TITLE).into_string(),
            Some(subtitle),
            None,
            cancel.clone(),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let (width, height) =
            super::widgets::fit_modal_size(self.ui_state.window_size, sizes::SHEET_SYNC);
        let inner = width - 2.0 * body_padding();
        let content = container(body(state, inner, i18n, tokens))
            .padding([SP.xl, body_padding()])
            .width(Length::Fill);
        let scroller = scrollable(content)
            .direction(crate::styles::thin_vertical())
            .style(crate::styles::scrollbar_style(tokens))
            .width(Length::Fill)
            .height(Length::Fill);
        let footer = footer(state, cancel, i18n, tokens);
        container(column![header, scroller, footer].width(Length::Fixed(width)))
            .height(Length::Fixed(height))
            .style(crate::styles::modal_card(tokens))
            .clip(true)
            .boxed()
    }
}

/// Cancel, and Apply with its count.
fn footer<'a>(
    state: &SheetSyncState,
    cancel: Option<Message>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    crate::styles::dialog_footer_split(
        Vec::new(),
        vec![
            dialog_button(
                t!(i18n, ids::SHEET_SYNC_CANCEL),
                DialogButtonKind::Cancel,
                cancel,
                tokens,
            ),
            dialog_button(
                apply_words(state, i18n),
                super::graphite::call_to_action(tokens),
                state.can_apply().then(|| ss(SheetSyncMsg::ApplyRequested)),
                tokens,
            ),
        ],
    )
}

/// How many changes Apply makes in the chosen direction.
fn change_count(state: &SheetSyncState) -> usize {
    state.preview().map_or(0, |preview| {
        preview.added.len() + preview.removed.len() + preview.renamed.len()
    })
}

/// Apply, with its count while it has something to do.
fn apply_words(state: &SheetSyncState, i18n: &I18n) -> Localized {
    match change_count(state) {
        0 => t!(i18n, ids::SHEET_SYNC_APPLY),
        count => t!(i18n, ids::SHEET_SYNC_APPLY_COUNT, "count" => count),
    }
}

/// What the drawing and the table are about: the written side first, then
/// the other side's name.
fn written_side(state: &SheetSyncState) -> (&str, &str) {
    let (symbol, file) = (state.symbol_name.as_str(), state.child_filename.as_str());
    match state.direction {
        SheetSyncDirection::SheetToSymbol => (symbol, file),
        SheetSyncDirection::SymbolToSheet => (file, symbol),
    }
}

/// The direction cards, the drawing, the change table (or why there is
/// none), the reason Apply refused, and the note on undo and saving.
fn body<'a>(
    state: &'a SheetSyncState,
    width: f32,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.l).width(Length::Fill);
    col = col.push(direction_section(state, i18n, tokens));
    if let Some(entries) = state.entries() {
        col = col.push(preview_section(state, entries, width, i18n, tokens));
    }
    let status = match (state.preview(), &state.problem) {
        (None, Some(problem)) => Some(problem_text(problem, i18n)),
        (None, None) => Some(t!(i18n, ids::SHEET_SYNC_READING)),
        (Some(preview), _) if preview.is_empty() => Some(t!(i18n, ids::SHEET_SYNC_NONE)),
        (Some(_), _) => None,
    };
    if let Some(line) = status {
        col = col.push(text(line).size(TS.body).color(ti(tokens.text)).boxed());
    }
    if let Some(entries) = state.entries().filter(|_| change_count(state) > 0) {
        col = col.push(changes_section(state, entries, i18n, tokens));
    }
    if state.preview().is_some()
        && let Some(problem) = state.problem.as_ref()
    {
        col = col.push(
            text(problem_text(problem, i18n))
                .size(TS.body)
                .color(tokens::danger(tokens))
                .boxed(),
        );
    }
    col.push(
        text(t!(i18n, ids::SHEET_SYNC_INTRO))
            .size(TS.caption)
            .color(tokens::resolve(tokens).ink4)
            .boxed(),
    )
    .boxed()
}

/// The WHICH SIDE WINS heading over its two option cards.
fn direction_section<'a>(
    state: &SheetSyncState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let names = |id: MessageId| {
        t!(
            i18n,
            id,
            "symbol" => state.symbol_name.as_str(),
            "file" => state.child_filename.as_str()
        )
    };
    let cards = row![
        direction_card(
            (SheetSyncDirection::SheetToSymbol, state.direction),
            ids::SHEET_SYNC_TO_SYMBOL,
            names(ids::SHEET_SYNC_TO_SYMBOL_LINE),
            i18n,
            tokens,
        ),
        direction_card(
            (SheetSyncDirection::SymbolToSheet, state.direction),
            ids::SHEET_SYNC_TO_SHEET,
            names(ids::SHEET_SYNC_TO_SHEET_LINE),
            i18n,
            tokens,
        ),
    ]
    .spacing(SP.m);
    column![
        section_label(t!(i18n, ids::SHEET_SYNC_WHICH_SIDE), i18n, tokens),
        cards
    ]
    .spacing(SP.s)
    .boxed()
}

/// One option card: a radio, the direction and what it moves where.
fn direction_card<'a>(
    (direction, chosen): (SheetSyncDirection, SheetSyncDirection),
    title: MessageId,
    line: Localized,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let on = direction == chosen;
    let words = column![
        text(t!(i18n, title)).size(TS.body).font(iced::Font {
            weight: iced::font::Weight::Semibold,
            ..signex_widgets::ui_font::regular()
        }),
        text(line)
            .size(TS.caption)
            .color(tokens::muted_text(tokens)),
    ]
    .spacing(SP.xs)
    .width(Length::Fill);
    let content = row![super::board_reannotate::radio_mark(on, tokens), words]
        .spacing(SP.m)
        .align_y(Alignment::Start);
    signex_widgets::button(content.boxed())
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .on_press(ss(SheetSyncMsg::DirectionPicked(direction)))
        .style(super::graphite_annotate::toggle_style(tokens, on))
        .boxed()
}

/// The PREVIEW header with After / Before and expand, the drawing, the legend.
fn preview_section<'a>(
    state: &SheetSyncState,
    entries: &[SheetSyncEntry],
    width: f32,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mode = state.preview_mode;
    let (title, other) = written_side(state);
    let rows = preview::rows_of(entries, mode);
    let size = preview::PreviewSize::new(width, state.preview_expanded, rows, TS.caption);
    let drawing = property_illustration(
        preview::illustration(
            entries,
            mode,
            (title, other),
            ti(tokens.success),
            size,
            i18n,
        ),
        &IllustrationHover::default(),
        tokens,
        size.text_size,
    )
    .map(|_| Message::noop());
    let (paper, edge) = (ti(tokens.paper), ti(tokens.border));
    let frame = container(drawing)
        .width(Length::Fill)
        .height(Length::Fixed(size.height))
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: Some(paper.into()),
            border: Border {
                width: HAIRLINE,
                color: edge,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    column![
        preview_header(state, title, i18n, tokens),
        frame,
        legend(i18n, tokens)
    ]
    .spacing(SP.s)
    .boxed()
}

/// PREVIEW, what it shows, the After / Before control and the expand button.
fn preview_header<'a>(
    state: &SheetSyncState,
    title: &str,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let caption = match state.preview_mode {
        SheetSyncPreviewMode::After => {
            t!(i18n, ids::SHEET_SYNC_PREVIEW_AFTER_CAPTION, "name" => title)
        }
        SheetSyncPreviewMode::Before => {
            t!(i18n, ids::SHEET_SYNC_PREVIEW_BEFORE_CAPTION, "name" => title)
        }
    };
    let modes = [
        (
            SheetSyncPreviewMode::After,
            Label::from(t!(i18n, ids::SHEET_SYNC_PREVIEW_AFTER)),
        ),
        (
            SheetSyncPreviewMode::Before,
            Label::from(t!(i18n, ids::SHEET_SYNC_PREVIEW_BEFORE)),
        ),
    ];
    let style = tokens::resolve(tokens).segmented(TS.body).fill_width(true);
    let picker = segmented(modes, state.preview_mode, style, |mode| {
        ss(SheetSyncMsg::PreviewModePicked(mode))
    });
    let tip = if state.preview_expanded {
        ids::SHEET_SYNC_PREVIEW_COLLAPSE
    } else {
        ids::SHEET_SYNC_PREVIEW_EXPAND
    };
    row![
        section_label(t!(i18n, ids::SHEET_SYNC_PREVIEW), i18n, tokens),
        text(caption)
            .size(TS.caption)
            .color(tokens::resolve(tokens).ink4),
        Space::new().width(Length::Fill),
        container(picker).width(Length::Fixed(TS.body * MODE_EM)),
        super::grid_properties::expand_button(
            state.preview_expanded,
            tip,
            ss(SheetSyncMsg::PreviewExpandToggled),
            i18n,
            tokens,
        ),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// A legend swatch with its name.
fn legend_item<'a>(
    fill: Color,
    edge: Color,
    name: MessageId,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let side = TS.caption * SWATCH_EM;
    let swatch = container(Space::new())
        .width(Length::Fixed(side))
        .height(Length::Fixed(side))
        .style(move |_: &Theme| container::Style {
            background: Some(fill.into()),
            border: Border {
                width: HAIRLINE,
                color: edge,
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    row![
        swatch,
        text(t!(i18n, name))
            .size(TS.caption)
            .color(tokens::muted_text(tokens)),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center)
    .boxed()
}

/// Added, Renamed and Removed as the drawing inks them, and the shape rule.
fn legend<'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let (ok_fill, ok) = pill_colors(tokens, PillTone::Ok);
    let err = pill_colors(tokens, PillTone::Err).1;
    let line: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .push(legend_item(
            ok_fill,
            ok,
            ids::SHEET_SYNC_ADDED,
            i18n,
            tokens,
        ))
        .push(legend_item(
            r.accent_soft,
            ti(tokens.accent),
            ids::SHEET_SYNC_RENAMED,
            i18n,
            tokens,
        ))
        .push(legend_item(
            Color::TRANSPARENT,
            err,
            ids::SHEET_SYNC_REMOVED,
            i18n,
            tokens,
        ))
        .push(
            text(t!(i18n, ids::SHEET_SYNC_LEGEND_SHAPES))
                .size(TS.caption)
                .color(tokens::muted_text(tokens))
                .boxed(),
        );
    line.wrap().vertical_spacing(SP.xs).boxed()
}

/// CHANGES TO the written side with the count, over the table.
fn changes_section<'a>(
    state: &SheetSyncState,
    entries: &[SheetSyncEntry],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let rows = table::changed_rows(entries);
    let (title, _) = written_side(state);
    let heading = row![
        section_label(
            t!(i18n, ids::SHEET_SYNC_CHANGES_TO, "name" => title),
            i18n,
            tokens
        ),
        Space::new().width(Length::Fill),
        text(t!(i18n, ids::SHEET_SYNC_CHANGE_COUNT, "count" => rows.len()))
            .size(TS.caption)
            .color(tokens::resolve(tokens).ink4),
    ]
    .align_y(Alignment::Center);
    column![heading, table::view(&rows, i18n, tokens)]
        .spacing(SP.s)
        .boxed()
}

/// Why there is no preview or Apply refused, in the dialog's language.
fn problem_text(problem: &SheetSyncProblem, i18n: &I18n) -> Localized {
    let path = |path: &std::path::Path| path.display().to_string();
    match problem {
        SheetSyncProblem::ReadFailed(why) => {
            t!(i18n, ids::SHEET_SYNC_ERR_READ, "why" => why.as_str())
        }
        SheetSyncProblem::Stale => t!(i18n, ids::SHEET_SYNC_ERR_STALE),
        SheetSyncProblem::Changed => t!(i18n, ids::SHEET_SYNC_ERR_CHANGED),
        SheetSyncProblem::Busy(sheet) => t!(i18n, ids::SHEET_SYNC_ERR_BUSY, "path" => path(sheet)),
        SheetSyncProblem::ReadOnly(sheet) => {
            t!(i18n, ids::SHEET_SYNC_ERR_READ_ONLY, "path" => path(sheet))
        }
        SheetSyncProblem::NotLoaded(sheet) => {
            t!(i18n, ids::SHEET_SYNC_ERR_NOT_LOADED, "path" => path(sheet))
        }
        SheetSyncProblem::NotChanged(sheet) => {
            t!(i18n, ids::SHEET_SYNC_ERR_NOT_CHANGED, "path" => path(sheet))
        }
        SheetSyncProblem::DiskChanged(sheet) => {
            t!(i18n, ids::SHEET_SYNC_ERR_DISK_CHANGED, "path" => path(sheet))
        }
    }
}

#[cfg(test)]
mod tests {
    use signex_engine::{SheetEntryChange, SheetSyncPreview};

    use super::*;
    use crate::app::sheet_sync::{SheetSyncEntries, SheetSyncPreviews};

    fn state() -> SheetSyncState {
        let entry = SheetSyncEntry {
            name: "SDA".to_string(),
            direction: "input",
            change: SheetEntryChange::Added,
        };
        let preview = SheetSyncPreview {
            added: vec!["SDA".to_string()],
            ..SheetSyncPreview::default()
        };
        SheetSyncState {
            generation: 1,
            parent: "top.snxsch".into(),
            symbol: uuid::Uuid::nil(),
            symbol_name: "U_Power".to_string(),
            child_filename: "power.snxsch".to_string(),
            child_path: "power.snxsch".into(),
            epoch: 0,
            direction: SheetSyncDirection::default(),
            read: None,
            previews: Some(SheetSyncPreviews {
                to_symbol: preview.clone(),
                to_sheet: preview,
            }),
            entries: Some(SheetSyncEntries {
                to_symbol: vec![entry.clone()],
                to_sheet: vec![entry],
            }),
            preview_mode: SheetSyncPreviewMode::default(),
            preview_expanded: false,
            checking: false,
            problem: None,
        }
    }

    /// Mockup row 79: Apply carries the number of changes.
    #[test]
    fn apply_shows_its_change_count() {
        let words = apply_words(&state(), I18n::en_us_ref()).into_string();
        assert!(words.contains("Apply") && words.contains('1'), "{words}");
        let mut none = state();
        none.previews = Some(SheetSyncPreviews::default());
        let words = apply_words(&none, I18n::en_us_ref()).into_string();
        assert_eq!(words, "Apply");
    }

    /// The written side names the drawing and the table.
    #[test]
    fn the_written_side_follows_the_direction() {
        let mut s = state();
        assert_eq!(written_side(&s), ("U_Power", "power.snxsch"));
        s.direction = SheetSyncDirection::SymbolToSheet;
        assert_eq!(written_side(&s), ("power.snxsch", "U_Power"));
    }

    #[test]
    fn every_direction_and_preview_state_builds() {
        let (mut app, _task) = Signex::new();
        for direction in [
            SheetSyncDirection::SheetToSymbol,
            SheetSyncDirection::SymbolToSheet,
        ] {
            for mode in [SheetSyncPreviewMode::After, SheetSyncPreviewMode::Before] {
                for expanded in [false, true] {
                    let mut s = state();
                    s.direction = direction;
                    s.preview_mode = mode;
                    s.preview_expanded = expanded;
                    app.ui_state.sheet_sync = Some(Box::new(s));
                    let _ = app.view_sheet_sync_dialog();
                }
            }
        }
    }
}
