//! Step 2's drawn preview (#3171, mockup row 211): the Parts | Report
//! items switch, the part table (part number, symbol, footprint and two
//! alternates), and the selected part with its symbol, footprint,
//! manufacturer, suppliers and parameters, drawn by the Components panel's
//! preview renderers.

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Column, Row, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, RenderTranslatable, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, segmented, text, text_input};

use super::{ImportDialogContext, SP, TS, caption, chip, mono};
use crate::library::editor::footprint::canvas::PreviewFootprint;
use crate::library::editor::kit;
use crate::library::import_dialog::parts::{
    MAX_PART_ROWS, PartRead, PartView, PartsState, PreviewPart, PreviewTab,
};
use crate::library::import_dialog::{ImportDialogMsg, ImportDialogState, msg};
use crate::library::messages::LibraryMessage;
use crate::library::modal_chrome::{key_value_box, notice_banner};
use crate::panels::components_panel::details::ComponentDetails;
use crate::panels::components_panel::details::footprint_canvas::FootprintPreview;
use crate::panels::components_panel::details::symbol_canvas::symbol_preview;
use crate::styles::{HAIRLINE, PillTone, Radii, ti, tokens};

/// Height of the part table box.
const TABLE_H: f32 = 180.0;
const ROW_H: f32 = 26.0;
/// Height of the symbol and footprint tiles.
const TILE_H: f32 = 150.0;

/// Parts | Report items.
pub(super) fn tabs<'a>(
    current: PreviewTab,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let style = tokens::resolve(tokens).segmented(TS.body);
    segmented(
        [
            (
                PreviewTab::Parts,
                t!(i18n, ids::LIBRARY_IMPORT_DIALOG_TAB_PARTS).into(),
            ),
            (
                PreviewTab::Report,
                t!(i18n, ids::LIBRARY_IMPORT_DIALOG_TAB_ITEMS).into(),
            ),
        ],
        current,
        style,
        |tab| msg(ImportDialogMsg::PreviewTabPicked(tab)),
    )
}

/// The Parts tab.
pub(super) fn body<'a>(
    state: &'a ImportDialogState,
    ctx: ImportDialogContext<'a>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    match &state.parts {
        None | Some(PartsState::Loading) => {
            caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PARTS_READING), tokens)
        }
        Some(PartsState::Failed(why)) => {
            let words = t!(
                i18n,
                ids::LIBRARY_IMPORT_DIALOG_PARTS_FAILED,
                "error" => why.as_str()
            );
            notice_banner(
                PillTone::Err,
                text(words).size(TS.body).boxed(),
                Vec::new(),
                tokens,
            )
        }
        Some(PartsState::Ready(_)) => column![
            filter(state, i18n),
            table(state, tokens, i18n),
            selected(state, ctx, tokens, i18n),
        ]
        .spacing(SP.m)
        .boxed(),
    }
}

fn filter<'a>(state: &'a ImportDialogState, i18n: &I18n) -> Element<'a, LibraryMessage> {
    text_input(
        t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PARTS_FILTER),
        &state.part_filter,
    )
    .style(crate::panels::form_look::text_input_style)
    .on_input(|v| msg(ImportDialogMsg::PartFilterEdited(v)))
    .size(TS.body)
    .padding(SP.s)
    .width(Length::Fill)
    .boxed()
}

/// One table line: the part number, symbol and footprint columns and the
/// alternates, `cells` in that order.
fn cells_row<'a>(cells: Vec<Element<'a, LibraryMessage>>) -> Row<Element<'a, LibraryMessage>> {
    let mut line = row![].spacing(SP.m).align_y(Alignment::Center);
    for (i, cell) in cells.into_iter().enumerate() {
        let portion = if i == 0 { 3 } else { 2 };
        line = line.push(container(cell).width(Length::FillPortion(portion)).boxed());
    }
    line
}

/// A cell's value in the mono face, or a muted dash when there is none.
fn cell<'a>(value: Option<&str>, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, LibraryMessage> {
    match value {
        Some(v) => mono(v.to_string(), tokens),
        None => caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_NONE_MARK), tokens),
    }
}

