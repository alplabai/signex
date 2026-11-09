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

use iced::widget::{column, container, row};
use iced::{Background, Border, Element, Length, Padding, Widget};
use signex_widgets::{button, raw, text};

use signex_types::theme::ThemeTokens;

use signex_library::primitive::footprint::Footprint;

use crate::app::FootprintDocument;
use crate::keymap::CompiledKeymap;
use crate::library::editor::footprint::state::silk_selection::is_sketch_owned_silk;
use crate::library::editor::footprint::state::{
    FootprintContextAction, FootprintContextSubmenu, FootprintContextTarget,
};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::styles::ti;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

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

/// The message the silk menu's Delete sends; `None` (item disabled) for an
/// entry baked from the sketch, which the next bake would bring back.
fn silk_delete_msg(fp: &Footprint, idx: usize) -> Option<FootprintEditorMsg> {
    (!is_sketch_owned_silk(fp, idx)).then_some(FootprintEditorMsg::DeleteSilkF)
}

const MENU_WIDTH: f32 = 220.0;
const ROW_PAD_X: u16 = 12;
const ROW_PAD_Y: u16 = 5;
const FONT_SIZE: f32 = crate::styles::TypeScale::DEFAULT.body;

/// Build the right-click context menu card for the active footprint
/// editor. Returns `None` when the menu is closed.
pub fn view_context_menu<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    path: &'a std::path::Path,
    has_clipboard: bool,
    keymap: &'a CompiledKeymap,
) -> Option<Element<'a, LibraryMessage>> {
    let menu_state = editor.state.context_menu.as_ref()?;

    let path_owned = path.to_path_buf();
    let make_msg = move |msg: FootprintEditorMsg| LibraryMessage::PrimitiveEditorEvent {
        path: path_owned.clone(),
        msg: PrimitiveEdit::Footprint(msg),
    };

    let mut items: Vec<Element<'a, LibraryMessage>> = Vec::new();
    // Shortcut hints resolved from the active keymap profile.
    let select_all_shortcut = keymap.shortcut_label_for_id("select_all");
    let unselect_all_shortcut = keymap.shortcut_label_for_id("unselect_all");
    let cut_shortcut = keymap.shortcut_label_for_id("cut");
    let copy_shortcut = keymap.shortcut_label_for_id("copy");
    let paste_shortcut = keymap.shortcut_label_for_id("paste");

    match menu_state.target {
        FootprintContextTarget::Empty => {
            items.push(item_disabled(tokens, "Find Similar Objects...", ""));
            items.push(separator(tokens));

            // Place ▸ — switches the Pads-mode tool. The five most-
            // used Place targets; rest of the Pads palette is still
            // reachable via the active bar.
            let place_open = menu_state.submenu == Some(FootprintContextSubmenu::Place);
            items.push(item_submenu_header(
                tokens,
                "Place",
                place_open,
                make_msg(FootprintEditorMsg::ContextMenuOpenSubmenu(Some(
                    FootprintContextSubmenu::Place,
                ))),
            ));
            if place_open {
                use crate::library::editor::footprint::state::PadsTool;
                items.push(item_indented(
                    tokens,
                    "Pad",
                    "P",
                    make_msg(FootprintEditorMsg::SetPadsTool(PadsTool::PlacePad)),
                ));
                items.push(item_indented(
                    tokens,
                    "Track",
                    "T",
                    make_msg(FootprintEditorMsg::SetPadsTool(PadsTool::PlaceTrack)),
                ));
                items.push(item_indented(
                    tokens,
                    "Arc",
                    "A",
                    make_msg(FootprintEditorMsg::SetPadsTool(PadsTool::PlaceArc)),
                ));
                items.push(item_indented(
                    tokens,
                    "Polygon (Region)",
                    "R",
                    make_msg(FootprintEditorMsg::SetPadsTool(PadsTool::PlacePolygon)),
                ));
                items.push(item_indented(
                    tokens,
                    "String (Text)",
                    "S",
                    make_msg(FootprintEditorMsg::SetPadsTool(PadsTool::PlaceString)),
                ));
            }

            let sel_open = menu_state.submenu == Some(FootprintContextSubmenu::Selection);
            items.push(item_submenu_header(
                tokens,
                "Selection",
                sel_open,
                make_msg(FootprintEditorMsg::ContextMenuOpenSubmenu(Some(
                    FootprintContextSubmenu::Selection,
                ))),
            ));
            if sel_open {
                items.push(item_indented(
                    tokens,
                    "Select All",
                    &select_all_shortcut,
                    make_msg(FootprintEditorMsg::ContextMenuAction(
                        FootprintContextAction::SelectAllPads,
                    )),
                ));
                items.push(item_indented(
                    tokens,
                    "Deselect All",
                    &unselect_all_shortcut,
                    make_msg(FootprintEditorMsg::ContextMenuAction(
                        FootprintContextAction::DeselectAll,
                    )),
                ));
            }

            let view_open = menu_state.submenu == Some(FootprintContextSubmenu::View);
            items.push(item_submenu_header(
                tokens,
                "View",
                view_open,
                make_msg(FootprintEditorMsg::ContextMenuOpenSubmenu(Some(
                    FootprintContextSubmenu::View,
                ))),
            ));
            if view_open {
                items.push(item_indented(
                    tokens,
                    "Fit to Window",
                    "V, F",
                    make_msg(FootprintEditorMsg::ContextMenuAction(
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
                    make_msg(FootprintEditorMsg::PastePad),
                ));
                items.push(separator(tokens));
            }

            items.push(item_msg(
                tokens,
                "Properties...",
                "",
                make_msg(FootprintEditorMsg::CloseContextMenu),
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
                make_msg(FootprintEditorMsg::CloseContextMenu),
            ));

            // v0.26-G — Pad Actions ▸ now expands to the items we
            // can wire today (Rotate 90°, Flip Layer). Altium''s
            // Custom Pad / Thermal Connection ops remain stubs until
            // those subsystems land.
            let pad_actions_open = menu_state.submenu == Some(FootprintContextSubmenu::PadActions);
            items.push(item_submenu_header(
                tokens,
                "Pad Actions",
                pad_actions_open,
                make_msg(FootprintEditorMsg::ContextMenuOpenSubmenu(Some(
                    FootprintContextSubmenu::PadActions,
                ))),
            ));
            if pad_actions_open {
                items.push(item_indented(
                    tokens,
                    "Rotate 90°",
                    "Space",
                    make_msg(FootprintEditorMsg::ActiveBarRotateSelection),
                ));
                items.push(item_indented(
                    tokens,
                    "Flip Layer",
                    "X",
                    make_msg(FootprintEditorMsg::ActiveBarFlipSelection),
                ));
                // Stubs — Custom Pad + Thermal subsystems pending.
                items.push(item_indented(
                    tokens,
                    "Custom Pad from Outline...",
                    "",
                    make_msg(FootprintEditorMsg::CloseContextMenu),
                ));
                items.push(item_indented(
                    tokens,
                    "Thermal Connection Points...",
                    "",
                    make_msg(FootprintEditorMsg::CloseContextMenu),
                ));
            }

            items.push(separator(tokens));

            // v0.26-E — wired clipboard ops on the selected pad.
            items.push(item_msg(
                tokens,
                "Cut",
                &cut_shortcut,
                make_msg(FootprintEditorMsg::CutPad),
            ));
            items.push(item_msg(
                tokens,
                "Copy",
                &copy_shortcut,
                make_msg(FootprintEditorMsg::CopyPad),
            ));
            if has_clipboard {
                items.push(item_msg(
                    tokens,
                    "Paste",
                    &paste_shortcut,
                    make_msg(FootprintEditorMsg::PastePad),
                ));
            }
            items.push(item_msg(
                tokens,
                "Delete",
                "Del",
                make_msg(FootprintEditorMsg::DeleteSelected),
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
            // A baked entry comes back with the next bake: not deletable.
            items.push(match silk_delete_msg(editor.primitive(), idx) {
                Some(msg) => item_msg(tokens, "Delete", "Del", make_msg(msg)),
                None => item_disabled(tokens, "Delete", "Del"),
            });
            items.push(separator(tokens));
            items.push(item_disabled(tokens, "Find Similar Objects...", ""));
        }
    }

    let card_bg = ti(tokens.panel_bg);
    let border_c = ti(tokens.border);
    let card = container(column(items).spacing(0))
        .padding(Padding::from([SP.s, 0.0]))
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(card_bg)),
            border: Border {
                color: border_c,
                width: 1.0,
                radius: 4.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .width(MENU_WIDTH);

    Some(card.boxed())
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
            text(raw(label.to_string())).size(FONT_SIZE).color(text_c),
            iced::widget::Space::new().width(Length::Fill),
            text(raw(shortcut.to_string()))
                .size(TS.caption)
                .color(secondary),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill),
    )
    .width(MENU_WIDTH)
    .padding([ROW_PAD_Y, ROW_PAD_X])
    .on_press(message)
    .style(
        move |_: &iced::Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(Background::Color(hover_c)),
                _ => None,
            };
            iced::widget::button::Style {
                background: bg,
                border: Border::default(),
                text_color: text_c,
                ..iced::widget::button::Style::default()
            }
        },
    )
    .boxed()
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
            iced::widget::Space::new().width(SP.xl),
            text(raw(label.to_string())).size(FONT_SIZE).color(text_c),
            iced::widget::Space::new().width(Length::Fill),
            text(raw(shortcut.to_string()))
                .size(TS.caption)
                .color(secondary),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill),
    )
    .width(MENU_WIDTH)
    .padding([ROW_PAD_Y, ROW_PAD_X])
    .on_press(message)
    .style(
        move |_: &iced::Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(Background::Color(hover_c)),
                _ => None,
            };
            iced::widget::button::Style {
                background: bg,
                border: Border::default(),
                text_color: text_c,
                ..iced::widget::button::Style::default()
            }
        },
    )
    .boxed()
}

