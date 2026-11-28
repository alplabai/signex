//! #3062, mockup row 164: the board's Grid popover names the key that
//! cycles the grid and lists the grid in effect, checked, even when it is
//! none of the presets.

use crate::app::{Message, UiMsg};
use crate::layout_probe::{LaidOutText, draw_pixels, lay_out_texts, messages_on_click_at};
use crate::status_bar::{BoardCounter, BoardPopoverEvent};

use super::tests::{app_with_board, click, event};

const WINDOW: iced::Size = iced::Size::new(1600.0, 900.0);

/// The laid-out texts with Fluent's isolation marks taken out.
fn plain(texts: Vec<LaidOutText>) -> Vec<LaidOutText> {
    texts
        .into_iter()
        .map(|t| LaidOutText {
            text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
            bounds: t.bounds,
        })
        .collect()
}

fn at<'t>(texts: &'t [LaidOutText], label: &str) -> &'t LaidOutText {
    crate::layout_probe::find(texts, label)
}

/// The pixels of the trailing part of the row whose label is `row`: from
/// past the label to the card's right edge, over the label's height.
fn row_tail(pixels: &[u8], row: iced::Rectangle) -> Vec<u8> {
    let width = WINDOW.width as usize;
    let (x0, x1) = ((row.x + 100.0) as usize, (row.x + 260.0) as usize);
    // Same band height for every row, so two tails compare pixel for pixel.
    let y0 = row.y as usize;
    (y0..y0 + row.height as usize)
        .flat_map(|y| pixels[(y * width + x0) * 4..(y * width + x1) * 4].to_vec())
        .collect()
}

#[test]
fn the_grid_popover_lists_the_current_grid_checked_and_names_its_key() {
    let (mut app, _path, main) = app_with_board();
    click(&mut app, main, BoardCounter::Grid);
    event(&mut app, BoardPopoverEvent::CustomGridEdited("0.05".into()));
    event(&mut app, BoardPopoverEvent::CustomGridSubmitted);
    assert_eq!(app.status_grid(main).size_mm, 0.05);

    let card = app.board_popover_card(main).expect("the Grid popover");
    let texts = plain(lay_out_texts(card, WINDOW));
    // The Signex Standard profile binds G to next_grid on a board.
    assert!(texts.iter().any(|t| t.text == "G cycles"), "{texts:#?}");
    let (current, preset) = (at(&texts, "0.050 mm").bounds, at(&texts, "0.100 mm").bounds);
    assert!(current.y < preset.y, "0.050 sorts before 0.100");

    // The check sits on the current row only.
    let card = app.board_popover_card(main).expect("the Grid popover");
    let pixels = draw_pixels(card, WINDOW);
    assert_ne!(
        row_tail(&pixels, current),
        row_tail(&pixels, preset),
        "the current row carries the check, the preset row none"
    );

    // The row is a pitch to pick like a preset.
    let card = app.board_popover_card(main).expect("the Grid popover");
    let sent = messages_on_click_at(card, WINDOW, None, current.center());
    assert!(
        sent.iter().any(|m| matches!(
            m,
            Message::Ui(UiMsg::StatusBar(crate::app::StatusBarRequest::BoardPopover(
                BoardPopoverEvent::GridPresetPicked(mm)
            ))) if (*mm - 0.05).abs() < 1e-6
        )),
        "{sent:?}"
    );
}

/// On a preset grid the list is the four presets, as before.
#[test]
fn a_preset_grid_adds_no_row() {
    let (mut app, _path, main) = app_with_board();
    click(&mut app, main, BoardCounter::Grid);
    event(&mut app, BoardPopoverEvent::GridPresetPicked(0.25));
    let card = app.board_popover_card(main).expect("the Grid popover");
    let rows: Vec<String> = plain(lay_out_texts(card, WINDOW))
        .into_iter()
        .map(|t| t.text)
        .filter(|t| t.ends_with(" mm") && t.starts_with(|c: char| c.is_ascii_digit()))
        .collect();
    assert_eq!(rows, ["0.100 mm", "0.250 mm", "0.500 mm", "1.270 mm"]);
}
