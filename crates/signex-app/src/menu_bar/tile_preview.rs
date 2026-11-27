//! The preview beside a tile row of View ▸ Window (mockup row 207): while
//! the row is hovered, a card shows the row's name and how many documents
//! it arranges, the panes in their arrangement with the names of the
//! documents each receives, and a line saying what the arrangement does.

use iced::widget::{Column, container};
use iced::{Border, Element, Length, Theme, Widget as _};
use signex_widgets::text;

use super::model::TilePreview;
use super::{MENU_SHORTCUT_SIZE, MenuColors, MenuMessage};

/// The panes' drawing inside the card, in pixels.
const PANES: (f32, f32) = (224.0, 112.0);

/// The card for `preview` in `mc`'s colours.
pub(super) fn card(preview: &TilePreview, mc: MenuColors) -> Element<'static, MenuMessage> {
    let tiles = crate::document_panes::pane_tiles(preview.panes.len(), PANES, preview.layout, &[]);
    let cells: Vec<Element<'static, MenuMessage>> = preview
        .panes
        .iter()
        .map(|names| {
            let lines = names.iter().map(|name| {
                text(signex_i18n::UserText::new(name.clone()))
                    .size(MENU_SHORTCUT_SIZE)
                    .color(mc.text)
                    .boxed()
            });
            container(Column::with_children(lines).spacing(2.0))
                .padding(4.0)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(mc.toolbar_bg.into()),
                    border: Border {
                        color: mc.border,
                        width: 1.0,
                        radius: 3.0.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .boxed()
        })
        .collect();
    let panes = container(crate::app::view::pane_chrome::place_cells(cells, &tiles))
        .width(Length::Fixed(PANES.0))
        .height(Length::Fixed(PANES.1));
    let title = text(signex_i18n::UserText::new(preview.title.clone()))
        .size(MENU_SHORTCUT_SIZE)
        .color(mc.text_muted);
    let note = text(signex_i18n::UserText::new(preview.note.clone()))
        .size(MENU_SHORTCUT_SIZE)
        .color(mc.text_muted)
        .width(Length::Fixed(PANES.0));
    container(Column::with_children([title.boxed(), panes.boxed(), note.boxed()]).spacing(6.0))
        .padding(8.0)
        .style(move |_: &Theme| container::Style {
            background: Some(mc.panel_bg.into()),
            border: Border {
                color: mc.border,
                width: 1.0,
                radius: 6.0.into(),
                ..Border::default()
            },
            shadow: mc.shadow,
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::TileLayout;

    /// Mockup row 207: the card names the row, the documents in their panes
    /// (two to a row in a grid) and what the arrangement does.
    #[test]
    fn the_card_shows_each_document_in_its_pane() {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::BUILTINS[0]);
        let preview = TilePreview {
            layout: TileLayout::Grid,
            title: "Tile Grid · 3 open documents".to_string(),
            panes: vec![
                vec!["power.snxsch".to_string(), "mcu.snxsch".to_string()],
                vec!["buck.snxpcb".to_string()],
                vec!["Design Checklist".to_string()],
            ],
            note: "One kind each".to_string(),
        };
        let texts = crate::layout_probe::lay_out_texts(
            card(&preview, MenuColors::from_tokens(&tokens)),
            iced::Size::new(400.0, 400.0),
        );
        let at = |label: &str| crate::layout_probe::find(&texts, label).bounds;
        let (power, mcu, board, list) = (
            at("power.snxsch"),
            at("mcu.snxsch"),
            at("buck.snxpcb"),
            at("Design Checklist"),
        );
        assert!(
            mcu.y > power.y,
            "a pane lists its documents one under another"
        );
        assert!(
            board.x > power.x && (board.y - power.y).abs() < 1.0,
            "second pane beside"
        );
        assert!(list.y > power.y + 20.0, "the third pane below");
        assert!(at("Tile Grid · 3 open documents").y < power.y);
        assert!(at("One kind each").y > list.y);
    }
}
