//! The General tab of the component editor (mockup row 128): three columns,
//! Symbol and Footprint (1fr each) and the PIN TO PAD table (1.25fr), with the
//! where-used line under the two drawings. A pad cell opens the existing
//! pin-map override editor under its row; the Pin Map tab keeps the full
//! table with its own actions.

use iced::widget::{Column, Space, Widget, column, container, row, scrollable};
use iced::{Element, Length, Theme};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::{CanvasColors, ThemeTokens};
use signex_widgets::{button, text, theme_ext};

use crate::panels::components_panel::details::symbol_canvas::SheetGrid;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::{ComponentPreviewState, EditorAddress, LibraryState};
use super::kit::{self, SP, TS};
use super::preview::{PinLine, editor_event, pin_lines, pin_map_counts};
use crate::styles::PillTone;

/// Height of the Symbol and Footprint tiles.
const TILE_HEIGHT: f32 = 200.0;

/// Width portions of the two drawing columns together (1fr + 1fr) and of
/// the pin-to-pad column (1.25fr): 8 : 5.
const DRAWINGS_PORTION: u16 = 8;
const PIN_TO_PAD_PORTION: u16 = 5;

/// What a tile shows when it has no drawing.
pub(super) fn pane_message(kind: PaneKind, unbound: bool) -> signex_i18n::MessageId {
    match (kind, unbound) {
        (PaneKind::Symbol, true) => ids::EDITORS_COMPONENT_NO_SYMBOL_BOUND,
        (PaneKind::Footprint, true) => ids::EDITORS_COMPONENT_NO_FOOTPRINT_BOUND,
        (PaneKind::Symbol, false) => ids::EDITORS_COMPONENT_SYMBOL_NOT_LOADED,
        (PaneKind::Footprint, false) => ids::EDITORS_COMPONENT_FOOTPRINT_NOT_LOADED,
    }
}

#[derive(Clone, Copy)]
pub(super) enum PaneKind {
    Symbol,
    Footprint,
}

/// The file name of `path`, for a tile caption.
fn file_name(path: Option<std::path::PathBuf>) -> String {
    path.and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

#[expect(
    clippy::too_many_arguments,
    reason = "the editor views take the language handle beside their existing seven inputs (#2225)"
)]
pub(crate) fn view<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    canvas_colors: &CanvasColors,
    grid: SheetGrid,
    device_px_per_logical_px: f32,
    address: EditorAddress,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let read_only = library_state.is_read_only(&state.library_path);
    let symbol = symbol_column(
        state,
        library_state,
        tokens,
        i18n,
        (canvas_colors, grid, device_px_per_logical_px),
        &address,
        read_only,
    );
    let footprint = footprint_column(state, library_state, tokens, i18n, &address, read_only);
    let drawings = column![
        row![
            container(symbol).width(Length::FillPortion(1)).boxed(),
            container(footprint).width(Length::FillPortion(1)).boxed(),
        ]
        .spacing(SP.l)
        .boxed(),
        where_used_line(state, library_state, tokens, i18n),
    ]
    .spacing(SP.l)
    .width(Length::FillPortion(DRAWINGS_PORTION));
    let pins = container(pin_to_pad_column(state, tokens, i18n, &address, read_only))
        .width(Length::FillPortion(PIN_TO_PAD_PORTION))
        .height(Length::Fill);
    row![drawings.boxed(), pins.boxed()]
        .spacing(SP.l)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

/// A column's caption in small capitals with its buttons at the right.
fn column_head<'a>(
    caption: Localized,
    buttons: Vec<Element<'a, LibraryMessage>>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let mut line: iced::widget::Row<Element<'a, LibraryMessage>> = row![
        text(caption)
            .size(TS.caption)
            .color(crate::styles::tokens::resolve(tokens).ink4)
            .width(Length::Fill)
            .boxed()
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    for b in buttons {
        line = line.push(b);
    }
    line.boxed()
}

