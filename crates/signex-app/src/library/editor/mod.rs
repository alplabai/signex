//! Component editor — a library row opened as a document (a main-window
//! tab or its own `WindowKind::ComponentEditor` window), Graphite mockup
//! row 128.
//!
//! In the DBLib model, components are TSV rows addressed by
//! [`crate::library::state::EditorAddress`] (`library_path + table +
//! row_id`). The state lives on
//! [`crate::library::state::ComponentPreviewState`].
//!
//! Layout: the header (`header.rs`: internal PN, version, unsaved and parse
//! pills, library / table, close), underline tabs with counts in the order
//! General, Parameters, Supply, Datasheet, Simulation, Pin Map, Alternates,
//! the active tab's body, and the footer (`footer.rs`: Reload saved
//! version, Ctrl+S, Release…, Save & Close, Cancel, Save). Symbol and
//! Footprint draw read-only; Open on either opens the standalone
//! `.snxsym` / `.snxfpt` editor.

pub mod alternate_symbols;
pub mod datasheet_picker;
mod footer;
pub mod footprint;
mod general;
mod header;
pub(crate) mod kit;
mod measurement_cell;
pub mod params;
pub(crate) mod pin_labels;
pub mod preview;
pub mod sim;
pub mod standalone;
pub mod supply;
pub mod symbol;
pub mod text_extent;

use iced::widget::{Widget, column, container, row};
use iced::{Color, Element, Length};
use signex_i18n::{I18n, t};
use signex_library::TemplateViolation;
use signex_types::theme::{CanvasColors, ThemeTokens};
use signex_widgets::inert;
use signex_widgets::text;
use signex_widgets::theme_ext;

use crate::panels::components_panel::details::symbol_canvas::SheetGrid;

use self::kit::{SP, TS};
use super::messages::{EditorMsg, LibraryMessage};
use super::state::{ComponentPreviewState, EditorAddress, LibraryState, PreviewTab};

/// Fade over a read-only tab: the panel colour, so it dims on any theme.
const READ_ONLY_VEIL_ALPHA: f32 = 0.55;

/// Narrowest the editor lays out: the mock's 980 px window. A narrower
/// window scrolls the editor sideways inside its frame.
const MIN_EDITOR_W: f32 = 980.0;

/// The widget id of a parameter's field, so the header's parse pill can
/// focus it. Keyed by the row too: two editors can be open.
pub(crate) fn unparsed_field_id(address: &EditorAddress, name: &str) -> iced::widget::Id {
    iced::widget::Id::from(format!(
        "component-editor-param:{}:{}:{}:{name}",
        address.library_path.display(),
        address.table,
        address.row_id
    ))
}

/// Render the component editor: header, tabs, the active tab and the
/// footer, kept at least [`MIN_EDITOR_W`] wide.
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
    i18n: &'a I18n,
) -> Element<'a, LibraryMessage> {
    let canvas_colors = *canvas_colors;
    kit::sideways_frame(MIN_EDITOR_W, tokens, move || {
        let read_only = library_state.is_read_only(&state.library_path);
        let header = header::view(state, tokens, i18n, address.clone());
        let tabs = view_tabs(state, library_state, tokens, i18n, address.clone());
        let body = view_active_tab(
            state,
            library_state,
            tokens,
            &canvas_colors,
            grid,
            device_px_per_logical_px,
            address.clone(),
            i18n,
        );
        let footer = footer::view(state, tokens, i18n, &address, read_only);
        column![header, tabs, body, footer]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed()
    })
}

/// How many required slots of `class` have no usable value. Zero for a
/// blank class or one with no template.
fn missing_required_in(
    registry: &signex_library::TemplateRegistry,
    library_id: uuid::Uuid,
    class: &str,
    params: &signex_library::ParamMap,
) -> usize {
    if class.trim().is_empty() {
        return 0;
    }
    registry
        .validate_params(library_id, class, params)
        .iter()
        .filter(|v| matches!(v, TemplateViolation::MissingRequired { .. }))
        .count()
}

/// The missing-required count the Parameters tab shows; `None` draws no
/// error badge. Hidden at 0 and on a read-only library, where the tab is
/// inert and the user cannot act on it.
fn tab_badge(
    tab: PreviewTab,
    state: &ComponentPreviewState,
    library_state: &LibraryState,
) -> Option<usize> {
    match tab {
        PreviewTab::Parameters => {
            if library_state.is_read_only(&state.library_path) {
                return None;
            }
            let library_id = params::library_id_of(state, library_state)?;
            let n = missing_required_in(
                &library_state.template_registry,
                library_id,
                state.row.class.as_str(),
                &state.row.parameters,
            );
            Some(n).filter(|n| *n > 0)
        }
        _ => None,
    }
}