fn item_disabled<'a>(
    tokens: &'a ThemeTokens,
    label: &str,
    shortcut: &str,
) -> Element<'a, LibraryMessage> {
    let muted = ti(tokens.text_secondary);
    container(
        row![
            text(raw(label.to_string())).size(FONT_SIZE).color(muted),
            iced::widget::Space::new().width(Length::Fill),
            text(raw(shortcut.to_string()))
                .size(TS.caption)
                .color(muted),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill),
    )
    .width(MENU_WIDTH)
    .padding([ROW_PAD_Y, ROW_PAD_X])
    .boxed()
}

fn item_submenu_header<'a>(
    tokens: &'a ThemeTokens,
    label: &str,
    expanded: bool,
    message: LibraryMessage,
) -> Element<'a, LibraryMessage> {
    let text_c = ti(tokens.text);
    let hover_c = ti(tokens.hover);
    button(
        row![
            text(raw(label.to_string())).size(FONT_SIZE).color(text_c),
            iced::widget::Space::new().width(Length::Fill),
            signex_widgets::chevron::chevron(expanded, TS.body, text_c),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill),
    )
    .width(MENU_WIDTH)
    .padding([ROW_PAD_Y, ROW_PAD_X])
    .on_press(message)
    .style(
        move |_: &iced::Theme, status: iced::widget::button::Status| {
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
        },
    )
    .boxed()
}

