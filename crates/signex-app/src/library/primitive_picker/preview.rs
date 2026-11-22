//! The primitive picker's preview column (mockup row 131): the highlighted
//! primitive's drawing and, for a symbol, its pin and part counts.

use iced::widget::{Column, column, container};
use iced::{Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, ids, t};
use signex_library::{PrimitiveKind, PrimitiveRef};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::{SP, TS};
use crate::library::browser::PreviewLook;
use crate::library::editor::footprint::canvas::PreviewFootprint;
use crate::library::editor::kit;
use crate::library::messages::LibraryMessage;
use crate::library::state::{PickerSelection, PrimitiveChoice, PrimitiveDrawing};
use crate::panels::components_panel::details::footprint_canvas::FootprintPreview;
use crate::panels::components_panel::details::load::MountedAdapters;
use crate::panels::components_panel::details::symbol_canvas::symbol_preview;

/// Width of the column (mockup row 131).
pub(super) const PREVIEW_W: f32 = 200.0;
/// Height of the drawing's tile.
const TILE_H: f32 = PREVIEW_W * 0.6;

/// Read `choice`'s drawing. Runs off the UI thread. `Ok(None)` for a kind
/// with no drawing (a sim model).
pub(crate) fn read_drawing(
    kind: PrimitiveKind,
    choice: &PrimitiveChoice,
    libraries: &MountedAdapters,
) -> Result<Option<PrimitiveDrawing>, String> {
    match (kind, choice) {
        (PrimitiveKind::Symbol, PrimitiveChoice::Pick(reference)) => {
            let symbol = owner(libraries, reference)?
                .get_symbol(reference.uuid)
                .map_err(|e| e.to_string())?;
            Ok(Some(symbol_drawing(&symbol, reference)))
        }
        (PrimitiveKind::Symbol, PrimitiveChoice::Loose(loose)) => {
            let symbol = crate::library::loose_symbols::read_loose_symbol(&loose.file, loose.uuid)?;
            let reference = PrimitiveRef::new(uuid::Uuid::nil(), loose.uuid);
            Ok(Some(symbol_drawing(&symbol, &reference)))
        }
        (PrimitiveKind::Footprint, PrimitiveChoice::Pick(reference)) => {
            let footprint = owner(libraries, reference)?
                .get_footprint(reference.uuid)
                .map_err(|e| e.to_string())?;
            Ok(Some(PrimitiveDrawing::Footprint(Box::new(
                PreviewFootprint::build(&footprint),
            ))))
        }
        _ => Ok(None),
    }
}

fn owner<'a>(
    libraries: &'a MountedAdapters,
    reference: &PrimitiveRef,
) -> Result<&'a dyn signex_library::LibraryAdapter, String> {
    libraries
        .iter()
        .find(|(id, _)| *id == reference.library_id)
        .map(|(_, adapter)| adapter.as_ref())
        .ok_or_else(|| format!("library {} is not open", reference.library_id))
}

fn symbol_drawing(symbol: &signex_library::Symbol, reference: &PrimitiveRef) -> PrimitiveDrawing {
    PrimitiveDrawing::Symbol {
        symbol: Box::new(symbol.to_lib_symbol(crate::library::place::lib_id_of(reference))),
        pins: symbol.pins.len(),
        parts: symbol.part_count,
    }
}

/// A muted line in the tile, in place of a drawing.
fn note<'a>(message: Localized, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    text(message)
        .size(TS.caption)
        .color(crate::styles::tokens::resolve(tokens).ink4)
        .align_x(iced::alignment::Horizontal::Center)
        .boxed()
}

/// The tile's content and, for a footprint, the board ground under it.
fn tile_content<'a>(
    selection: &'a PickerSelection,
    look: PreviewLook,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> (Element<'a, LibraryMessage>, Option<iced::Color>) {
    match selection.drawing.as_deref() {
        Some(PrimitiveDrawing::Symbol { symbol, .. }) => (
            symbol_preview(
                (**symbol).clone(),
                look.canvas_colors,
                look.grid,
                TILE_H - 2.0 * crate::styles::HAIRLINE,
                look.device_px_per_logical_px,
            )
            .map(|()| LibraryMessage::Noop)
            .boxed(),
            None,
        ),
        Some(PrimitiveDrawing::Footprint(model)) => (
            iced::widget::canvas(FootprintPreview::new(model).with_theme_tokens(tokens))
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
            Some(crate::library::editor::footprint::graphite_look_for(tokens).bg),
        ),
        None if selection.loading => (
            note(t!(i18n, ids::LIBRARY_BROWSER_PREVIEW_LOADING), tokens),
            None,
        ),
        None => (
            note(t!(i18n, ids::LIBRARY_BROWSER_PREVIEW_UNREADABLE), tokens),
            None,
        ),
    }
}

/// The column. `shown` is false while nothing is highlighted or the filter
/// hides the highlighted row.
pub(super) fn pane<'a>(
    selection: &'a PickerSelection,
    shown: bool,
    look: PreviewLook,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let mut body: Column<Element<'a, LibraryMessage>> = column![].spacing(SP.s).width(Length::Fill);
    if shown {
        let (content, ground) = tile_content(selection, look, tokens, i18n);
        body = body.push(kit::preview_tile(content, TILE_H, ground, None, tokens));
        if let Some(PrimitiveDrawing::Symbol { pins, parts, .. }) = selection.drawing.as_deref() {
            body = body.push(
                text(t!(
                    i18n,
                    ids::LIBRARY_PRIMITIVE_PICKER_PREVIEW_FACTS,
                    "pins" => *pins,
                    "parts" => usize::from(*parts)
                ))
                .size(TS.caption)
                .color(r.ink4)
                .boxed(),
            );
        }
    } else {
        body = body.push(note(
            t!(i18n, ids::LIBRARY_PRIMITIVE_PICKER_PREVIEW_EMPTY),
            tokens,
        ));
    }
    let rule = r.line_soft;
    container(body)
        .padding(SP.m)
        .width(Length::Fixed(PREVIEW_W))
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border::default().left(iced::border::Side::default().width(1.0).color(rule)),
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn shown_texts(selection: &PickerSelection, shown: bool) -> Vec<String> {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteDark);
        let i18n = I18n::en_us_ref();
        crate::layout_probe::lay_out_texts(
            pane(selection, shown, PreviewLook::for_tests(), &tokens, i18n),
            iced::Size::new(PREVIEW_W, 300.0),
        )
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
    }

    /// Mockup row 131: a symbol reads "2 pins · 1 part" under its drawing.
    #[test]
    fn a_symbol_preview_counts_its_pins_and_parts() {
        let symbol = signex_library::Symbol::empty("R").to_lib_symbol("x");
        let selection = PickerSelection {
            drawing: Some(Arc::new(PrimitiveDrawing::Symbol {
                symbol: Box::new(symbol),
                pins: 2,
                parts: 1,
            })),
            ..PickerSelection::default()
        };
        let shown = shown_texts(&selection, true);
        assert!(shown.iter().any(|t| t == "2 pins · 1 part"), "{shown:#?}");
    }

    #[test]
    fn with_no_row_highlighted_the_column_says_what_to_do() {
        let shown = shown_texts(&PickerSelection::default(), false);
        assert!(
            shown.iter().any(|t| t == "Click a row to preview it."),
            "{shown:#?}"
        );
    }
}