fn mono_caption<'a>(caption: Localized, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    text(caption)
        .size(TS.caption)
        .font(crate::fonts::iced_font_for_family(
            crate::fonts::CHROME_MONO_FONT,
        ))
        .color(crate::styles::tokens::resolve(tokens).ink4)
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

fn symbol_column<'a>(
    state: &'a ComponentPreviewState,
    library_state: &LibraryState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    (canvas_colors, grid, device_px): (&CanvasColors, SheetGrid, f32),
    address: &EditorAddress,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let unbound = state.row.is_symbol_unbound();
    let path = (!unbound)
        .then(|| super::preview::symbol_path(state, library_state))
        .flatten();
    // Binding writes the row, so Change… is off on a read-only library;
    // Open stays live (a stock symbol opens the Copy to Library flow).
    let change = (!read_only).then(|| LibraryMessage::OpenPrimitivePicker {
        kind: signex_library::PrimitiveKind::Symbol,
        target: super::super::state::PrimitivePickerTarget::PreviewRow(address.clone()),
    });
    let mut buttons = Vec::new();
    if let Some(path) = path.clone() {
        buttons.push(kit::small_action(
            t!(i18n, ids::EDITORS_COMPONENT_OPEN_PRIMITIVE),
            Some(LibraryMessage::OpenPrimitiveEditor { path }),
            tokens,
        ));
    }
    buttons.push(kit::small_action(
        t!(i18n, ids::EDITORS_COMPONENT_CHANGE),
        change,
        tokens,
    ));
    let loaded = (!unbound).then_some(state.symbol.as_ref()).flatten();
    let drawing: Element<'a, LibraryMessage> = match loaded {
        Some(sym) => crate::panels::components_panel::details::symbol_canvas::symbol_preview(
            sym.to_lib_symbol(sym.name.clone()),
            *canvas_colors,
            grid,
            TILE_HEIGHT - 2.0 * crate::styles::HAIRLINE,
            device_px,
        )
        .map(|()| LibraryMessage::Noop)
        .boxed(),
        None => empty_tile_text(
            t!(i18n, pane_message(PaneKind::Symbol, unbound)),
            unbound,
            tokens,
        ),
    };
    let mut col: Column<Element<'a, LibraryMessage>> = column![
        column_head(
            t!(i18n, ids::EDITORS_COMPONENT_SYMBOL_HEADING),
            buttons,
            tokens
        ),
        kit::preview_tile(drawing, TILE_HEIGHT, None, None, tokens),
    ]
    .spacing(SP.s);
    if let Some(sym) = loaded {
        let file = file_name(path);
        let caption = if sym.part_count > 1 {
            t!(
                i18n,
                ids::EDITORS_COMPONENT_SYMBOL_PART_CAPTION,
                "file" => file,
                "symbol" => sym.name.as_str(),
                "part" => 1,
                "parts" => usize::from(sym.part_count)
            )
        } else {
            t!(
                i18n,
                ids::EDITORS_COMPONENT_SYMBOL_CAPTION,
                "file" => file,
                "symbol" => sym.name.as_str()
            )
        };
        col = col.push(mono_caption(caption, tokens));
    }
    col.boxed()
}

