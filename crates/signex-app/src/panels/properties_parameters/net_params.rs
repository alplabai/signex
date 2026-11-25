//! The Net Parameters chrome of a power port (the All / Parameters / Rules /
//! Classes tabs, the empty card and its Add button) plus the 3x3
//! justification-grid picker of a net label. Moved from the former
//! single-file `properties_parameters` module.

use super::super::*;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{MessageId, ids};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Net Parameters tabs — All / Parameters / Rules / Classes, as a segmented
/// control. A net holds no parameters, rules or classes here yet, so All
/// stays on and every tab shows the same empty card.
pub fn net_params_tabs<'a>(
    i18n: &signex_i18n::I18n,
    primary: Color,
    muted: Color,
    input_bg: Color,
) -> Element<'a, PanelMsg> {
    let tab = |label: MessageId, active: bool| {
        super::seg_btn(
            signex_i18n::t!(i18n, label),
            active,
            PanelMsg::Noop,
            input_bg,
            primary,
            muted,
        )
    };
    container(
        container(
            signex_widgets::FitRow::new(vec![
                tab(ids::PROPERTIES_TAB_ALL, true),
                tab(ids::PROPERTIES_TAB_PARAMETERS, false),
                tab(ids::PROPERTIES_TAB_RULES, false),
                tab(ids::PROPERTIES_TAB_CLASSES, false),
            ])
            .fill(true),
        )
        .padding(SP.xs)
        .width(Length::Fill)
        .style(crate::panels::form_look::segmented_frame),
    )
    .padding([4, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// Empty-state row — centered muted text spanning the whole row.
pub fn empty_section_row<'a>(
    label: signex_i18n::Localized,
    label_c: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    container(
        container(text(label).size(TS.caption).color(label_c))
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
    )
    .padding([6, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: border_c,
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// The Add ▾ button under the Net Parameters card.
pub fn net_params_add_bar<'a>(i18n: &signex_i18n::I18n) -> Element<'a, PanelMsg> {
    container(
        row![
            Space::new().width(Length::Fill),
            signex_widgets::button(
                text(signex_i18n::t!(i18n, ids::PROPERTIES_NET_PARAMS_ADD)).size(TS.body)
            )
            .padding([SP.s, SP.l])
            .on_press(PanelMsg::Noop)
            .style(crate::panels::form_look::ghost_button),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([6, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// Which way a label's text runs from its anchor: the four directions the
/// 3x3 grid maps its nine cells onto.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LabelDir {
    Left,
    Up,
    Right,
    Down,
}

/// The direction a label turned `rotation_deg` and justified `h` reads in.
fn label_direction(rotation_deg: f64, h: signex_types::schematic::HAlign) -> LabelDir {
    use signex_types::schematic::HAlign;
    let turn = (rotation_deg.round() as i32).rem_euclid(360);
    // Right-justified text runs left of its anchor, unless turned upside down.
    let runs_right = (h == HAlign::Right) == (turn == 180);
    match turn {
        90 => LabelDir::Up,
        270 => LabelDir::Down,
        _ if runs_right => LabelDir::Right,
        _ => LabelDir::Left,
    }
}

/// The word beside the justification grid that names the label's anchor:
/// where on its text the anchor sits.
pub fn label_anchor(rotation_deg: f64, h: signex_types::schematic::HAlign) -> MessageId {
    match label_direction(rotation_deg, h) {
        LabelDir::Right => ids::PROPERTIES_ANCHOR_BOTTOM_LEFT,
        LabelDir::Left => ids::PROPERTIES_ANCHOR_BOTTOM_RIGHT,
        LabelDir::Up => ids::PROPERTIES_ANCHOR_BOTTOM,
        LabelDir::Down => ids::PROPERTIES_ANCHOR_TOP,
    }
}

/// The (row, column) of the grid cell that marks `dir`: the place on the
/// text where the anchor sits, as [`label_anchor`] words it.
fn anchor_cell(dir: LabelDir) -> (usize, usize) {
    match dir {
        LabelDir::Right => (2, 0),
        LabelDir::Up => (2, 1),
        LabelDir::Left => (2, 2),
        LabelDir::Down => (0, 1),
    }
}

/// The direction a press on cell (`row`, `column`) picks. A label reads in
/// four directions: the left column anchors the text at its left, the right
/// column at its right, top and bottom middle turn it, and the centre keeps
/// `current`.
fn cell_direction(row: usize, column: usize, current: LabelDir) -> LabelDir {
    match (row, column) {
        (_, 0) => LabelDir::Right,
        (_, 2) => LabelDir::Left,
        (0, 1) => LabelDir::Down,
        (2, 1) => LabelDir::Up,
        _ => current,
    }
}

/// The 3x3 justification picker of a selected label (mockup row 44): a
/// sunken grid of dots. The one raised cell with the accent dot is the
/// anchor the word beside the grid names.
pub fn justification_grid(
    id: uuid::Uuid,
    rotation_deg: f64,
    h: signex_types::schematic::HAlign,
) -> Element<'static, PanelMsg> {
    use signex_types::schematic::HAlign;
    let current_dir = label_direction(rotation_deg, h);

    let to_msg = |dir: LabelDir| -> PanelMsg {
        match dir {
            LabelDir::Right => PanelMsg::EditLabelDirection(id, 0.0, HAlign::Left),
            LabelDir::Left => PanelMsg::EditLabelDirection(id, 0.0, HAlign::Right),
            LabelDir::Up => PanelMsg::EditLabelDirection(id, 90.0, HAlign::Left),
            LabelDir::Down => PanelMsg::EditLabelDirection(id, 270.0, HAlign::Left),
        }
    };

    let lit = anchor_cell(current_dir);
    crate::panels::form_look::anchor_grid(std::array::from_fn(|r| {
        std::array::from_fn(|c| ((r, c) == lit, to_msg(cell_direction(r, c, current_dir))))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::HAlign;

    const DIRECTIONS: [LabelDir; 4] = [
        LabelDir::Right,
        LabelDir::Up,
        LabelDir::Left,
        LabelDir::Down,
    ];

    /// Mockup row 44: a label running right is anchored at its bottom left,
    /// and that is the cell lit beside the word.
    #[test]
    fn the_lit_cell_is_the_anchor_the_word_names() {
        assert_eq!(
            label_anchor(0.0, HAlign::Left),
            ids::PROPERTIES_ANCHOR_BOTTOM_LEFT
        );
        assert_eq!(anchor_cell(label_direction(0.0, HAlign::Left)), (2, 0));
        assert_eq!(
            label_anchor(0.0, HAlign::Right),
            ids::PROPERTIES_ANCHOR_BOTTOM_RIGHT
        );
        assert_eq!(anchor_cell(label_direction(0.0, HAlign::Right)), (2, 2));
        assert_eq!(
            label_anchor(90.0, HAlign::Left),
            ids::PROPERTIES_ANCHOR_BOTTOM
        );
        assert_eq!(anchor_cell(label_direction(90.0, HAlign::Left)), (2, 1));
        assert_eq!(
            label_anchor(270.0, HAlign::Left),
            ids::PROPERTIES_ANCHOR_TOP
        );
        assert_eq!(anchor_cell(label_direction(270.0, HAlign::Left)), (0, 1));
    }

    /// Pressing the lit cell of any direction keeps that direction, so a
    /// press never lights a different cell than the one pressed.
    #[test]
    fn a_press_on_an_anchor_cell_picks_that_anchor() {
        for dir in DIRECTIONS {
            let (row, column) = anchor_cell(dir);
            for current in DIRECTIONS {
                assert!(cell_direction(row, column, current) == dir);
            }
        }
    }

    #[test]
    fn the_centre_keeps_the_current_direction() {
        for current in DIRECTIONS {
            assert!(cell_direction(1, 1, current) == current);
        }
    }
}
