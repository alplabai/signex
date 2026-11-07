//! Details section of the Components panel: the selected row's header,
//! Symbol and Footprint previews and Parameters table. View-only; the data
//! is read off-thread by [`load::load_details`] and held in
//! `ComponentsPanelState::details`, keyed by the row it was read for.

pub(crate) mod footprint_canvas;
mod graphite;
pub mod load;
pub(crate) mod symbol_canvas;

#[cfg(test)]
#[path = "pan_app_tests.rs"]
mod pan_app_tests;

use iced::widget::{Column, Space, column, container, pick_list, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};

use crate::library::editor::footprint::canvas::PreviewFootprint;
use signex_library::primitive::PrimitiveRef;
use signex_types::schematic::LibSymbol;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::{HINT_TEXT_SIZE, PANEL_TEXT_SIZE, ROW_TEXT_SIZE, SECTION_HEADER_TEXT_SIZE, SP};
use crate::library::messages::LibraryMessage;
use crate::library::state::{ComponentsPanelRowKey, ComponentsPanelState};

/// Height of the Symbol and Footprint preview boxes.
const PREVIEW_HEIGHT: f32 = HINT_TEXT_SIZE * 12.0;

/// What the details section shows for one row.
#[derive(Debug, Clone)]
pub struct ComponentDetails {
    pub internal_pn: String,
    pub class: String,
    pub lifecycle: String,
    /// The row's lifecycle state, for the Graphite lifecycle pill.
    pub state: signex_library::LifecycleState,
    pub manufacturer: String,
    pub mpn: String,
    pub description: String,
    /// The row binds a footprint (it may still have failed to read).
    pub has_footprint_ref: bool,
    pub symbol: Option<LibSymbol>,
    /// The editor's model of the row's default footprint, built once at load.
    pub footprint: Option<PreviewFootprint>,
    /// The row's default footprint, whether or not it could be read (#1234).
    pub default_footprint: Option<PrimitiveRef>,
    /// The row's alternate footprints, read with the rest (#1234).
    pub alternate_footprints: Vec<FootprintOption>,
    /// `(name, value)` of every row parameter.
    pub parameters: Vec<(String, String)>,
    /// Why the symbol or footprint is missing, one line each.
    pub notes: Vec<String>,
}

/// One alternate footprint of the row, with its preview (`None`: it could
/// not be read; the load's notes say why).
#[derive(Debug, Clone)]
pub struct FootprintOption {
    pub reference: PrimitiveRef,
    pub name: String,
    pub preview: Option<PreviewFootprint>,
}

/// One entry of the Footprint picker: a footprint the row offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootprintChoice {
    pub reference: PrimitiveRef,
    /// The footprint's name, "(default)" appended for the row's default.
    pub label: String,
}

impl std::fmt::Display for FootprintChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

impl ComponentDetails {
    /// The footprints the row offers, the default first. Fewer than two
    /// entries means there is nothing to pick.
    pub fn footprint_choices(&self) -> Vec<FootprintChoice> {
        let Some(default) = self.default_footprint else {
            return Vec::new();
        };
        let default_name = self
            .footprint
            .as_ref()
            .map_or("Default", |fp| fp.name.as_str());
        let default = FootprintChoice {
            reference: default,
            label: format!("{default_name} (default)"),
        };
        std::iter::once(default)
            .chain(self.alternate_footprints.iter().map(|a| FootprintChoice {
                reference: a.reference,
                label: a.name.clone(),
            }))
            .collect()
    }

    /// The footprint the picker shows: `pick` when the row offers it, else
    /// the default.
    pub fn shown_footprint(&self, pick: Option<&PrimitiveRef>) -> Option<FootprintChoice> {
        let choices = self.footprint_choices();
        pick.and_then(|p| choices.iter().find(|c| c.reference == *p))
            .or_else(|| choices.first())
            .cloned()
    }