/// The count a tab shows: Parameters, its missing required slots (error)
/// or else how many parameters the row has (neutral); Pin Map, the pins
/// with a pad of all pins (ok when complete, warning otherwise);
/// Alternates, the alternate symbols (neutral).
fn graphite_badge(
    tab: PreviewTab,
    state: &ComponentPreviewState,
    library_state: &LibraryState,
) -> Option<(String, crate::styles::PillTone)> {
    use crate::styles::PillTone;
    match tab {
        PreviewTab::Parameters => tab_badge(tab, state, library_state)
            .map(|n| (n.to_string(), PillTone::Err))
            .or_else(|| {
                Some(state.row.parameters.len())
                    .filter(|n| *n > 0)
                    .map(|n| (n.to_string(), PillTone::Neutral))
            }),
        PreviewTab::PinMap => preview::pin_map_counts(state).map(|(mapped, total)| {
            let tone = if mapped == total {
                PillTone::Ok
            } else {
                PillTone::Warn
            };
            (format!("{mapped}/{total}"), tone)
        }),
        PreviewTab::Alternates => Some(state.row.alternate_symbols.len())
            .filter(|n| *n > 0)
            .map(|n| (n.to_string(), PillTone::Neutral)),
        _ => None,
    }
}

fn view_tabs<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    // The shared `TabPill`, so these tabs read like the document and dock
    // tabs. The tabs are flat: no pill fill or outline, only the accent
    // underline on the active one.
    let (active_fill, resting_fill, border) =
        (Color::TRANSPARENT, Color::TRANSPARENT, Color::TRANSPARENT);
    let accent = crate::styles::ti(tokens.accent);
    let order = PreviewTab::GRAPHITE_ORDER;
    let last = order.len().saturating_sub(1);
    let mut row_widget: iced::widget::Row<Element<'_, LibraryMessage>> = iced::widget::Row::new()
        .spacing(0)
        .align_y(iced::Alignment::End);
    for (i, tab) in order.iter().enumerate() {
        let is_active = *tab == state.active_tab;
        let (fill, text_c) = if is_active {
            (active_fill, theme_ext::text_primary(tokens))
        } else {
            (resting_fill, theme_ext::text_secondary(tokens))
        };
        let mut content: iced::widget::Row<Element<'_, LibraryMessage>> = row![
            text(t!(i18n, tab.graphite_label()))
                .size(TS.label)
                .color(text_c)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed()
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
        if let Some((label, tone)) = graphite_badge(*tab, state, library_state) {
            content = content.push(kit::toned_pill(label, tone, tokens));
        }
        let label = container(content.boxed()).padding([SP.s, SP.l]);
        let style = signex_widgets::tab_pill::TabPillStyle {
            fill,
            border,
            accent,
            is_active,
            is_last: i == last,
            accent_position: signex_widgets::tab_pill::AccentPosition::Bottom,
        };
        let pill = signex_widgets::tab_pill::TabPill::new(label.boxed(), style);
        let tab_msg = LibraryMessage::EditorEvent {
            library_path: address.library_path.clone(),
            table: address.table.clone(),
            row_id: address.row_id,
            msg: EditorMsg::SelectTab(*tab),
        };
        row_widget = row_widget.push(
            iced::widget::mouse_area(pill.boxed())
                .on_press(tab_msg)
                .boxed(),
        );
    }
    container(row_widget)
        .padding(iced::Padding {
            top: SP.xs,
            right: SP.s,
            bottom: 0.0,
            left: SP.s,
        })
        .width(Length::Fill)
        .style(crate::styles::tab_bar_strip(tokens))
        .boxed()
}