fn table<'a>(
    state: &'a ImportDialogState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let all = state.preview_parts().len();
    if all == 0 {
        return caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PARTS_EMPTY), tokens);
    }
    let visible = state.visible_parts();
    if visible.is_empty() {
        return caption(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PARTS_NONE_MATCH),
            tokens,
        );
    }
    let alternates = state.alternate_columns();
    let mut head = vec![
        caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_COL_PART), tokens),
        caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_COL_SYMBOL), tokens),
        caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_COL_FOOTPRINT), tokens),
    ];
    for n in 1..=alternates {
        head.push(caption(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_COL_ALTERNATE, "n" => n),
            tokens,
        ));
    }
    let selected = state.part_view.as_ref().map(|v| v.index);
    let rows = visible.iter().take(MAX_PART_ROWS).map(|&(index, part)| {
        part_row(
            index,
            part,
            alternates,
            selected == Some(index),
            tokens,
            i18n,
        )
    });
    let line = ti(tokens.border);
    let list = container(
        column![
            container(cells_row(head)).padding([SP.xs, SP.s]),
            scrollable(column(rows).width(Length::Fill))
                .height(Length::Fixed(TABLE_H))
                .width(Length::Fill),
        ]
        .width(Length::Fill),
    )
    .padding(SP.xs)
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: HAIRLINE,
            color: line,
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    let words = t!(
        i18n,
        ids::LIBRARY_IMPORT_DIALOG_PARTS_CAPTION,
        "shown" => visible.len().min(MAX_PART_ROWS),
        "count" => all
    );
    column![list, caption(words, tokens)].spacing(SP.xs).boxed()
}

fn part_row<'a>(
    index: usize,
    part: &'a PreviewPart,
    alternates: usize,
    selected: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mut cells = vec![
        mono(part.part_number.clone(), tokens),
        cell(part.symbol.as_deref(), tokens, i18n),
        cell(part.footprint.as_deref(), tokens, i18n),
    ];
    for n in 0..alternates {
        cells.push(cell(
            part.alternates.get(n).map(String::as_str),
            tokens,
            i18n,
        ));
    }
    let r = tokens::resolve(tokens);
    let (ink, hover, picked) = (
        ti(tokens.text),
        tokens::hover_overlay(tokens),
        r.accent_soft,
    );
    button(cells_row(cells))
        .height(Length::Fixed(ROW_H))
        .width(Length::Fill)
        .padding([0.0, SP.s])
        .on_press(msg(ImportDialogMsg::PartSelected(index)))
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            let fill = if selected {
                Some(picked)
            } else {
                hovered.then_some(hover)
            };
            iced::widget::button::Style {
                background: fill.map(Background::Color),
                text_color: ink,
                border: Border {
                    radius: Radii::GRAPHITE.small.into(),
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            }
        })
        .boxed()
}

/// The selected part: its drawing, footprints, manufacturer, suppliers
/// and parameters; a hint while none is selected.
fn selected<'a>(
    state: &'a ImportDialogState,
    ctx: ImportDialogContext<'a>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let Some((view, part)) = state.selected_part() else {
        return caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PART_HINT), tokens);
    };
    match &view.read {
        PartRead::Loading => caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PART_READING), tokens),
        PartRead::Failed(why) => caption(
            t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PART_FAILED, "error" => why.as_str()),
            tokens,
        ),
        PartRead::Ready(drawing) => drawn_part(&drawing.0, view, part, ctx, tokens, i18n),
    }
}

fn drawn_part<'a>(
    details: &'a ComponentDetails,
    view: &PartView,
    part: &'a PreviewPart,
    ctx: ImportDialogContext<'a>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let footprint = details.footprint_preview(view.footprint.as_ref());
    let tiles = row![
        symbol_tile(details, ctx, tokens, i18n),
        footprint_tile(footprint, tokens, i18n),
    ]
    .spacing(SP.m);
    let mut col: Column<Element<'a, LibraryMessage>> = column![tiles].spacing(SP.s);
    let choices = details.footprint_choices(i18n);
    if choices.len() > 1 {
        let shown = details
            .shown_footprint(view.footprint.as_ref(), i18n)
            .map(|c| c.reference);
        let mut line = row![].spacing(SP.s).align_y(Alignment::Center);
        for choice in choices {
            line = line.push(chip(
                UserText::new(choice.label),
                shown == Some(choice.reference),
                PillTone::Accent,
                msg(ImportDialogMsg::PartFootprintPicked(choice.reference)),
                tokens,
            ));
        }
        col = col.push(line.boxed());
    }
    col = col.push(facts(details, part, tokens, i18n));
    for note in &details.notes {
        col = col.push(caption(UserText::new(note.clone()), tokens));
    }
    col.boxed()
}