fn separator<'a>(tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    let c = ti(tokens.border);
    container(iced::widget::Space::new().height(crate::styles::HAIRLINE))
        .padding(Padding::from([SP.s, 0.0]))
        .width(MENU_WIDTH)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(c)),
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::{Footprint, FootprintEditorMsg, silk_delete_msg};
    use crate::keymap::{
        AppCommandId, CompiledKeymap, ShortcutBinding, ShortcutBindingAction, ShortcutContext,
        ShortcutProfile, ShortcutProfileKind, ShortcutTrigger,
    };
    use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};

    fn keymap_with(bindings: Vec<(ShortcutBindingAction, &str)>) -> CompiledKeymap {
        CompiledKeymap::compile(&ShortcutProfile {
            id: "t".to_string(),
            name: "T".to_string(),
            kind: ShortcutProfileKind::Custom,
            schema_version: 1,
            description: None,
            base_profile: None,
            bindings: bindings
                .into_iter()
                .map(|(action, trigger)| ShortcutBinding {
                    action,
                    context: ShortcutContext::Global,
                    triggers: vec![ShortcutTrigger::parse(trigger).unwrap()],
                })
                .collect(),
        })
    }

    // #1378: an unbound or unbound-by-profile command advertises no key.
    #[test]
    fn a_context_menu_hint_follows_the_keymap_not_a_fixed_fallback() {
        let copy = AppCommandId::new("copy").unwrap();
        let keymap = keymap_with(vec![
            (ShortcutBindingAction::Command(copy.clone()), "Ctrl+C"),
            (ShortcutBindingAction::Unbind(copy), "Ctrl+C"),
            (
                ShortcutBindingAction::Command(AppCommandId::new("paste").unwrap()),
                "F4",
            ),
        ]);
        assert_eq!(
            keymap.shortcut_label_for_id("copy"),
            "",
            "unbound by the user"
        );
        assert_eq!(keymap.shortcut_label_for_id("cut"), "", "never bound");
        assert_eq!(keymap.shortcut_label_for_id("paste"), "F4");
    }

    #[test]
    fn a_baked_silk_entry_has_no_delete_message() {
        let mut fp = Footprint::empty("t");
        let hand = FpGraphic::drawn(
            FpGraphicKind::Line {
                from: [0.0, 0.0],
                to: [1.0, 0.0],
            },
            0.12,
            false,
        );
        let mut baked = hand.clone();
        baked.sketch_entity = Some(signex_sketch::id::SketchEntityId::new());
        fp.silk_f = vec![baked, hand];

        assert!(silk_delete_msg(&fp, 0).is_none());
        assert!(matches!(
            silk_delete_msg(&fp, 1),
            Some(FootprintEditorMsg::DeleteSilkF)
        ));
    }
}