/// The fade drawn over an inert (read-only) tab.
fn read_only_veil(tokens: &ThemeTokens) -> Color {
    Color {
        a: READ_ONLY_VEIL_ALPHA,
        ..crate::styles::ti(tokens.panel_bg)
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the editor views take the language handle beside their existing seven inputs (#2225)"
)]
fn view_active_tab<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    canvas_colors: &CanvasColors,
    grid: SheetGrid,
    device_px_per_logical_px: f32,
    address: EditorAddress,
    i18n: &'a I18n,
) -> Element<'a, LibraryMessage> {
    // A read-only library (the stock library) shows every editing tab
    // dimmed and inert; the General tab guards its own editing parts
    // because its Open buttons must stay live.
    let read_only = library_state.is_read_only(&state.library_path);
    let veil = read_only_veil(tokens);
    let inner: Element<'_, LibraryMessage> = match state.active_tab {
        PreviewTab::Preview => preview::view(
            state,
            library_state,
            tokens,
            canvas_colors,
            grid,
            device_px_per_logical_px,
            address,
            i18n,
        ),
        PreviewTab::Parameters => inert::unless_read_only_veiled(
            read_only,
            veil,
            params::view(state, library_state, tokens, i18n, address),
        ),
        PreviewTab::Supply => inert::unless_read_only_veiled(
            read_only,
            veil,
            supply::view(state, tokens, i18n, address),
        ),
        // Open stays live on a read-only library, so the tab veils its
        // own editing controls instead of being wrapped whole.
        PreviewTab::Datasheet => {
            datasheet_picker::view(state, tokens, i18n, address, read_only, veil)
        }
        PreviewTab::Simulation => {
            inert::unless_read_only_veiled(read_only, veil, sim::view(state, tokens, i18n, address))
        }
        PreviewTab::PinMap => preview::pin_map_tab(state, tokens, i18n, &address, read_only),
        PreviewTab::Alternates => inert::unless_read_only_veiled(
            read_only,
            veil,
            iced::widget::scrollable(alternate_symbols::view(state, tokens, i18n, &address))
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
        ),
    };
    // The one body padding (the mock's 16 px). A tab is a scrolling stack
    // of `kit::card` sections with no outer surface of its own.
    container(inner)
        .padding(SP.xl)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styles::tokens::{self, Overlay};
    use signex_types::theme::{ThemeId, theme_tokens};

    #[test]
    fn missing_required_count_follows_template() {
        use signex_library::{ParamMap, ParamValue, TemplateRegistry};
        let reg = TemplateRegistry::new_with_builtins();
        let id = uuid::Uuid::nil();
        let mut p = ParamMap::new();
        assert_eq!(missing_required_in(&reg, id, "unicorn", &p), 0);
        assert_eq!(missing_required_in(&reg, id, "  ", &p), 0);
        assert_eq!(missing_required_in(&reg, id, "resistor", &p), 3);
        p.insert(
            "value".into(),
            ParamValue::Measurement {
                value: 10_000.0,
                unit: "ohm".into(),
            },
        );
        assert_eq!(missing_required_in(&reg, id, "resistor", &p), 2);
    }

    #[test]
    fn resting_and_active_tab_fills_differ_on_every_theme() {
        for id in ThemeId::BUILTINS {
            let t = theme_tokens(*id);
            assert_ne!(
                crate::styles::ti(t.hover),
                tokens::overlay(&t, Overlay::Faint),
                "{id:?}"
            );
        }
    }

    #[test]
    fn read_only_veil_follows_the_panel_colour() {
        for &id in ThemeId::BUILTINS {
            let t = theme_tokens(id);
            let veil = read_only_veil(&t);
            assert_eq!(veil.a, READ_ONLY_VEIL_ALPHA);
            assert_eq!(
                Color { a: 1.0, ..veil },
                Color {
                    a: 1.0,
                    ..crate::styles::ti(t.panel_bg)
                }
            );
        }
    }

    /// Mockup row 128: the tab strip reads General, Parameters, Supply, Datasheet,
    /// Simulation, Pin Map, Alternates.
    #[test]
    fn the_tab_strip_follows_the_mock_order() {
        assert_eq!(
            PreviewTab::GRAPHITE_ORDER,
            &[
                PreviewTab::Preview,
                PreviewTab::Parameters,
                PreviewTab::Supply,
                PreviewTab::Datasheet,
                PreviewTab::Simulation,
                PreviewTab::PinMap,
                PreviewTab::Alternates,
            ]
        );
    }

    /// Two editors' fields never share a focus id.
    #[test]
    fn unparsed_field_ids_are_per_row_and_per_field() {
        let a = EditorAddress::new("/l.snxlib".into(), "t".into(), signex_library::RowId::new());
        let b = EditorAddress::new("/l.snxlib".into(), "t".into(), signex_library::RowId::new());
        assert_ne!(
            unparsed_field_id(&a, "value"),
            unparsed_field_id(&b, "value")
        );
        assert_ne!(
            unparsed_field_id(&a, "value"),
            unparsed_field_id(&a, "power")
        );
        assert_eq!(
            unparsed_field_id(&a, "value"),
            unparsed_field_id(&a, "value")
        );
    }

    fn preview_with(pn: &str, class: &str) -> ComponentPreviewState {
        use signex_library::{
            ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState,
            ManufacturerPart, ParamMap, PlmReserved, PrimitiveRef,
        };
        let row = ComponentRow {
            alternate_symbols: Vec::new(),
            row_id: uuid::Uuid::now_v7(),
            internal_pn: InternalPn::new(pn),
            class: ComponentClass::new(class),
            datasheet: DatasheetRef::default(),
            datasheet_label: String::new(),
            description: String::new(),
            more_datasheets: Vec::new(),
            state: LifecycleState::Draft,
            symbol_ref: PrimitiveRef::new(uuid::Uuid::now_v7(), uuid::Uuid::now_v7()),
            footprint_ref: None,
            alternate_footprints: Vec::new(),
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "ACM-001"),
            alternates: Vec::new(),
            supply: Vec::new(),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            version: "0.0.1".into(),
            released: false,
            symbol_version: String::new(),
            footprint_version: String::new(),
            sim_version: String::new(),
            created: chrono::Utc::now(),
            updated: chrono::Utc::now(),
            content_hash: [0u8; 32],
            server_revision: None,
        };
        ComponentPreviewState::from_row(
            std::path::PathBuf::from("/tmp/test.snxlib"),
            "t".into(),
            row,
        )
    }

    /// Header texts with Fluent's bidi isolation marks removed.
    fn plain_header_texts(state: &ComponentPreviewState) -> Vec<String> {
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let address = EditorAddress::new(
            std::path::PathBuf::from("/tmp/test.snxlib"),
            "t".into(),
            signex_library::RowId::from(state.row.row_id),
        );
        crate::layout_probe::lay_out_texts(
            header::view(state, &tokens, &signex_i18n::I18n::en_us(), address),
            iced::Size::new(1400.0, 200.0),
        )
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
    }

    /// Mockup row 128: the header shows the part number once, its version pill and the
    /// library / table under it; lifecycle, class, description and the
    /// manufacturer part are not in the header any more.
    #[test]
    fn header_shows_the_part_number_version_and_location() {
        let mut state = preview_with("RES-0001", "resistor");
        state.row.description = "Thick-film chip resistor".into();
        let texts = plain_header_texts(&state);
        for want in ["RES-0001", "v0.0.1", "test / t"] {
            assert_eq!(
                texts.iter().filter(|t| t.as_str() == want).count(),
                1,
                "{want:?} in {texts:?}"
            );
        }
        for gone in [
            "Draft",
            "resistor",
            "Thick-film chip resistor",
            "Acme ACM-001",
        ] {
            assert!(!texts.iter().any(|t| t == gone), "{gone:?} in {texts:?}");
        }
    }

    #[test]
    fn identity_falls_back_when_pn_and_class_are_empty() {
        let texts = plain_header_texts(&preview_with(" ", ""));
        assert!(texts.iter().any(|t| t == "(no part number)"), "{texts:?}");
        assert!(
            texts.iter().all(|t| !t.trim().is_empty()),
            "an empty value leaves a blank text: {texts:?}"
        );
    }

    /// A row with no description and no manufacturer part draws no empty
    /// text in the header.
    #[test]
    fn header_leaves_out_an_empty_summary() {
        let mut state = preview_with("RES-0001", "resistor");
        state.row.primary_mpn = signex_library::ManufacturerPart::draft("", " ");
        let texts = plain_header_texts(&state);
        assert!(texts.iter().all(|t| !t.trim().is_empty()), "{texts:?}");
    }

    /// The parse pill counts the pending measurement edits that did not
    /// parse, and is absent when there are none; the "• unsaved" pill
    /// follows the dirty flag.
    #[test]
    fn header_chip_counts_unparsed_fields_and_marks_unsaved() {
        let mut state = preview_with("RES-0001", "resistor");
        let clean = plain_header_texts(&state);
        assert!(!clean.iter().any(|t| t.contains("parse")), "{clean:?}");
        assert!(!clean.iter().any(|t| t.contains("unsaved")), "{clean:?}");
        state
            .params_error
            .insert("value".into(), "not a number".into());
        state.dirty = true;
        let texts = plain_header_texts(&state);
        assert!(
            texts.iter().any(|t| t == "1 field does not parse"),
            "{texts:?}"
        );
        assert!(texts.iter().any(|t| t == "• unsaved"), "{texts:?}");
        state
            .params_error
            .insert("power".into(), "not a number".into());
        let texts = plain_header_texts(&state);
        assert!(
            texts.iter().any(|t| t == "2 fields do not parse"),
            "{texts:?}"
        );
    }

    /// The Parameters tab counts the row's parameters when nothing required
    /// is missing.
    #[test]
    fn the_parameters_tab_counts_the_parameters() {
        let mut state = preview_with("RES-0001", "");
        let library = LibraryState::default();
        assert!(graphite_badge(PreviewTab::Parameters, &state, &library).is_none());
        state.row.parameters.insert(
            "tolerance".into(),
            signex_library::ParamValue::Text("1%".into()),
        );
        let (label, tone) =
            graphite_badge(PreviewTab::Parameters, &state, &library).expect("a count");
        assert_eq!(label, "1");
        assert_eq!(tone, crate::styles::PillTone::Neutral);
    }
}