fn symbol_tile<'a>(
    details: &'a ComponentDetails,
    ctx: ImportDialogContext<'a>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let tag = Some(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_COL_SYMBOL));
    let content = match &details.symbol {
        Some(symbol) => symbol_preview(
            symbol.clone(),
            ctx.look.canvas_colors,
            ctx.look.grid,
            TILE_H - 2.0 * HAIRLINE,
            ctx.look.device_px_per_logical_px,
        )
        .map(|()| LibraryMessage::Noop)
        .boxed(),
        None => caption(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PART_NO_SYMBOL), tokens),
    };
    container(kit::preview_tile(content, TILE_H, None, tag, tokens))
        .width(Length::FillPortion(1))
        .boxed()
}

fn footprint_tile<'a>(
    footprint: Option<&'a PreviewFootprint>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let tag = Some(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_COL_FOOTPRINT));
    let (content, ground) = match footprint {
        Some(model) => (
            iced::widget::canvas(FootprintPreview::new(model).with_theme_tokens(tokens))
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
            Some(crate::library::editor::footprint::graphite_look_for(tokens).bg),
        ),
        None => (
            caption(
                t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PART_NO_FOOTPRINT),
                tokens,
            ),
            None,
        ),
    };
    container(kit::preview_tile(content, TILE_H, ground, tag, tokens))
        .width(Length::FillPortion(1))
        .boxed()
}

/// Manufacturer, suppliers and parameters, as key and value lines.
fn facts<'a>(
    details: &'a ComponentDetails,
    part: &'a PreviewPart,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let lines = |values: Vec<String>, empty: Localized| -> Element<'a, LibraryMessage> {
        if values.is_empty() {
            return caption(empty, tokens);
        }
        let mut col: Column<Element<'a, LibraryMessage>> = column![].spacing(SP.xs);
        for value in values {
            col = col.push(
                text(UserText::new(value))
                    .size(TS.body)
                    .wrapping(Wrapping::None)
                    .ellipsis(Ellipsis::End)
                    .boxed(),
            );
        }
        col.boxed()
    };
    let none = || t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PART_NONE_LISTED);
    let maker = [&details.manufacturer, &details.mpn]
        .iter()
        .any(|s| !s.trim().is_empty())
        .then(|| {
            t!(
                i18n,
                ids::LIBRARY_IMPORT_DIALOG_PART_PAIR,
                "first" => details.manufacturer.as_str(),
                "second" => details.mpn.as_str()
            )
            .into_string()
        });
    let suppliers = part
        .suppliers
        .iter()
        .map(|s| {
            t!(
                i18n,
                ids::LIBRARY_IMPORT_DIALOG_PART_PAIR,
                "first" => s.distributor.as_str(),
                "second" => s.sku.as_str()
            )
            .into_string()
        })
        .collect();
    let parameters = details
        .parameters
        .iter()
        .map(|(key, value)| {
            t!(
                i18n,
                ids::LIBRARY_IMPORT_DIALOG_DETAIL_LINE,
                "key" => key.as_str(),
                "value" => value.as_str()
            )
            .into_string()
        })
        .collect();
    key_value_box(
        vec![
            (
                t!(i18n, ids::LIBRARY_IMPORT_DIALOG_COL_PART),
                mono(details.internal_pn.clone(), tokens),
            ),
            (
                t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PART_MANUFACTURER),
                lines(maker.into_iter().collect(), none()),
            ),
            (
                t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PART_SUPPLIERS),
                lines(suppliers, none()),
            ),
            (
                t!(i18n, ids::LIBRARY_IMPORT_DIALOG_PART_PARAMETERS),
                lines(parameters, none()),
            ),
        ],
        tokens,
    )
}

/// The footer caption for a staged preview.
pub(super) fn staged_note(i18n: &I18n) -> Localized {
    t!(i18n, ids::LIBRARY_IMPORT_DIALOG_STAGED_NOTE)
}

/// Why Import last did not move the preview, as a banner.
pub(super) fn move_problem<'a>(
    state: &'a ImportDialogState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Option<Element<'a, LibraryMessage>> {
    let problem = state.move_problem.as_ref()?;
    let words = text(problem.localized(i18n)).size(TS.body).boxed();
    Some(notice_banner(PillTone::Warn, words, Vec::new(), tokens))
}