    /// The preview of the footprint `pick` names (the default when `None` or
    /// not offered), and whether that footprint could not be read.
    pub fn footprint_preview(&self, pick: Option<&PrimitiveRef>) -> Option<&PreviewFootprint> {
        let alternate =
            pick.and_then(|p| self.alternate_footprints.iter().find(|a| a.reference == *p));
        match alternate {
            Some(alternate) => alternate.preview.as_ref(),
            None => self.footprint.as_ref(),
        }
    }
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
pub(crate) fn view<'a>(
    panel: &'a ComponentsPanelState,
    tokens: &'a ThemeTokens,
    canvas_colors: &signex_types::theme::CanvasColors,
    grid: symbol_canvas::SheetGrid,
    device_px_per_logical_px: f32,
) -> Element<'a, LibraryMessage> {
    let is_graphite = crate::styles::is_graphite(tokens);
    let muted = if is_graphite {
        crate::styles::tokens::resolve(tokens).ink4
    } else {
        theme_ext::text_secondary(tokens)
    };
    let primary = theme_ext::text_primary(tokens);

    let empty = |message: &'a str, color: Color| -> Element<'a, LibraryMessage> {
        container(text(message).size(HINT_TEXT_SIZE).color(color))
            .padding(if is_graphite {
                [SP.s, 10.0]
            } else {
                [SP.l, SP.m]
            })
            .width(Length::Fill)
            .boxed()
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

    let mut col: Column<Element<'a, LibraryMessage>> = column![].spacing(SP.s).width(Length::Fill);
    if let Some(why) = &panel.details.error {
        col = col.push(
            text(why.as_str())
                .size(ROW_TEXT_SIZE)
                .color(theme_ext::error_color(tokens))
                .boxed(),
        );
    }
    let maker = match (d.manufacturer.trim(), d.mpn.trim()) {
        ("", "") => String::new(),
        (m, "") => m.to_owned(),
        ("", p) => p.to_owned(),
        (m, p) => format!("{m} — {p}"),
    };
    if is_graphite {
        col = col.push(graphite::header_card(d, maker, tokens));
    } else {
        col = legacy_header(col, d, maker, primary, muted);
    }
    let title = |label: &'a str| {
        if is_graphite {
            graphite::section_label(label, tokens)
        } else {
            section_title(label, primary)
        }
    };

    col = col.push(title("Symbol"));
    col = col.push(match &d.symbol {
        Some(symbol) => preview_box(
            symbol_canvas::symbol_preview(
                symbol.clone(),
                *canvas_colors,
                grid,
                PREVIEW_HEIGHT,
                device_px_per_logical_px,
            )
            // The preview canvas publishes nothing.
            .map(|()| LibraryMessage::ComponentsPanelRowMenuDismissed)
            .boxed(),
            tokens,
        ),
        None => empty("No symbol.", muted),
    });

    col = col.push(title("Footprint"));
    // The pick belongs to the selected row only (#1234).
    let pick = panel
        .footprint_pick
        .as_ref()
        .filter(|(key, _)| panel.selected.as_ref() == Some(key))
        .map(|(_, reference)| reference);
    let shown = d.shown_footprint(pick);
    let choices = d.footprint_choices();
    if choices.len() >= 2 {
        col = col.push(
            pick_list(shown.clone(), choices, |choice: &FootprintChoice| {
                choice.to_string()
            })
            .on_select(|choice: FootprintChoice| {
                LibraryMessage::ComponentsPanelFootprintPicked(choice.reference)
            })
            .text_size(ROW_TEXT_SIZE)
            .width(Length::Fill)
            .style(crate::panels::form_look::pick_list_style)
            .boxed(),
        );
    } else if let Some(fp) = &d.footprint {
        col = col.push(
            text(fp.name.as_str())
                .size(ROW_TEXT_SIZE)
                .color(muted)
                .boxed(),
        );
    }
    let preview = d.footprint_preview(shown.as_ref().map(|c| &c.reference));
    col = col.push(match preview {
        Some(fp) => {
            let program = footprint_canvas::FootprintPreview::new(fp).with_theme_tokens(tokens);
            let canvas = iced::widget::canvas(program)
                .width(Length::Fill)
                .height(PREVIEW_HEIGHT);
            preview_box(canvas.boxed(), tokens)
        }
        None if d.has_footprint_ref => empty("Footprint could not be read.", muted),
        None => empty("No footprint.", muted),
    });
    for note in &d.notes {
        col = col.push(
            text(note.as_str())
                .size(HINT_TEXT_SIZE)
                .color(muted)
                .boxed(),
        );
    }

    col = col.push(title("Parameters"));
    if d.parameters.is_empty() {
        col = col.push(
            text("No parameters.")
                .size(HINT_TEXT_SIZE)
                .color(muted)
                .boxed(),
        );
    }
    for (name, value) in &d.parameters {
        if is_graphite {
            col = col.push(graphite::parameter_row(name, value, tokens));
            continue;
        }
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
            .spacing(SP.s)
            .boxed(),
        );
    }
    col = col.push(Space::new().height(SP.m).boxed());

    scrollable(
        container(col)
            .padding(if is_graphite {
                [SP.s, 10.0]
            } else {
                [SP.s, SP.m]
            })
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(crate::panels::form_look::scrollbar_style)
    .boxed()
}

/// The legacy header lines: part number, class and lifecycle, maker and
/// description.
fn legacy_header<'a>(
    mut col: Column<Element<'a, LibraryMessage>>,
    d: &'a ComponentDetails,
    maker: String,
    primary: Color,
    muted: Color,
) -> Column<Element<'a, LibraryMessage>> {
    col = col.push(
        text(d.internal_pn.as_str())
            .size(SECTION_HEADER_TEXT_SIZE)
            .color(primary)
            .boxed(),
    );
    let mut facts = vec![d.class.as_str(), d.lifecycle.as_str()];
    facts.retain(|f| !f.is_empty());
    col = col.push(
        text(facts.join("  ·  "))
            .size(HINT_TEXT_SIZE)
            .color(muted)
            .boxed(),
    );
    if !maker.is_empty() {
        col = col.push(text(maker).size(ROW_TEXT_SIZE).color(primary).boxed());
    }
    if !d.description.trim().is_empty() {
        col = col.push(
            text(d.description.as_str())
                .size(HINT_TEXT_SIZE)
                .color(muted)
                .boxed(),
        );
    }
    col
}

fn section_title<'a>(label: &'a str, color: Color) -> Element<'a, LibraryMessage> {
    container(text(label).size(PANEL_TEXT_SIZE).color(color))
        .padding([SP.s, 0.0])
        .boxed()
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
    let radius = if crate::styles::is_graphite(tokens) {
        crate::styles::Radii::GRAPHITE.control
    } else {
        SP.xs
    };
    container(inner)
        .width(Length::Fill)
        // Inset by the border so a preview's own fill leaves the stroke and
        // the rounded corners visible.
        .padding(PREVIEW_BORDER_WIDTH)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: PREVIEW_BORDER_WIDTH,
                radius: radius.into(),
                color: border_c,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
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
