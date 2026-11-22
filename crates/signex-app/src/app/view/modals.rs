//! The Move Selection dialog body (#66): ΔX / ΔY with the unit as a suffix,
//! a read-only line-art preview of the move and a grid-step help line. The
//! dialog lives in its own window, whose title bar carries the title and the
//! selection count. The net colour bodies are in `view/net_color/`.

use super::*;
use crate::app::handlers::editing_commands::move_selection_dialog::{
    MOVE_DX_INPUT_ID, PREVIEW_EM, grid_steps, offset_mm, stored_dy,
};
use crate::styles::{DialogButtonKind, dialog_body, dialog_button, dialog_footer};
use iced::{Point, Rectangle, Size};
use signex_i18n::{Arg, UserText, ids, t};
use signex_types::coord::Unit;
use signex_widgets::LengthFormat;
use signex_widgets::property_illustration::{
    Callout, Illustration, IllustrationHover, Ink, Paint, Part, Primitive, Style, Tint,
};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Preview geometry, in the illustration's own view units: the selection's
/// box, the longest offset drawn, and the room kept for the callouts.
const BOX_W: f32 = 60.0;
const BOX_H: f32 = 34.0;
const SPAN: f32 = 80.0;
const MARGIN: f32 = 26.0;
const CALLOUT_GAP: f32 = 12.0;

impl Signex {
    /// The Move Selection dialog: the two offsets, the preview, the help
    /// line, and Cancel / Move. Enter in either field moves.
    pub(super) fn view_move_selection_body(&self) -> Element<'_, Message> {
        use iced::widget::{Widget, column, container, row};
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let ms = &self.ui_state.move_selection;
        let unit = self.ui_state.unit;
        let has_selection = !self.interaction_state.active_canvas().selected.is_empty();
        let preview = |text: &str| offset_mm(text, unit).unwrap_or(0.0);
        let (dx_mm, dy_mm) = (preview(&ms.dx), preview(&ms.dy));

        let fields = row![
            offset_field(
                t!(i18n, ids::DIALOG_MOVE_SELECTION_DX),
                &ms.dx,
                Some(MOVE_DX_INPUT_ID.clone()),
                unit,
                |s| Message::MoveSelection(MoveSelectionMsg::DxChanged(s)),
                self,
            ),
            offset_field(
                t!(i18n, ids::DIALOG_MOVE_SELECTION_DY),
                &ms.dy,
                None,
                unit,
                |s| Message::MoveSelection(MoveSelectionMsg::DyChanged(s)),
                self,
            ),
        ]
        .spacing(SP.l);

        let fmt = LengthFormat::new(i18n, unit);
        let preview = signex_widgets::property_illustration::property_illustration(
            move_preview(dx_mm, dy_mm, &fmt),
            &IllustrationHover::default(),
            tokens,
            TS.caption,
        )
        .map(|_| Message::noop());
        let help = signex_widgets::text(UserText::new(self.move_selection_help(dx_mm, dy_mm)))
            .size(TS.caption)
            .color(crate::styles::tokens::resolve(tokens).ink3);

        let body = dialog_body(column![fields, preview, help].spacing(SP.m));
        let footer = dialog_footer(vec![
            dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                DialogButtonKind::Cancel,
                Some(Message::MoveSelection(MoveSelectionMsg::Close)),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::DIALOG_MOVE_SELECTION_MOVE),
                DialogButtonKind::Primary,
                has_selection.then_some(Message::MoveSelection(MoveSelectionMsg::Apply)),
                tokens,
            ),
        ]);

        container(column![body, footer].width(iced::Length::Fill))
            .style(crate::styles::modal_card(tokens))
            .clip(true)
            .boxed()
    }

    /// The dialog's subtitle: how many items the move takes.
    pub(crate) fn move_selection_subtitle(&self) -> String {
        let count = self.interaction_state.active_canvas().selected.len();
        self.ui_state.i18n.message_plain(
            ids::DIALOG_MOVE_SELECTION_COUNT,
            &[("count", Arg::from(count))],
        )
    }

    /// The help line: the axis directions and the move in whole grid steps,
    /// or "off grid" for a part step.
    pub(crate) fn move_selection_help(&self, dx_mm: f64, dy_mm: f64) -> String {
        let i18n = &self.ui_state.i18n;
        let unit = self.ui_state.unit;
        let grid_mm = crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm);
        let grid = LengthFormat::new(i18n, unit).number(grid_mm);
        let unit = unit.to_string();
        match (grid_steps(dx_mm, grid_mm), grid_steps(dy_mm, grid_mm)) {
            (Some(x), Some(y)) => i18n.message_plain(
                ids::DIALOG_MOVE_SELECTION_HELP,
                &[
                    ("grid", Arg::from(grid)),
                    ("unit", Arg::from(unit)),
                    ("x", Arg::from(x)),
                    ("y", Arg::from(y)),
                ],
            ),
            _ => i18n.message_plain(
                ids::DIALOG_MOVE_SELECTION_HELP_OFF_GRID,
                &[("grid", Arg::from(grid)), ("unit", Arg::from(unit))],
            ),
        }
    }
}

