//! Details section of the Components panel: the selected row's header,
//! Symbol and Footprint previews and Parameters table. View-only; the data
//! is read off-thread by [`load::load_details`] and held in
//! `ComponentsPanelState::details`, keyed by the row it was read for.

pub(crate) mod footprint_canvas;
pub mod load;

use iced::widget::{Column, Space, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use signex_library::primitive::Footprint;
use signex_types::schematic::LibSymbol;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::{PANEL_TEXT_SIZE, ROW_TEXT_SIZE, SECTION_HEADER_TEXT_SIZE, SP};
use crate::library::messages::LibraryMessage;
use crate::library::state::{ComponentsPanelRowKey, ComponentsPanelState};

/// Height of the Symbol and Footprint preview boxes.
const PREVIEW_HEIGHT: f32 = ROW_TEXT_SIZE * 12.0;

/// What the details section shows for one row.
#[derive(Debug, Clone)]
pub struct ComponentDetails {
    pub internal_pn: String,
    pub class: String,
    pub lifecycle: String,
    pub manufacturer: String,
    pub mpn: String,
    pub description: String,
    /// The row binds a footprint (it may still have failed to read).
    pub has_footprint_ref: bool,
    pub symbol: Option<LibSymbol>,
    pub footprint: Option<Footprint>,
    /// `(name, value)` of every row parameter.
    pub parameters: Vec<(String, String)>,
    /// Why the symbol or footprint is missing, one line each.
    pub notes: Vec<String>,
}

/// The selected row's details or the read filling them.
#[derive(Debug, Clone, Default)]
pub struct DetailsState {
    /// The row the data below was requested for.
    pub key: Option<ComponentsPanelRowKey>,
    pub loaded: Option<Box<ComponentDetails>>,
    pub error: Option<String>,
    /// The row's `(updated, content hash)` when the read started; a later
    /// difference means the row was saved and the details are stale.
    pub stamp: Option<RowStamp>,
}

/// What identifies one saved state of a row.
pub type RowStamp = (chrono::DateTime<chrono::Utc>, [u8; 32]);

impl DetailsState {
    /// A read for the shown row is still running.
    pub fn in_flight(&self) -> bool {
        self.key.is_some() && self.loaded.is_none() && self.error.is_none()
    }

    /// Start waiting for `key`: whatever was shown before is dropped.
    pub fn begin(&mut self, key: ComponentsPanelRowKey, stamp: Option<RowStamp>) {
        *self = Self {
            key: Some(key),
            loaded: None,
            error: None,
            stamp,
        };
    }

    /// Store a finished read. A result for a row that is no longer the
    /// awaited one is dropped; returns whether it was kept.
    pub fn finish(
        &mut self,
        key: &ComponentsPanelRowKey,
        result: Result<Box<ComponentDetails>, String>,
    ) -> bool {
        if self.key.as_ref() != Some(key) {
            return false;
        }
        match result {
            Ok(details) => self.loaded = Some(details),
            Err(why) => self.error = Some(why),
        }
        true
    }
}

/// The details section for the panel's selected row.
pub fn view<'a>(
    panel: &'a ComponentsPanelState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let primary = theme_ext::text_primary(tokens);

    let empty = |message: &'a str, color: Color| -> Element<'a, LibraryMessage> {
        container(text(message).size(ROW_TEXT_SIZE).color(color))
            .padding([SP.l, SP.m])
            .width(Length::Fill)
            .into()
    };
    if panel.selected.is_none() {
        return empty("Select a component to see its details.", muted);
    }
    if let Some(why) = &panel.details.error {
        return empty(why, theme_ext::error_color(tokens));
    }
    let Some(d) = panel.details.loaded.as_deref() else {
        return empty("Loading…", muted);
    };

    let mut col: Column<'a, LibraryMessage> = column![].spacing(SP.s).width(Length::Fill);
    col = col.push(
        text(d.internal_pn.as_str())
            .size(SECTION_HEADER_TEXT_SIZE)
            .color(primary),
    );
    let mut facts = vec![d.class.as_str(), d.lifecycle.as_str()];
    facts.retain(|f| !f.is_empty());
    col = col.push(text(facts.join("  ·  ")).size(ROW_TEXT_SIZE).color(muted));
    let maker = match (d.manufacturer.trim(), d.mpn.trim()) {
        ("", "") => String::new(),
        (m, "") => m.to_owned(),
        ("", p) => p.to_owned(),
        (m, p) => format!("{m} — {p}"),
    };
    if !maker.is_empty() {
        col = col.push(text(maker).size(ROW_TEXT_SIZE).color(primary));
    }
    if !d.description.trim().is_empty() {
        col = col.push(
            text(d.description.as_str())
                .size(ROW_TEXT_SIZE)
                .color(muted),
        );
    }

    col = col.push(section_title("Symbol", primary));
    col = col.push(match &d.symbol {
        Some(symbol) => preview_box(
            signex_widgets::symbol_preview::symbol_preview(symbol.clone(), PREVIEW_HEIGHT)
                // The preview canvas publishes nothing.
                .map(|()| LibraryMessage::ComponentsPanelRowMenuDismissed),
            tokens,
        ),
        None => empty("No symbol.", muted),
    });

    col = col.push(section_title("Footprint", primary));
    if let Some(fp) = &d.footprint {
        col = col.push(text(fp.name.as_str()).size(ROW_TEXT_SIZE).color(muted));
    }
    col = col.push(match &d.footprint {
        Some(fp) => {
            let program = footprint_canvas::FootprintPreview::new(
                fp.clone(),
                theme_ext::accent(tokens),
                primary,
                muted,
                primary,
            );
            let canvas = iced::widget::canvas(program)
                .width(Length::Fill)
                .height(PREVIEW_HEIGHT);
            preview_box(canvas.into(), tokens)
        }
        None if d.has_footprint_ref => empty("Footprint could not be read.", muted),
        None => empty("No footprint.", muted),
    });
    for note in &d.notes {
        col = col.push(text(note.as_str()).size(ROW_TEXT_SIZE).color(muted));
    }

    col = col.push(section_title("Parameters", primary));
    if d.parameters.is_empty() {
        col = col.push(text("No parameters.").size(ROW_TEXT_SIZE).color(muted));
    }
    for (name, value) in &d.parameters {
        col = col.push(
            row![
                text(name.as_str())
                    .size(ROW_TEXT_SIZE)
                    .color(muted)
                    .width(Length::FillPortion(2))
                    .wrapping(iced::widget::text::Wrapping::None),
                text(value.as_str())
                    .size(ROW_TEXT_SIZE)
                    .color(primary)
                    .width(Length::FillPortion(3)),
            ]
            .spacing(SP.s),
        );
    }
    col = col.push(Space::new().height(SP.m));

    scrollable(container(col).padding([SP.s, SP.m]).width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn section_title<'a>(label: &'a str, color: Color) -> Element<'a, LibraryMessage> {
    container(text(label).size(PANEL_TEXT_SIZE).color(color))
        .padding([SP.s, 0.0])
        .into()
}

/// A bordered panel-coloured box around a preview.
fn preview_box<'a>(
    inner: Element<'a, LibraryMessage>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let bg = crate::styles::ti(tokens.panel_bg);
    let border_c = theme_ext::border_color(tokens);
    container(inner)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: SP.xs.into(),
                color: border_c,
            },
            ..container::Style::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u128) -> ComponentsPanelRowKey {
        ComponentsPanelRowKey {
            library_path: "lib.snxlib".into(),
            table: "t".into(),
            row_id: signex_library::RowId::from_uuid(uuid::Uuid::from_u128(n)),
        }
    }

    #[test]
    fn a_result_for_a_row_no_longer_awaited_is_dropped() {
        let mut state = DetailsState::default();
        state.begin(key(1), None);
        state.begin(key(2), None);
        assert!(!state.finish(&key(1), Err("late".into())));
        assert!(state.error.is_none());
        assert!(state.finish(&key(2), Err("now".into())));
        assert_eq!(state.error.as_deref(), Some("now"));
    }
}