fn footprint_column<'a>(
    state: &'a ComponentPreviewState,
    library_state: &LibraryState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    address: &EditorAddress,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let unbound = state.row.is_footprint_unbound();
    let path = (!unbound)
        .then(|| super::preview::footprint_path(state, library_state))
        .flatten();
    let change = (!read_only).then(|| LibraryMessage::OpenPrimitivePicker {
        kind: signex_library::PrimitiveKind::Footprint,
        target: super::super::state::PrimitivePickerTarget::PreviewRow(address.clone()),
    });
    let mut buttons = Vec::new();
    if let Some(path) = path.clone() {
        buttons.push(kit::small_action(
            t!(i18n, ids::EDITORS_COMPONENT_OPEN_PRIMITIVE),
            Some(LibraryMessage::OpenPrimitiveEditor { path }),
            tokens,
        ));
    }
    buttons.push(kit::small_action(
        t!(i18n, ids::EDITORS_COMPONENT_CHANGE),
        change,
        tokens,
    ));
    // The board palette and ground, as the PCB canvas paints them.
    let ground = crate::library::editor::footprint::graphite_look_for(tokens).bg;
    let model = (!unbound).then_some(state.footprint_preview()).flatten();
    let drawing: Element<'a, LibraryMessage> = match model {
        Some(model) => {
            let program =
                crate::panels::components_panel::details::footprint_canvas::FootprintPreview::new(
                    model,
                )
                .with_theme_tokens(tokens);
            iced::widget::canvas(program)
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed()
        }
        None => empty_tile_text(
            t!(i18n, pane_message(PaneKind::Footprint, unbound)),
            unbound,
            tokens,
        ),
    };
    let mut col: Column<Element<'a, LibraryMessage>> = column![
        column_head(
            t!(i18n, ids::EDITORS_COMPONENT_FOOTPRINT_HEADING),
            buttons,
            tokens
        ),
        kit::preview_tile(drawing, TILE_HEIGHT, Some(ground), None, tokens),
    ]
    .spacing(SP.s);
    if let Some(fp) = state.footprint.as_ref().filter(|_| !unbound) {
        col = col.push(mono_caption(
            t!(
                i18n,
                ids::EDITORS_COMPONENT_FOOTPRINT_CAPTION,
                "file" => file_name(path),
                "footprint" => fp.name.as_str()
            ),
            tokens,
        ));
    }
    col.boxed()
}

/// The text a tile shows without a drawing: the warning ink for an unbound
/// side, the muted ink for one that could not be loaded.
fn empty_tile_text<'a>(
    message: Localized,
    unbound: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let ink = if unbound {
        theme_ext::warning_color(tokens)
    } else {
        theme_ext::text_secondary(tokens)
    };
    container(text(message).size(TS.body).color(ink))
        .padding(SP.l)
        .boxed()
}

fn where_used_line<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let row_id = signex_library::RowId::from_uuid(state.row.row_id);
    let uses = library_state.where_used_for(row_id);
    let projects = uses
        .iter()
        .map(|u| &u.project_path)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    text(super::preview::where_used_label(i18n, uses.len(), projects))
        .size(TS.caption)
        .color(crate::styles::tokens::resolve(tokens).ink4)
        .boxed()
}

