//! v0.26 — Right-click canvas context menu for the footprint editor.
//!
//! Mirrors Altium's PCB Library Editor right-click conventions:
//!  - On bare canvas: Place ▸ / View ▸ / Selection ▸ / Properties /
//!    Find Similar.
//!  - On a pad: Properties / Pad Actions ▸ stub / Cut / Copy / Delete /
//!    Find Similar.
//!  - On a silk graphic: Properties / Cut / Copy / Delete / Find Similar.
//!
//! Mounting site: the layer-site loop at `app/view/mod.rs::collect_overlays`
//! pushes `view_context_menu(...)` immediately after the active bar
//! overlay so the dismiss layer occludes everything else.
//!
//! Coords: `editor.state.context_menu.x/y` are window-absolute screen
//! coords (computed in `canvas.rs::ButtonReleased(Right)` from
//! `bounds.x + cursor.x`). The Translate widget at the call site
//! positions the card at exactly those pixels.

use iced::widget::{button, column, container, row, text};
use iced::{Background, Border, Element, Length, Padding};

use signex_types::theme::ThemeTokens;

use crate::app::FootprintEditorState;
use crate::keymap::{AppCommandId, CompiledKeymap};
use crate::library::editor::footprint::state::{
    FootprintContextAction, FootprintContextSubmenu, FootprintContextTarget,
};
use crate::library::messages::{LibraryMessage, PrimitiveEditorMsg};
use crate::styles::ti;

/// v0.26-C — surface the silk graphic''s kind in the menu header so
/// the user can tell at a glance what they''re about to delete /
/// inspect. Mirrors Altium''s naming.
fn silk_kind_label(kind: &signex_library::FpGraphicKind) -> &'static str {
    use signex_library::FpGraphicKind;
    match kind {
        FpGraphicKind::Line { .. } => "Track",
        FpGraphicKind::Rectangle { .. } => "Rectangle",
        FpGraphicKind::Circle { .. } => "Circle",
        FpGraphicKind::Arc { .. } => "Arc",
        FpGraphicKind::Text { .. } => "String",
        FpGraphicKind::Polygon { .. } => "Region",
    }
}

const MENU_WIDTH: f32 = 220.0;
const ROW_PAD_X: u16 = 12;
const ROW_PAD_Y: u16 = 5;
const FONT_SIZE: f32 = 11.0;

