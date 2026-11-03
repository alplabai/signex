//! Details section of the Components panel: the selected row's header,
//! Symbol and Footprint previews and Parameters table. View-only; the data
//! is read off-thread by [`load::load_details`] and held in
//! `ComponentsPanelState::details`, keyed by the row it was read for.

pub(crate) mod footprint_canvas;
pub mod load;
pub(crate) mod symbol_canvas;

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
    /// Counts the reads started; a result carrying an older number is
    /// dropped, so a slow read never overwrites a newer one.
    pub generation: u64,
    /// A read is running. `loaded` keeps showing meanwhile.
    pub reloading: bool,
}

/// What identifies one saved state of a row.
pub type RowStamp = (chrono::DateTime<chrono::Utc>, [u8; 32]);

impl DetailsState {
    /// A read for the shown row is still running.
    pub fn in_flight(&self) -> bool {
        self.reloading
    }

    /// The shown data is current for `stamp` and needs no read.
    pub fn is_fresh(&self, key: &ComponentsPanelRowKey, stamp: Option<RowStamp>) -> bool {
        self.key.as_ref() == Some(key)
            && self.loaded.is_some()
            && self.error.is_none()
            && self.stamp == stamp
    }

    /// Start a read of `key` and return its generation. Another row's data
    /// is dropped; the same row's stays shown until the result lands.
    pub fn begin(&mut self, key: ComponentsPanelRowKey, stamp: Option<RowStamp>) -> u64 {
        if self.key.as_ref() != Some(&key) {
            self.loaded = None;
        }
        self.key = Some(key);
        self.error = None;
        self.stamp = stamp;
        self.reloading = true;
        self.generation += 1;
        self.generation
    }

    /// Store a finished read. A result for another row or an older read is
    /// dropped; returns whether it was kept. A failed read keeps the data
    /// already shown and adds the error.
    pub fn finish(
        &mut self,
        key: &ComponentsPanelRowKey,
        generation: u64,
        result: Result<Box<ComponentDetails>, String>,
    ) -> bool {
        if self.key.as_ref() != Some(key) || self.generation != generation {
            return false;
        }
        self.reloading = false;
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
    canvas_colors: &signex_types::theme::CanvasColors,
    device_px_per_logical_px: f32,
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
    let Some(d) = panel.details.loaded.as_deref() else {
        return match &panel.details.error {
            Some(why) => empty(why, theme_ext::error_color(tokens)),
            None => empty("Loading…", muted),
        };
    };

    let mut col: Column<'a, LibraryMessage> = column![].spacing(SP.s).width(Length::Fill);
    if let Some(why) = &panel.details.error {
        col = col.push(
            text(why.as_str())
                .size(ROW_TEXT_SIZE)
                .color(theme_ext::error_color(tokens)),
        );
    }
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
            symbol_canvas::symbol_preview(
                symbol.clone(),
                *canvas_colors,
                PREVIEW_HEIGHT,
                device_px_per_logical_px,
            )
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
            let program = footprint_canvas::FootprintPreview::new(fp.clone(), tokens);
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

/// Border width of the preview box, px.
const PREVIEW_BORDER_WIDTH: f32 = 1.0;

/// A bordered panel-coloured box around a preview.
fn preview_box<'a>(
    inner: Element<'a, LibraryMessage>,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let bg = crate::styles::ti(tokens.panel_bg);
    let border_c = theme_ext::border_color(tokens);
    container(inner)
        .width(Length::Fill)
        // Inset by the border so a preview's own fill leaves the stroke and
        // the rounded corners visible.
        .padding(PREVIEW_BORDER_WIDTH)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: PREVIEW_BORDER_WIDTH,
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
        let first = state.begin(key(1), None);
        let second = state.begin(key(2), None);
        assert!(!state.finish(&key(1), first, Err("late".into())));
        assert!(state.error.is_none());
        assert!(state.finish(&key(2), second, Err("now".into())));
        assert_eq!(state.error.as_deref(), Some("now"));
    }

    #[test]
    fn an_older_read_of_the_same_row_is_dropped() {
        let mut state = DetailsState::default();
        let a1 = state.begin(key(1), None);
        let _b = state.begin(key(2), None);
        let a2 = state.begin(key(1), None);
        assert!(!state.finish(&key(1), a1, Err("old".into())));
        assert!(state.finish(&key(1), a2, Err("new".into())));
        assert_eq!(state.error.as_deref(), Some("new"));
    }
}