/// The PIN TO PAD column: caption with the matched pill, the boxed table
/// (scrolling inside the column) and the override hint.
fn pin_to_pad_column<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    address: &EditorAddress,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let summary = pin_map_counts(state).map(|(mapped, total)| {
        kit::toned_pill(
            t!(
                i18n,
                ids::EDITORS_COMPONENT_PIN_TO_PAD_MATCHED,
                "mapped" => mapped,
                "total" => total
            )
            .to_string(),
            if mapped == total {
                PillTone::Ok
            } else {
                PillTone::Warn
            },
            tokens,
        )
    });
    let head = column_head(
        t!(i18n, ids::EDITORS_COMPONENT_PIN_TO_PAD_HEADING),
        summary.into_iter().collect(),
        tokens,
    );
    let Some(sym) = state.symbol.as_ref() else {
        return column![
            head,
            kit::empty_state(
                t!(i18n, ids::EDITORS_COMPONENT_PIN_MAP_PENDING),
                None,
                tokens
            ),
        ]
        .spacing(SP.s)
        .boxed();
    };
    if sym.pins.is_empty() {
        return column![
            head,
            kit::empty_state(
                t!(i18n, ids::EDITORS_COMPONENT_PIN_MAP_NO_PINS),
                None,
                tokens
            ),
        ]
        .spacing(SP.s)
        .boxed();
    }
    let lines = pin_lines(
        sym.pins
            .iter()
            .map(|p| (p.number.as_str(), p.name.as_str())),
        &state.row.pin_map_overrides,
    );
    let hint_pin = state
        .pin_map_state
        .expanded_row
        .clone()
        .or_else(|| lines.first().map(|l| l.number.clone()))
        .unwrap_or_default();
    let widths = pin_widths();
    let mut table: Column<Element<'a, LibraryMessage>> = column![kit::table_header(
        &[
            (Some(ids::EDITORS_COMPONENT_COLUMN_PIN), widths[0]),
            (Some(ids::EDITORS_COMPONENT_COLUMN_NAME), widths[1]),
            (Some(ids::EDITORS_COMPONENT_COLUMN_TYPE), widths[2]),
            (Some(ids::EDITORS_COMPONENT_COLUMN_PAD), widths[3]),
            (Some(ids::EDITORS_COMPONENT_COLUMN_MATCH), widths[4]),
        ],
        i18n,
        tokens,
    )]
    .width(Length::Fill);
    for (index, line) in lines.iter().enumerate() {
        let direction = sym
            .pins
            .iter()
            .find(|p| p.number == line.number)
            .map(|p| super::pin_labels::direction_message(p.electrical));
        table = table.push(pin_row(
            state, line, direction, index, tokens, i18n, address, read_only,
        ));
        if state.pin_map_state.expanded_row.as_deref() == Some(line.number.as_str()) {
            table = table.push(super::preview::override_editor_row(
                &line.number,
                &state.pin_map_state.override_buf,
                tokens,
                i18n,
                address,
            ));
        }
    }
    let line_c = theme_ext::border_color(tokens);
    let boxed = container(
        scrollable(table)
            .direction(crate::styles::thin_vertical())
            .style(crate::styles::scrollbar_style(tokens))
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(crate::styles::HAIRLINE)
    .clip(true)
    .style(move |_: &Theme| container::Style {
        border: iced::Border {
            width: crate::styles::HAIRLINE,
            radius: crate::styles::Radii::GRAPHITE.button.into(),
            color: line_c,
            ..iced::Border::default()
        },
        ..container::Style::default()
    });
    column![
        head,
        boxed.boxed(),
        text(t!(i18n, ids::EDITORS_COMPONENT_PIN_TO_PAD_HINT, "pin" => hint_pin))
            .size(TS.caption)
            .color(crate::styles::tokens::resolve(tokens).ink4)
            .boxed(),
    ]
    .spacing(SP.s)
    .height(Length::Fill)
    .boxed()
}

/// Widths of Pin | Name | Type | Pad | Match.
fn pin_widths() -> [Length; 5] {
    [
        Length::FillPortion(2),
        Length::FillPortion(3),
        Length::FillPortion(4),
        Length::FillPortion(2),
        Length::FillPortion(3),
    ]
}

#[expect(
    clippy::too_many_arguments,
    reason = "one table row over the editor, the pin line and the theme"
)]
fn pin_row<'a>(
    state: &'a ComponentPreviewState,
    line: &PinLine,
    direction: Option<signex_i18n::MessageId>,
    index: usize,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    address: &EditorAddress,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let widths = pin_widths();
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let cell = |value: &str| -> Element<'a, LibraryMessage> {
        text(UserText::new(value.to_string()))
            .size(TS.body)
            .font(mono)
            .color(text_c)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed()
    };
    let kind: Element<'a, LibraryMessage> = match direction {
        Some(id) => text(t!(i18n, id))
            .size(TS.body)
            .color(muted)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed(),
        None => Space::new().boxed(),
    };
    // A pad cell opens the override editor under its row (read-only: inert).
    let hover = crate::styles::tokens::overlay(tokens, crate::styles::tokens::Overlay::Hover);
    let pad = button(cell(&line.pad))
        .padding([0.0, SP.xs])
        .on_press_maybe((!read_only).then(|| {
            editor_event(
                address,
                EditorMsg::PinMapOpenOverrideEdit(line.number.clone()),
            )
        }))
        .style(move |_: &Theme, status| iced::widget::button::Style {
            background: matches!(status, iced::widget::button::Status::Hovered)
                .then_some(iced::Background::Color(hover)),
            text_color: text_c,
            ..iced::widget::button::Style::default()
        })
        .boxed();
    let cells = vec![
        (cell(&line.number), widths[0]),
        (cell(&line.name), widths[1]),
        (kind, widths[2]),
        (pad, widths[3]),
        (match_pill(state, line, tokens, i18n), widths[4]),
    ];
    kit::table_row(cells, index, None, tokens)
}