/// Build the right-click context menu card for the active footprint
/// editor. Returns `None` when the menu is closed.
pub fn view_context_menu<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
    path: &'a std::path::Path,
    has_clipboard: bool,
    keymap: &'a CompiledKeymap,
) -> Option<Element<'a, LibraryMessage>> {
    let menu_state = editor.state.context_menu.as_ref()?;

    let path_owned = path.to_path_buf();
    let make_msg = move |msg: PrimitiveEditorMsg| LibraryMessage::PrimitiveEditorEvent {
        path: path_owned.clone(),
        msg,
    };

    let mut items: Vec<Element<'a, LibraryMessage>> = Vec::new();
    let select_all_shortcut = shortcut_label(keymap, "select_all", "Ctrl+A");
    let unselect_all_shortcut = shortcut_label(keymap, "unselect_all", "Ctrl+Shift+A");
    let cut_shortcut = shortcut_label(keymap, "cut", "Ctrl+X");
    let copy_shortcut = shortcut_label(keymap, "copy", "Ctrl+C");
    let paste_shortcut = shortcut_label(keymap, "paste", "Ctrl+V");

    match menu_state.target {
        FootprintContextTarget::Empty => {
            items.push(item_disabled(tokens, "Find Similar Objects...", ""));
            items.push(separator(tokens));

            // Place ▸ — switches the Pads-mode tool. The five most-
            // used Place targets; rest of the Pads palette is still
            // reachable via the active bar.
            let place_open =
                menu_state.submenu == Some(FootprintContextSubmenu::Place);
            items.push(item_submenu_header(
                tokens,
                "Place",
                place_open,
                make_msg(PrimitiveEditorMsg::FootprintContextMenuOpenSubmenu(Some(
                    FootprintContextSubmenu::Place,
                ))),
            ));
            if place_open {
                use crate::library::editor::footprint::state::PadsTool;
                items.push(item_indented(
                    tokens,
                    "Pad",
                    "P",
                    make_msg(PrimitiveEditorMsg::FootprintSetPadsTool(PadsTool::PlacePad)),
                ));
                items.push(item_indented(
                    tokens,
                    "Track",
                    "T",
                    make_msg(PrimitiveEditorMsg::FootprintSetPadsTool(PadsTool::PlaceTrack)),
                ));
                items.push(item_indented(
                    tokens,
                    "Arc",
                    "A",
                    make_msg(PrimitiveEditorMsg::FootprintSetPadsTool(PadsTool::PlaceArc)),
                ));
                items.push(item_indented(
                    tokens,
                    "Polygon (Region)",
                    "R",
                    make_msg(PrimitiveEditorMsg::FootprintSetPadsTool(PadsTool::PlacePolygon)),
                ));
                items.push(item_indented(
                    tokens,
                    "String (Text)",
                    "S",
                    make_msg(PrimitiveEditorMsg::FootprintSetPadsTool(PadsTool::PlaceString)),
                ));
            }

            let sel_open =
                menu_state.submenu == Some(FootprintContextSubmenu::Selection);
            items.push(item_submenu_header(
                tokens,
                "Selection",
                sel_open,
                make_msg(PrimitiveEditorMsg::FootprintContextMenuOpenSubmenu(Some(
                    FootprintContextSubmenu::Selection,
                ))),
            ));
            if sel_open {
                items.push(item_indented(
                    tokens,
                    "Select All",
                    &select_all_shortcut,
                    make_msg(PrimitiveEditorMsg::FootprintContextMenuAction(
                        FootprintContextAction::SelectAllPads,
                    )),
                ));
                items.push(item_indented(
                    tokens,
                    "Deselect All",
                    &unselect_all_shortcut,
                    make_msg(PrimitiveEditorMsg::FootprintContextMenuAction(
                        FootprintContextAction::DeselectAll,
                    )),
                ));
            }

            let view_open =
                menu_state.submenu == Some(FootprintContextSubmenu::View);
            items.push(item_submenu_header(
                tokens,
                "View",
                view_open,
                make_msg(PrimitiveEditorMsg::FootprintContextMenuOpenSubmenu(Some(
                    FootprintContextSubmenu::View,
                ))),
            ));
            if view_open {
                items.push(item_indented(
                    tokens,
                    "Fit to Window",
                    "V, F",
                    make_msg(PrimitiveEditorMsg::FootprintContextMenuAction(
                        FootprintContextAction::FitToWindow,
                    )),
                ));
            }

            // v0.26-E — Paste from clipboard. Visible (active) only
            // when there''s a pad on the clipboard. Pastes at cursor
            // — the canvas''s last-known cursor world position; falls
            // back to a 1 mm offset of the original when cursor is
            // unknown.
            if has_clipboard {
                items.push(item_msg(
                    tokens,
                    "Paste",
                    &paste_shortcut,
                    make_msg(PrimitiveEditorMsg::FootprintPastePad),
                ));
                items.push(separator(tokens));
            }

            items.push(item_msg(
                tokens,
                "Properties...",
                "",
                make_msg(PrimitiveEditorMsg::FootprintCloseContextMenu),
            ));
        }

        FootprintContextTarget::Pad(idx) => {
            // Header row showing which pad the menu acts on. Greyed
            // and unclickable so it reads as a label, not an action.
            let header = editor
                .state
                .pads
                .get(idx)
                .map(|p| format!("Pad {}", p.number))
                .unwrap_or_else(|| format!("Pad {idx}"));
            items.push(item_disabled(tokens, &header, ""));
            items.push(separator(tokens));

            items.push(item_msg(
                tokens,
                "Properties...",
                "",
                make_msg(PrimitiveEditorMsg::FootprintCloseContextMenu),
            ));

            // v0.26-G — Pad Actions ▸ now expands to the items we
            // can wire today (Rotate 90°, Flip Layer). Altium''s
            // Custom Pad / Thermal Connection ops remain stubs until
            // those subsystems land.
            let pad_actions_open =
                menu_state.submenu == Some(FootprintContextSubmenu::PadActions);
            items.push(item_submenu_header(
                tokens,
                "Pad Actions",
                pad_actions_open,
                make_msg(PrimitiveEditorMsg::FootprintContextMenuOpenSubmenu(Some(
                    FootprintContextSubmenu::PadActions,
                ))),
            ));
            if pad_actions_open {
                items.push(item_indented(
                    tokens,
                    "Rotate 90°",
                    "Space",
                    make_msg(PrimitiveEditorMsg::FootprintActiveBarRotateSelection),
                ));
                items.push(item_indented(
                    tokens,
                    "Flip Layer",
                    "X",
                    make_msg(PrimitiveEditorMsg::FootprintActiveBarFlipSelection),
                ));
                // Stubs — Custom Pad + Thermal subsystems pending.
                items.push(item_indented(
                    tokens,
                    "Custom Pad from Outline...",
                    "",
                    make_msg(PrimitiveEditorMsg::FootprintCloseContextMenu),
                ));
                items.push(item_indented(
                    tokens,
                    "Thermal Connection Points...",
                    "",
                    make_msg(PrimitiveEditorMsg::FootprintCloseContextMenu),
                ));
            }

            items.push(separator(tokens));

            // v0.26-E — wired clipboard ops on the selected pad.
            items.push(item_msg(
                tokens,
                "Cut",
                &cut_shortcut,
                make_msg(PrimitiveEditorMsg::FootprintCutPad),
            ));
            items.push(item_msg(
                tokens,
                "Copy",
                &copy_shortcut,
                make_msg(PrimitiveEditorMsg::FootprintCopyPad),
            ));
            if has_clipboard {
                items.push(item_msg(
                    tokens,
                    "Paste",
                    &paste_shortcut,
                    make_msg(PrimitiveEditorMsg::FootprintPastePad),
                ));
            }
            items.push(item_msg(
                tokens,
                "Delete",
                "Del",
                make_msg(PrimitiveEditorMsg::FootprintDeleteSelected),
            ));

            items.push(separator(tokens));

            items.push(item_disabled(tokens, "Find Similar Objects...", ""));
        }

        FootprintContextTarget::SilkF(idx) => {
            // v0.26-C — silk graphic header carries the kind so the
            // user can tell at a glance what they''re inspecting /
            // deleting. Falls back to the bare index when the
            // primitive''s silk_f vec doesn''t contain the expected
            // entry (e.g. concurrent mutation between hit-test +
            // render).
            let header = match editor.primitive().silk_f.get(idx) {
                Some(g) => format!("{} (silk)", silk_kind_label(&g.kind)),
                None => format!("Silk graphic {idx}"),
            };
            items.push(item_disabled(tokens, &header, ""));
            items.push(separator(tokens));

            items.push(item_disabled(tokens, "Properties...", ""));
            // SilkF dispatches via FootprintDeleteSilkF which reads
            // `state.selected_silk_f` — the show-context-menu handler
            // set that when target == SilkF(idx) so this fires the
            // right delete.
            items.push(item_msg(
                tokens,
                "Delete",
                "Del",
                make_msg(PrimitiveEditorMsg::FootprintDeleteSilkF),
            ));
            items.push(separator(tokens));
            items.push(item_disabled(tokens, "Find Similar Objects...", ""));
        }
    }

    let card_bg = ti(tokens.panel_bg);
    let border_c = ti(tokens.border);
    let card = container(column(items).spacing(0))
        .padding(Padding::from([4u16, 0u16]))
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(card_bg)),
            border: Border {
                color: border_c,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        })
        .width(MENU_WIDTH);

    Some(card.into())
}

