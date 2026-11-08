//! Component Preview tab — read-only Symbol/Footprint render plus
//! template-validated forms for parameters / supply / datasheet /
//! simulation.
//!
//! In the v0.9-refactor-2 (DBLib) model, components are TSV rows
//! addressed by [`crate::library::state::EditorAddress`]
//! (`library_path + table + row_id`). The state lives on
//! [`crate::library::state::ComponentPreviewState`].
//!
//! The Component view is preview-only: Symbol and Footprint render
//! read-only. Editing happens via standalone `.snxsym` / `.snxfpt`
//! document tabs in the main window — right-click on either render
//! in the Preview tab fires
//! [`crate::library::messages::LibraryMessage::OpenPrimitiveEditor`]
//! to open the standalone editor.
//!
//! Tab count is 5: Preview / Parameters / Supply / Datasheet /
//! Simulation. Pin Map folds into Preview as an inline subsection;
//! History is reachable via `git log`; Where-Used is a footer line
//! on Preview.

pub mod alternate_symbols;
pub mod datasheet_picker;
pub mod footprint;
pub mod inert;
pub(crate) mod kit;
mod measurement_cell;
pub mod params;
pub mod preview;
pub mod sim;
pub mod standalone;
pub mod supply;
pub mod symbol;
pub mod text_extent;

use iced::widget::{Space, Widget, column, container, row, text};
use iced::{Color, Element, Length};
use signex_library::TemplateViolation;
use signex_types::theme::{CanvasColors, ThemeTokens};
use signex_widgets::theme_ext;

use crate::panels::components_panel::details::symbol_canvas::SheetGrid;

use self::kit::{SP, TS};
use super::messages::{EditorMsg, LibraryMessage};
use super::state::{ComponentPreviewState, EditorAddress, LibraryState, PreviewTab};

/// Fade over a read-only tab: the panel colour, so it dims on any theme.
const READ_ONLY_VEIL_ALPHA: f32 = 0.55;
/// Render a Component Preview surface — five tabs (Preview / Parameters
/// / Supply / Datasheet / Simulation), all read-only for Symbol+
/// Footprint. The active tab body fills the panel; the header strip
/// and footer give the row's identity + save controls.
pub(crate) fn view<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    canvas_colors: &CanvasColors,
    grid: SheetGrid,
    device_px_per_logical_px: f32,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let read_only = library_state.is_read_only(&state.library_path);
    let header = view_header(state, tokens, address.clone(), read_only);
    let tabs = view_tabs(state, library_state, tokens, address.clone());
    let body = view_active_tab(
        state,
        library_state,
        tokens,
        canvas_colors,
        grid,
        device_px_per_logical_px,
        address.clone(),
    );
    let footer = view_footer(state, tokens, address, read_only);

    column![header, tabs, body, footer]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

fn view_header<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    graphite_header(state, tokens, address, read_only)
}

/// Graphite document header: the internal PN in mono, then the lifecycle
/// and class as pills, with Discard / Release... / Save and the close
/// button at the right.
fn graphite_header<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let pn = state.row.internal_pn.as_str();
    let pn = if pn.trim().is_empty() {
        "(no part number)"
    } else {
        pn
    };
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let mut line: iced::widget::Row<Element<'a, LibraryMessage>> = row![
        text(pn)
            .size(TS.heading)
            .font(mono)
            .color(theme_ext::text_primary(tokens))
            .boxed(),
        kit::toned_pill(
            kit::lifecycle_label(state.row.state).to_string(),
            kit::lifecycle_tone(state.row.state),
            tokens,
        ),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    let class = state.row.class.as_str().trim();
    if !class.is_empty() {
        line = line.push(kit::toned_pill(
            class.to_string(),
            crate::styles::PillTone::Neutral,
            tokens,
        ));
    }
    let event = |msg: EditorMsg| {
        Some(LibraryMessage::EditorEvent {
            library_path: address.library_path.clone(),
            table: address.table.clone(),
            row_id: address.row_id,
            msg,
        })
    };
    line = line.push(Space::new().width(Length::Fill).boxed());
    if state.dirty {
        line = line.push(
            text("\u{2022} Unsaved changes")
                .size(TS.body)
                .color(theme_ext::text_secondary(tokens))
                .boxed(),
        );
    }
    let editable = !read_only;
    // Discard drops the working copy for the stored row (asks first when
    // dirty); Release has no backend yet, so it is a disabled stub.
    line = line
        .push(kit::secondary_button(
            "Discard",
            event(EditorMsg::ReloadRequested).filter(|_| editable && state.dirty),
            tokens,
        ))
        .push(kit::secondary_button("Release...", None, tokens))
        .push(kit::secondary_button(
            "Save & Close",
            event(EditorMsg::Commit).filter(|_| editable),
            tokens,
        ))
        .push(kit::primary_button(
            "Save",
            event(EditorMsg::SaveDraft).filter(|_| editable),
            tokens,
        ));
    // No close button: the document tab's own cross closes the editor.
    container(line.boxed())
        .padding([SP.m, SP.l])
        .style(crate::styles::modal_header_strip(tokens))
        .boxed()
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

/// The number a tab shows in its badge slot; `None` draws no badge. The
/// Parameters count is hidden at 0 and on a read-only library, where the
/// tab is inert and the user cannot act on it.
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