/// How the pin found its pad: ok (a pad of its number), override, or no pad.
fn match_pill<'a>(
    state: &ComponentPreviewState,
    line: &PinLine,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let (id, tone) = if line.is_override {
        (ids::EDITORS_COMPONENT_MATCH_OVERRIDE, PillTone::Info)
    } else if state
        .footprint
        .as_ref()
        .is_some_and(|f| f.pads.iter().any(|p| p.number == line.pad))
    {
        (ids::EDITORS_COMPONENT_MATCH_OK, PillTone::Ok)
    } else {
        (ids::EDITORS_COMPONENT_MATCH_NO_PAD, PillTone::Warn)
    };
    kit::toned_pill(t!(i18n, id).to_string(), tone, tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_message_is_plain_language() {
        let english = |kind, unbound| {
            I18n::en_us()
                .message(pane_message(kind, unbound), &[])
                .to_string()
        };
        assert_eq!(english(PaneKind::Symbol, true), "No symbol bound");
        let missing = english(PaneKind::Footprint, false);
        assert_eq!(
            missing,
            "The footprint could not be loaded. It may be missing from the open libraries."
        );
        assert!(!missing.contains("uuid"));
    }

    /// Mockup row 128: the three columns keep the mock's 1fr / 1fr / 1.25fr widths.
    #[test]
    fn the_columns_follow_one_one_and_a_quarter() {
        let ratio = f32::from(PIN_TO_PAD_PORTION) / (f32::from(DRAWINGS_PORTION) / 2.0);
        assert!((ratio - 1.25).abs() < f32::EPSILON);
    }

    fn texts(state: &ComponentPreviewState) -> Vec<String> {
        laid_out(state, 980.0)
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    /// Mockup row 128 (decided 2026-11-22): at the narrowest docked width
    /// the columns shrink and PIN TO PAD stays inside the tab.
    #[test]
    fn pin_to_pad_stays_visible_at_the_narrowest_width() {
        let row = crate::library::component_preview::test_row();
        let state = ComponentPreviewState::from_row("/tmp/test.snxlib".into(), "t".into(), row);
        let width = super::super::MIN_EDITOR_W;
        let texts = laid_out(&state, width);
        let heading = crate::layout_probe::find(&texts, "PIN TO PAD");
        assert!(
            heading.bounds.x + heading.bounds.width <= width,
            "{:?} in {width}",
            heading.bounds
        );
    }

    fn laid_out(
        state: &ComponentPreviewState,
        width: f32,
    ) -> Vec<crate::layout_probe::LaidOutText> {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark);
        let library = LibraryState::default();
        let address = EditorAddress::new(
            state.library_path.clone(),
            state.table.clone(),
            signex_library::RowId::from(state.row.row_id),
        );
        let canvas = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::GraphiteDark);
        crate::layout_probe::lay_out_texts(
            view(
                state,
                &library,
                &tokens,
                &canvas,
                SheetGrid {
                    visible: true,
                    pitch_mm: 1.27,
                    style: crate::render_config::GridStyle::Dots,
                },
                1.0,
                address,
                &I18n::en_us(),
            ),
            iced::Size::new(width, 480.0),
        )
    }

    /// The General tab shows the three column captions and Change… for both
    /// drawings; an unbound side says so in its tile.
    #[test]
    fn general_shows_symbol_footprint_and_pin_to_pad() {
        let row = crate::library::component_preview::test_row();
        let state = ComponentPreviewState::from_row("/tmp/test.snxlib".into(), "t".into(), row);
        let texts = texts(&state);
        for want in ["SYMBOL", "FOOTPRINT", "PIN TO PAD"] {
            assert!(texts.iter().any(|t| t == want), "{want:?} in {texts:?}");
        }
        assert_eq!(
            texts.iter().filter(|t| t.as_str() == "Change…").count(),
            2,
            "{texts:?}"
        );
    }
}