fn shortcut_label(keymap: &CompiledKeymap, command_id: &str, fallback: &str) -> String {
    AppCommandId::new(command_id)
        .ok()
        .and_then(|command| keymap.shortcut_label(&command))
        .unwrap_or_else(|| fallback.to_string())
}

// ── helpers ─────────────────────────────────────────────────────────

fn item_msg<'a>(
    tokens: &'a ThemeTokens,
    label: &str,
    shortcut: &str,
    message: LibraryMessage,
) -> Element<'a, LibraryMessage> {
    let text_c = ti(tokens.text);
    let secondary = ti(tokens.text_secondary);
    let hover_c = ti(tokens.hover);
    button(
        row![
            text(label.to_string()).size(FONT_SIZE).color(text_c),
            iced::widget::Space::new().width(Length::Fill),
            text(shortcut.to_string()).size(10).color(secondary),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill),
    )
    .width(MENU_WIDTH)
    .padding([ROW_PAD_Y, ROW_PAD_X])
    .on_press(message)
    .style(move |_: &iced::Theme, status: iced::widget::button::Status| {
        let bg = match status {
            iced::widget::button::Status::Hovered => {
                Some(Background::Color(hover_c))
            }
            _ => None,
        };
        iced::widget::button::Style {
            background: bg,
            border: Border::default(),
            text_color: text_c,
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

fn item_indented<'a>(
    tokens: &'a ThemeTokens,
    label: &str,
    shortcut: &str,
    message: LibraryMessage,
) -> Element<'a, LibraryMessage> {
    let text_c = ti(tokens.text);
    let secondary = ti(tokens.text_secondary);
    let hover_c = ti(tokens.hover);
    button(
        row![
            iced::widget::Space::new().width(16),
            text(label.to_string()).size(FONT_SIZE).color(text_c),
            iced::widget::Space::new().width(Length::Fill),
            text(shortcut.to_string()).size(10).color(secondary),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill),
    )
    .width(MENU_WIDTH)
    .padding([ROW_PAD_Y, ROW_PAD_X])
    .on_press(message)
    .style(move |_: &iced::Theme, status: iced::widget::button::Status| {
        let bg = match status {
            iced::widget::button::Status::Hovered => {
                Some(Background::Color(hover_c))
            }
            _ => None,
        };
        iced::widget::button::Style {
            background: bg,
            border: Border::default(),
            text_color: text_c,
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

fn item_disabled<'a>(
    tokens: &'a ThemeTokens,
    label: &str,
    shortcut: &str,
) -> Element<'a, LibraryMessage> {
    let muted = ti(tokens.text_secondary);
    container(
        row![
            text(label.to_string()).size(FONT_SIZE).color(muted),
            iced::widget::Space::new().width(Length::Fill),
            text(shortcut.to_string()).size(10).color(muted),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill),
    )
    .width(MENU_WIDTH)
    .padding([ROW_PAD_Y, ROW_PAD_X])
    .into()
}

fn item_submenu_header<'a>(
    tokens: &'a ThemeTokens,
    label: &str,
    expanded: bool,
    message: LibraryMessage,
) -> Element<'a, LibraryMessage> {
    let text_c = ti(tokens.text);
    let hover_c = ti(tokens.hover);
    let chev = if expanded { "▾" } else { "▸" };
    button(
        row![
            text(label.to_string()).size(FONT_SIZE).color(text_c),
            iced::widget::Space::new().width(Length::Fill),
            text(chev.to_string()).size(11).color(text_c),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill),
    )
    .width(MENU_WIDTH)
    .padding([ROW_PAD_Y, ROW_PAD_X])
    .on_press(message)
    .style(move |_: &iced::Theme, status: iced::widget::button::Status| {
        let bg = match status {
            iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed => {
                Some(Background::Color(hover_c))
            }
            _ => None,
        };
        iced::widget::button::Style {
            background: bg,
            border: Border::default(),
            text_color: text_c,
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}

fn separator<'a>(tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    let c = ti(tokens.border);
    container(iced::widget::Space::new().height(1))
        .padding(Padding::from([4u16, 0u16]))
        .width(MENU_WIDTH)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(c)),
            ..container::Style::default()
        })
        .into()
}