/// One offset field: its caption over the input, the unit as a suffix
/// inside the field. Enter moves.
fn offset_field<'a>(
    label: signex_i18n::Localized,
    value: &str,
    id: Option<iced::widget::Id>,
    unit: Unit,
    on_input: fn(String) -> Message,
    app: &Signex,
) -> Element<'a, Message> {
    use iced::widget::{Widget, column, container, stack};
    use signex_widgets::{text, text_input};
    let tokens = &app.document_state.panel_ctx.tokens;
    let muted = crate::styles::ti(tokens.text_secondary);
    let ink3 = crate::styles::tokens::resolve(tokens).ink3;
    let suffix = UserText::new(unit.to_string());
    let mut input = text_input(
        t!(app.ui_state.i18n, ids::PLACEHOLDER_MOVE_OFFSET),
        value.to_owned(),
    )
    .style(crate::panels::form_look::text_input_style)
    .on_input(on_input)
    .on_submit(Message::MoveSelection(MoveSelectionMsg::Apply))
    .padding(iced::Padding {
        top: SP.s,
        bottom: SP.s,
        left: SP.m,
        // Room for the suffix: about three characters.
        right: SP.m + 3.0 * TS.caption,
    })
    .size(TS.label);
    if let Some(id) = id {
        input = input.id(id);
    }
    // The suffix sits on the field and takes no input, so presses reach it.
    let unit_suffix = container(text(suffix).size(TS.caption).color(ink3))
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .padding([0.0, SP.m])
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Center);
    column![
        text(label).size(TS.caption).color(muted),
        stack![input, unit_suffix],
    ]
    .spacing(SP.s)
    .width(iced::Length::Fill)
    .boxed()
}

/// The preview: the selection's box as a dashed ghost at its place and drawn
/// at its new place, with a ΔX and a ΔY callout. Read-only line art. A
/// positive ΔY draws upward; the callout shows the value as typed.
fn move_preview(dx_mm: f64, dy_mm: f64, fmt: &LengthFormat<'_>) -> Illustration {
    let largest = dx_mm.abs().max(dy_mm.abs());
    let k = if largest > 0.0 {
        SPAN / largest as f32
    } else {
        0.0
    };
    let (dx, dy) = (dx_mm as f32 * k, stored_dy(dy_mm) as f32 * k);
    let (min_x, max_x) = (dx.min(0.0), (dx + BOX_W).max(BOX_W));
    let (min_y, max_y) = (dy.min(0.0), (dy + BOX_H).max(BOX_H));
    let (ox, oy) = (MARGIN - min_x, MARGIN - min_y);
    let at = |x: f32, y: f32| Point::new(ox + x, oy + y);
    let rect = |x: f32, y: f32| Rectangle::new(at(x, y), Size::new(BOX_W, BOX_H));
    let mut drawing = Illustration::new(max_x - min_x + 2.0 * MARGIN, max_y - min_y + 2.0 * MARGIN)
        .height_em(PREVIEW_EM)
        .part(Part::new(
            Primitive::RoundedRect {
                rect: rect(0.0, 0.0),
                radius: 3.0,
            },
            Style::dashed(),
        ));
    if largest == 0.0 {
        return drawing;
    }
    drawing = drawing.part(Part::new(
        Primitive::RoundedRect {
            rect: rect(dx, dy),
            radius: 3.0,
        },
        Style::edge().with_fill(Paint::Ink(Ink::Accent), Tint::Light),
    ));
    let label = |mm: f64| UserText::new(format!("{} {}", fmt.number(mm), fmt.unit));
    if dx != 0.0 {
        let y = min_y - CALLOUT_GAP;
        drawing = drawing.part(Part::callout(
            Callout::new(at(0.0, y), at(dx, y))
                .label(label(dx_mm))
                .extension(at(0.0, 0.0), at(0.0, y))
                .extension(at(dx, dy), at(dx, y)),
        ));
    }
    if dy != 0.0 {
        let x = max_x + CALLOUT_GAP;
        drawing = drawing.part(Part::callout(
            Callout::new(at(x, 0.0), at(x, dy))
                .label(label(dy_mm))
                .extension(at(BOX_W, 0.0), at(x, 0.0))
                .extension(at(dx + BOX_W, dy), at(x, dy)),
        ));
    }
    drawing
}

#[cfg(test)]
mod tests {
    use crate::app::state::ModalId;
    use signex_types::schematic::{SelectedItem, SelectedKind};

    fn app_with_selection(count: usize) -> crate::app::Signex {
        let (mut app, _boot) = crate::app::Signex::new();
        app.interaction_state.active_canvas_mut().selected = (0..count)
            .map(|_| SelectedItem::new(uuid::Uuid::now_v7(), SelectedKind::Wire))
            .collect();
        app
    }

    /// The count moves to the header subtitle with a real plural (#66): the
    /// locale groups it as `I18n::count` does. The fields name the axes
    /// without the unit, which is their suffix.
    #[test]
    fn move_selection_names_the_count_and_the_offsets() {
        let app = app_with_selection(1200);
        assert_eq!(
            app.detached_modal_subtitle(ModalId::MoveSelection)
                .as_deref(),
            Some("1,200 items selected")
        );
        assert_eq!(
            app_with_selection(1).move_selection_subtitle(),
            "1 item selected"
        );
        let texts: Vec<String> = crate::layout_probe::lay_out_texts(
            app.view_move_selection_body(),
            iced::Size::new(360.0, 400.0),
        )
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
        for expected in ["ΔX", "ΔY", "Cancel", "Move"] {
            assert!(
                texts.iter().any(|t| t == expected),
                "{expected:?} not in {texts:?}"
            );
        }
    }

    /// The help line counts whole grid steps per axis and says "off grid"
    /// for a part step.
    #[test]
    fn the_help_line_counts_grid_steps() {
        let mut app = app_with_selection(3);
        app.ui_state.unit = signex_types::coord::Unit::Mm;
        app.ui_state.grid_size_mm = 1.27;
        assert_eq!(
            app.move_selection_help(5.08, -2.54),
            "Positive X moves right, positive Y moves up. Grid 1.27 mm: 4 × 2 steps."
        );
        assert_eq!(
            app.move_selection_help(1.0, 0.0),
            "Positive X moves right, positive Y moves up. Grid 1.27 mm: off grid."
        );
    }
}
