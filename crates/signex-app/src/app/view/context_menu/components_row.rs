//! Right-click menu of a Components panel row (Place, Edit Component, Copy
//! Part Number) and
//! the card that follows a row dragged toward the canvas (#2535).

use super::*;

use iced::Widget as _;

use super::items::{dd_msg, text};
use crate::dock::{PanelPosition, PanelSite};
use crate::library::messages::LibraryMessage;
use crate::library::state::ComponentsRowAction;
use signex_i18n::{I18n, menu_ids};
use signex_types::theme::ThemeId;
use signex_widgets::active_bar_dropdown::{DropdownEntry, MenuMetrics};

/// The command whose key the Place row shows: Enter accepts a placement.
const PLACE_HINT_COMMAND: &str = "placement_accept";

/// Menu rows: pure data, so the labels and messages are tested without a
/// renderer. `place_hint` is the key the active profile gives Enter's
/// command ("" when it binds none); the icon column aligns the labels.
pub(super) fn components_row_menu_entries(
    i18n: &I18n,
    theme: ThemeId,
    place_hint: &str,
) -> Vec<DropdownEntry<Message>> {
    let pick = |action| Message::Library(LibraryMessage::ComponentsPanelRowMenuPicked(action));
    vec![
        dd_msg(
            Some(crate::icons::icon_plus(theme)),
            &text(i18n, menu_ids::CTX_COMPONENTS_ROW_PLACE),
            place_hint,
            pick(ComponentsRowAction::Place),
        ),
        dd_msg(
            Some(crate::icons::icon_dd_properties(theme)),
            &text(i18n, menu_ids::CTX_COMPONENTS_ROW_EDIT),
            "",
            pick(ComponentsRowAction::Open),
        ),
        DropdownEntry::Separator,
        dd_msg(
            Some(crate::icons::icon_dd_copy(theme)),
            &text(i18n, menu_ids::CTX_COMPONENTS_ROW_COPY_PART_NUMBER),
            "",
            pick(ComponentsRowAction::CopyPartNumber),
        ),
    ]
}

/// Where the row menu opens: right-aligned inside the dock that holds the
/// panel, level with the pointer. A floating or bottom-docked panel keeps
/// the pointer anchor. `area` is the document area between the docks.
pub(super) fn dock_menu_anchor(
    site: Option<PanelSite>,
    area: iced::Rectangle,
    window_width: f32,
    pointer: (f32, f32),
    menu_width: f32,
) -> (f32, f32) {
    let dock_right = match site {
        Some(PanelSite::Docked(PanelPosition::Left, _)) => area.x,
        Some(PanelSite::Docked(PanelPosition::Right, _)) => window_width,
        _ => return pointer,
    };
    let x = (dock_right - menu_width - super::placement::EDGE_MARGIN).max(0.0);
    (x, pointer.1)
}

impl Signex {
    /// The open row menu with its click-outside backdrop; empty when no
    /// menu is open.
    pub(in crate::app::view) fn components_row_context_menu_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        let Some(menu_ctx) = self.library.components_panel.row_menu.as_ref() else {
            return Vec::new();
        };
        if !self.overlay_belongs_to(menu_ctx.window, window_id) {
            return Vec::new();
        }
        let panel_ctx = &self.document_view.panel_ctx;
        let entries = components_row_menu_entries(
            &self.ui_state.i18n,
            self.ui_state.effective_theme_id(),
            &self.keymap_shortcut_label(PLACE_HINT_COMMAND),
        );
        let pointer = (menu_ctx.x, menu_ctx.y);
        let anchor = match menu_ctx.window {
            None => dock_menu_anchor(
                self.document_view
                    .dock
                    .locate(crate::panels::PanelKind::Components),
                self.document_area_in(None),
                self.ui_state.window_size.0,
                pointer,
                MenuMetrics::GRAPHITE.fitted_width(&entries, f32::MAX, &panel_ctx.ui_font_name),
            ),
            // An undocked panel window is the panel: the pointer is in it.
            Some(_) => pointer,
        };
        let menu = super::placement::menu_card(entries, &panel_ctx.tokens, &panel_ctx.ui_font_name);
        let layers = Self::menu_overlay(
            Message::Library(LibraryMessage::ComponentsPanelRowMenuDismissed),
            menu.element,
            anchor,
        );
        match menu_ctx.window {
            None => layers,
            // A pick in an undocked window acts on that window's sheet.
            Some(window) => layers
                .into_iter()
                .map(|layer| {
                    layer
                        .map(move |m| {
                            crate::app::dispatch::window_action::scope_row_menu_message(window, m)
                        })
                        .boxed()
                })
                .collect(),
        }
    }
}

