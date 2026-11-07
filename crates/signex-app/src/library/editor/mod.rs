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

use iced::widget::{Space, Widget, button, column, container, row, text};
use iced::{Border, Color, Element, Length, Theme};
use signex_library::TemplateViolation;
use signex_types::theme::{CanvasColors, ThemeTokens};
use signex_widgets::theme_ext;

use crate::panels::components_panel::details::symbol_canvas::SheetGrid;

use self::kit::{SP, TS};
use super::messages::{EditorMsg, LibraryMessage};
use super::state::{ComponentPreviewState, EditorAddress, LibraryState, PreviewTab};
use crate::styles::tokens::{self, Overlay};

/// Fade over a read-only tab: the panel colour, so it dims on any theme.
const READ_ONLY_VEIL_ALPHA: f32 = 0.55;
/// The close glyph is drawn this many times the heading size.
const CLOSE_GLYPH_SCALE: f32 = 1.6;

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
    let header = view_header(state, tokens, address.clone());
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

/// The row's identity, shown once in the shell header: part number, then
/// class and lifecycle state when present.
fn identity_line(pn: &str, class: &str, state: &str) -> String {
    let pn = if pn.trim().is_empty() {
        "(no part number)"
    } else {
        pn
    };
    let mut line = pn.to_string();
    if !class.trim().is_empty() {
        line.push_str(&format!("  \u{2014}  {class}"));
    }
    if !state.trim().is_empty() {
        line.push_str(&format!("  \u{2014}  {state}"));
    }
    line
}

fn view_header<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    // The row id is an internal key; the internal PN is the name a user
    // knows the part by.
    let identity = identity_line(
        state.row.internal_pn.as_str(),
        state.row.class.as_str(),
        &format!("{:?}", state.row.state),
    );
    if crate::styles::is_graphite(tokens) {
        return graphite_header(state, tokens, address);
    }
    container(
        row![
            text("Component Preview").size(TS.heading).color(muted),
            Space::new().width(SP.m),
            text(identity).size(TS.heading).color(text_c),
            Space::new().width(Length::Fill),
            close_btn(address, tokens),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.m, SP.l])
    .style(crate::styles::modal_header_strip(tokens))
    .boxed()
}

/// Graphite document header: the internal PN in mono, then the lifecycle
/// and class as pills, with the close button at the right.
fn graphite_header<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
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
    line = line
        .push(Space::new().width(Length::Fill).boxed())
        .push(close_btn(address, tokens));
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

fn view_tabs<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    // The shared `TabPill`, so these tabs read like the document and dock
    // tabs: bordered pills, the active one filled with an accent line.
    let active_fill = crate::styles::ti(tokens.hover);
    let resting_fill = tokens::overlay(tokens, Overlay::Faint);
    let border = crate::styles::ti(tokens.border);
    let accent = crate::styles::ti(tokens.accent);
    let last = PreviewTab::ORDER.len().saturating_sub(1);
    let mut row_widget: iced::widget::Row<Element<'_, LibraryMessage>> = iced::widget::Row::new()
        .spacing(0)
        .align_y(iced::Alignment::End);
    for (i, tab) in PreviewTab::ORDER.iter().enumerate() {
        let is_active = *tab == state.active_tab;
        let (fill, text_c) = if is_active {
            (active_fill, theme_ext::text_primary(tokens))
        } else {
            (resting_fill, theme_ext::text_secondary(tokens))
        };
        let mut content: iced::widget::Row<Element<'_, LibraryMessage>> = row![
            text(tab.label())
                .size(TS.label)
                .color(text_c)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed()
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
        if let Some(count) = tab_badge(*tab, state, library_state) {
            let fill = tokens::danger(tokens);
            content = content.push(if crate::styles::is_graphite(tokens) {
                kit::toned_pill(count.to_string(), crate::styles::PillTone::Err, tokens)
            } else {
                kit::pill(count.to_string(), fill, tokens::readable_on(fill))
            });
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
        row![
            kit::secondary_button("Save", event(EditorMsg::SaveDraft), tokens),
            Space::new().width(SP.m),
            kit::primary_button("Save & Close", event(EditorMsg::Commit), tokens),
            reload,
            Space::new().width(Length::Fill),
            dirty_marker,
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.m, SP.l])
    .style(crate::styles::modal_footer_strip(tokens))
    .boxed()
}

/// Fill of the close button: none at rest, token overlays on hover/press.
fn close_background(tokens: &ThemeTokens, status: button::Status) -> Option<iced::Background> {
    let step = match status {
        button::Status::Hovered => Overlay::Hover,
        button::Status::Pressed => Overlay::Pressed,
        _ => return None,
    };
    Some(tokens::overlay(tokens, step).into())
}

fn close_btn<'a>(address: EditorAddress, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    let tokens = *tokens;
    button(
        container(
            text("\u{00D7}")
                .size(TS.heading * CLOSE_GLYPH_SCALE)
                .color(text_c),
        )
        .padding([0.0, SP.m]),
    )
    .on_press(LibraryMessage::EditorEvent {
        library_path: address.library_path,
        table: address.table,
        row_id: address.row_id,
        msg: EditorMsg::CloseEditor,
    })
    .style(move |_: &Theme, status| button::Style {
        background: close_background(&tokens, status),
        border: Border {
            width: crate::styles::HAIRLINE,
            radius: crate::styles::RADIUS.into(),
            color: border,
            ..Border::default()
        },
        text_color: text_c,
        ..button::Style::default()
    })
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn identity_is_shown_once_with_class_and_state() {
        assert_eq!(
            identity_line("RES-0001", "resistor", "Draft"),
            "RES-0001  \u{2014}  resistor  \u{2014}  Draft"
        );
    }

    #[test]
    fn identity_falls_back_when_pn_and_class_are_empty() {
        assert_eq!(
            identity_line(" ", "", "Draft"),
            "(no part number)  \u{2014}  Draft"
        );
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

    #[test]
    fn close_button_fills_only_on_hover_and_press() {
        let t = theme_tokens(ThemeId::BUILTINS[0]);
        assert!(close_background(&t, button::Status::Active).is_none());
        let hover = close_background(&t, button::Status::Hovered);
        let pressed = close_background(&t, button::Status::Pressed);
        assert!(hover.is_some() && pressed.is_some());
        assert_ne!(hover, pressed);
    }
}