/// The badge of a Graphite tab: the failing-parameter count (error), the
/// pins with a pad out of all pins (ok when complete, warning otherwise),
/// the alternate-symbol count (neutral).
fn graphite_badge(
    tab: PreviewTab,
    state: &ComponentPreviewState,
    library_state: &LibraryState,
) -> Option<(String, crate::styles::PillTone)> {
    use crate::styles::PillTone;
    match tab {
        PreviewTab::Parameters => {
            tab_badge(tab, state, library_state).map(|n| (n.to_string(), PillTone::Err))
        }
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
            text(tab.graphite_label())
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

fn view_active_tab<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    canvas_colors: &CanvasColors,
    grid: SheetGrid,
    device_px_per_logical_px: f32,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    // A read-only library (the stock library) shows every editing tab
    // dimmed and inert; the Preview tab guards its own editing parts
    // because its Open Editor buttons must stay live.
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
        ),
        PreviewTab::Parameters => inert::unless_read_only_veiled(
            read_only,
            veil,
            params::view(state, library_state, tokens, address),
        ),
        PreviewTab::Supply => {
            inert::unless_read_only_veiled(read_only, veil, supply::view(state, tokens, address))
        }
        // Open stays live on a read-only library, so the tab veils its
        // own editing controls instead of being wrapped whole.
        PreviewTab::Datasheet => datasheet_picker::view(state, tokens, address, read_only, veil),
        PreviewTab::Simulation => {
            inert::unless_read_only_veiled(read_only, veil, sim::view(state, tokens, address))
        }
        PreviewTab::PinMap => preview::pin_map_tab(state, tokens, &address, read_only),
        PreviewTab::Alternates => inert::unless_read_only_veiled(
            read_only,
            veil,
            iced::widget::scrollable(alternate_symbols::view(state, tokens, &address))
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
        ),
    };
    // The one body padding. A tab is a scrolling stack of `kit::card`
    // sections with no outer surface of its own.
    container(inner)
        .padding(SP.l)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

fn view_footer<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);

    if read_only {
        return container(
            text("Read-only library - this component cannot be changed.")
                .size(TS.body)
                .color(muted),
        )
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .style(crate::styles::modal_footer_strip(tokens))
        .boxed();
    }

    // There is no bottom bar: Save / Save & Close live in the header.
    // The bar only returns for the stale-row reload prompt.
    if !state.stale_base {
        return Space::new().height(0.0).boxed();
    }

    let event = |msg: EditorMsg| {
        Some(LibraryMessage::EditorEvent {
            library_path: address.library_path.clone(),
            table: address.table.clone(),
            row_id: address.row_id,
            msg,
        })
    };
    let dirty_marker: Element<'_, LibraryMessage> = if state.dirty {
        text("\u{2022} unsaved").size(TS.body).color(muted).boxed()
    } else {
        Space::new().width(0.0).boxed()
    };

    // Offered once a save was refused because the row changed on the
    // server; discarding the working copy asks first.
    let reload: Element<'_, LibraryMessage> = if !state.stale_base {
        Space::new().width(0.0).boxed()
    } else if state.reload_confirming {
        row![
            Space::new().width(SP.l),
            text("Discard your edits and reload?")
                .size(TS.body)
                .color(muted),
            Space::new().width(SP.m),
            kit::primary_button("Reload", event(EditorMsg::ReloadConfirmed), tokens),
            Space::new().width(SP.m),
            kit::secondary_button("Keep editing", event(EditorMsg::ReloadCancelled), tokens),
        ]
        .align_y(iced::Alignment::Center)
        .boxed()
    } else {
        row![
            Space::new().width(SP.l),
            kit::secondary_button(
                "Reload saved version",
                event(EditorMsg::ReloadRequested),
                tokens
            ),
        ]
        .boxed()
    };

    container(
        row![reload, Space::new().width(Length::Fill), dirty_marker,]
            .align_y(iced::Alignment::Center),
    )
    .padding([SP.m, SP.l])
    .style(crate::styles::modal_footer_strip(tokens))
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

    fn header_texts(state: &ComponentPreviewState) -> Vec<String> {
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let address = EditorAddress::new(
            std::path::PathBuf::from("/tmp/test.snxlib"),
            "t".into(),
            signex_library::RowId::from(state.row.row_id),
        );
        crate::layout_probe::lay_out_texts(
            view_header(state, &tokens, address, false),
            iced::Size::new(1400.0, 200.0),
        )
        .into_iter()
        .map(|t| t.text)
        .collect()
    }

    /// The header shows the row's identity once: part number, lifecycle
    /// state and class.
    #[test]
    fn identity_is_shown_once_with_class_and_state() {
        let texts = header_texts(&preview_with("RES-0001", "resistor"));
        for want in ["RES-0001", "Draft", "resistor"] {
            assert_eq!(
                texts.iter().filter(|t| t.as_str() == want).count(),
                1,
                "{want:?} in {texts:?}"
            );
        }
    }

    #[test]
    fn identity_falls_back_when_pn_and_class_are_empty() {
        let texts = header_texts(&preview_with(" ", ""));
        assert!(texts.iter().any(|t| t == "(no part number)"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "Draft"), "{texts:?}");
        assert!(
            texts.iter().all(|t| !t.trim().is_empty()),
            "an empty class leaves a blank pill: {texts:?}"
        );
    }
}