impl Signex {
    /// The card beside the pointer while a row is dragged: the symbol
    /// thumbnail, the part number and "release to place". Empty until the
    /// press has become a drag, and once the part itself follows the
    /// pointer on the canvas. Kept left of the canvas's right edge.
    pub(in crate::app::view) fn components_row_drag_label_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Vec<Element<'_, Message>> {
        use iced::widget::{column, container, row};

        const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
        const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;
        let panel = &self.library.components_panel;
        let Some(drag) = panel.drag.as_ref().filter(|d| d.started) else {
            return Vec::new();
        };
        if !self.overlay_belongs_to(drag.window, window_id) {
            return Vec::new();
        }
        // Over the canvas the part itself follows the pointer.
        let (px, py) = self.interaction_state.last_mouse_pos;
        let canvas = self.canvas_bounds_in(drag.window);
        if drag.armed
            && (self.interaction_state.placement.is_some() || self.has_pcb_part_in_hand())
            && canvas.contains(iced::Point::new(px, py))
        {
            return Vec::new();
        }
        let key = &drag.key;
        let Some(row_data) = self
            .library
            .library_at(&key.library_path)
            .and_then(|lib| lib.tables.get(&key.table))
            .and_then(|rows| rows.iter().find(|r| r.row_id == key.row_id.0))
        else {
            return Vec::new();
        };
        let panel_ctx = &self.document_view.panel_ctx;
        let tokens = &panel_ctx.tokens;
        let look = crate::panels::components_panel::thumbs::ThumbLook {
            colors: panel_ctx.canvas_colors,
            device_px_per_logical_px: panel_ctx.device_px_per_logical_px,
        };
        let thumb: Element<'_, Message> = match panel.thumbs.get(&row_data.symbol_ref) {
            Some(symbol) => look.thumb(symbol.clone()),
            None => look.placeholder(),
        };
        let ink = signex_widgets::theme_ext::text_primary(tokens);
        let muted = crate::styles::tokens::resolve(tokens).ink4;
        let i18n = &self.ui_state.i18n;
        let card = container(
            row![
                thumb,
                column![
                    signex_widgets::text(signex_i18n::UserText::new(
                        row_data.internal_pn.to_string()
                    ))
                    .size(TS.caption)
                    .color(ink),
                    signex_widgets::text(signex_i18n::t!(
                        i18n,
                        signex_i18n::ids::PANELS_COMPONENTS_DRAG_RELEASE
                    ))
                    .size(TS.caption)
                    .color(muted),
                ]
                .spacing(SP.xs / 2.0),
            ]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.xs, SP.s])
        .style(crate::styles::context_menu(tokens));
        // Today's offset from the pointer, then clamped so the card stays
        // left of the canvas's right edge (and inside the window).
        let anchored = super::anchored::AnchoredMenu::new(card.boxed(), (px + SP.l, py + SP.l));
        vec![
            container(anchored.boxed())
                .width(iced::Length::Fixed(canvas.x + canvas.width))
                .height(iced::Length::Fill)
                .boxed(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels_and_hints(entries: Vec<DropdownEntry<Message>>) -> Vec<(String, Option<String>)> {
        entries
            .into_iter()
            .filter_map(|e| match e {
                DropdownEntry::Item(item) => Some((item.label.to_string(), item.shortcut.clone())),
                _ => None,
            })
            .collect()
    }

    /// Place (with Enter on the right), Edit Component and, below a
    /// separator, Copy Part Number, all with an icon so the labels align.
    #[test]
    fn the_menu_offers_place_with_its_key_and_edit_component() {
        let entries =
            components_row_menu_entries(I18n::en_us_ref(), ThemeId::GraphiteDark, "Enter");
        assert!(entries.iter().all(|e| match e {
            DropdownEntry::Item(item) => item.icon.is_some(),
            _ => true,
        }));
        assert_eq!(
            labels_and_hints(entries),
            [
                ("Place".to_owned(), Some("Enter".to_owned())),
                ("Edit Component".to_owned(), None),
                ("Copy Part Number".to_owned(), None),
            ]
        );
    }

    /// A profile that binds no key to Enter's command shows no hint.
    #[test]
    fn an_unbound_place_key_shows_no_hint() {
        let entries = components_row_menu_entries(I18n::en_us_ref(), ThemeId::GraphiteLight, "");
        assert_eq!(labels_and_hints(entries)[0].1, None);
    }

    /// The menu opens right-aligned inside the dock holding the panel.
    #[test]
    fn the_menu_opens_right_aligned_inside_its_dock() {
        let area =
            iced::Rectangle::new(iced::Point::new(240.0, 60.0), iced::Size::new(900.0, 700.0));
        let margin = super::super::placement::EDGE_MARGIN;
        let left = dock_menu_anchor(
            Some(PanelSite::Docked(PanelPosition::Left, 0)),
            area,
            1400.0,
            (100.0, 300.0),
            180.0,
        );
        assert_eq!(left, (240.0 - 180.0 - margin, 300.0));
        let right = dock_menu_anchor(
            Some(PanelSite::Docked(PanelPosition::Right, 1)),
            area,
            1400.0,
            (1300.0, 300.0),
            180.0,
        );
        assert_eq!(right, (1400.0 - 180.0 - margin, 300.0));
        let floating = dock_menu_anchor(
            Some(PanelSite::Floating(0)),
            area,
            1400.0,
            (5.0, 6.0),
            180.0,
        );
        assert_eq!(floating, (5.0, 6.0));
    }

    /// The opened menu, laid out, shows each row's label on one line at its
    /// natural width (#2043): the box must not collapse to rows without text.
    #[test]
    fn the_open_menu_lays_out_every_row_label_at_its_natural_width() {
        use crate::layout_probe::{find, lay_out_texts, natural};
        use crate::library::state::{ComponentsPanelRowKey, ComponentsRowMenu};

        let (mut app, _boot) = Signex::new();
        app.library.components_panel.row_menu = Some(ComponentsRowMenu {
            key: ComponentsPanelRowKey {
                library_path: "lib.snxlib".into(),
                table: "t".into(),
                row_id: signex_library::RowId::from_uuid(uuid::Uuid::from_u128(1)),
            },
            x: 100.0,
            y: 100.0,
            window: None,
        });
        let main = iced::window::Id::unique();
        app.ui_state.main_window_id = Some(main);
        let mut overlay = app.components_row_context_menu_overlay(main);
        assert_eq!(overlay.len(), 2, "backdrop and menu");
        let menu = overlay.remove(1);
        let texts = lay_out_texts(menu, iced::Size::new(1400.0, 900.0));

        for label in ["Place", "Edit Component", "Copy Part Number"] {
            let alone = natural::<()>(
                // The size the dropdown draws its labels at.
                signex_widgets::text(signex_widgets::raw(label))
                    .size(signex_widgets::active_bar::dropdown::MENU_LABEL_SIZE)
                    .wrapping(iced::widget::text::Wrapping::None)
                    .boxed(),
                label,
            );
            let laid = find(&texts, label).bounds;
            assert!(
                laid.width >= alone.width - 0.5 && laid.height >= alone.height - 0.5,
                "{label} laid out at {laid:?}, natural {alone:?}"
            );
        }
    }
}
