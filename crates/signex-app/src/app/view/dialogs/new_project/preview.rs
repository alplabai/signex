//! The preview pane: the stack-up on the board ground, the board and sheet
//! lines and the note.

use iced::widget::{Space, column, container, row, scrollable};
use iced::{Alignment, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens, theme_tokens};
use signex_widgets::text;

use super::{PREVIEW_W, SP, TS};
use crate::app::new_project::{NewProjectMsg, NewProjectState};
use crate::styles::{Radii, ti, tokens};

/// Width of a stack-up bar.
const BAR_W: f32 = 120.0;
/// Tallest the stack-up shows before it scrolls.
const STACK_MAX_H: f32 = 300.0;

/// What a stack-up row is, which decides its colour and height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app::view) enum Layer {
    Silk,
    Mask,
    TopCopper,
    InnerCopper(u8),
    BottomCopper,
    Prepreg,
    Core,
    MaskBottom,
}

/// The rows of the stack-up for `copper` copper layers, top to bottom: silk,
/// mask, the copper layers with a dielectric between each pair, mask.
pub(in crate::app::view) fn stack_rows(copper: u8) -> Vec<Layer> {
    let copper = copper.max(1);
    let mut rows = vec![Layer::Silk, Layer::Mask];
    for i in 0..copper {
        rows.push(match i {
            0 => Layer::TopCopper,
            i if i == copper - 1 => Layer::BottomCopper,
            i => Layer::InnerCopper(i),
        });
        if i + 1 < copper {
            // Prepreg against the outer copper, core between the inner ones.
            let outer = i == 0 || i + 2 == copper;
            rows.push(if outer { Layer::Prepreg } else { Layer::Core });
        }
    }
    rows.push(Layer::MaskBottom);
    rows
}

/// Height of a dielectric bar: thinner as the board gets more layers.
fn dielectric_h(copper: u8) -> f32 {
    (30.0 / f32::from(copper.saturating_sub(1).max(1)))
        .round()
        .max(3.0)
}

/// Fabrication colours of the board materials; the ground is the board
/// canvas, the same in both Graphite themes.
struct Materials {
    silk: Color,
    mask: Color,
    copper: Color,
    dielectric: Color,
}

const MATERIALS: Materials = Materials {
    silk: Color::from_rgb(0.91, 0.91, 0.91),
    mask: Color::from_rgb(0.18, 0.48, 0.26),
    copper: Color::from_rgb(0.79, 0.54, 0.23),
    dielectric: Color::from_rgb(0.36, 0.35, 0.27),
};

fn bar<'a>(color: Color, height: f32) -> Element<'a, NewProjectMsg> {
    container(Space::new())
        .width(Length::Fixed(BAR_W))
        .height(Length::Fixed(height))
        .style(move |_: &Theme| container::Style {
            background: Some(color.into()),
            border: iced::Border {
                radius: 1.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

fn layer_row<'a>(layer: Layer, copper: u8, i18n: &I18n, ink: Color) -> Element<'a, NewProjectMsg> {
    let (color, height, name): (Color, f32, Localized) = match layer {
        Layer::Silk => (
            MATERIALS.silk,
            3.0,
            t!(i18n, ids::DIALOG_NEW_PROJECT_STACK_SILK_TOP),
        ),
        Layer::Mask => (
            MATERIALS.mask,
            4.0,
            t!(i18n, ids::DIALOG_NEW_PROJECT_STACK_MASK_TOP),
        ),
        Layer::TopCopper => (
            MATERIALS.copper,
            5.0,
            t!(i18n, ids::DIALOG_NEW_PROJECT_STACK_COPPER_TOP),
        ),
        Layer::InnerCopper(n) => (
            MATERIALS.copper,
            5.0,
            t!(i18n, ids::DIALOG_NEW_PROJECT_STACK_COPPER_INNER, "n" => u32::from(n)),
        ),
        Layer::BottomCopper => (
            MATERIALS.copper,
            5.0,
            t!(i18n, ids::DIALOG_NEW_PROJECT_STACK_COPPER_BOTTOM),
        ),
        Layer::Prepreg => (
            MATERIALS.dielectric,
            dielectric_h(copper),
            t!(i18n, ids::DIALOG_NEW_PROJECT_STACK_PREPREG),
        ),
        Layer::Core => (
            MATERIALS.dielectric,
            dielectric_h(copper),
            t!(i18n, ids::DIALOG_NEW_PROJECT_STACK_CORE),
        ),
        Layer::MaskBottom => (
            MATERIALS.mask,
            4.0,
            t!(i18n, ids::DIALOG_NEW_PROJECT_STACK_MASK_BOTTOM),
        ),
    };
    row![
        bar(color, height),
        text(name)
            .size(TS.caption)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None)
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}

/// The preview column.
pub(super) fn pane<'a>(
    state: &NewProjectState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, NewProjectMsg> {
    let ground = crate::styles::board_canvas_colors().background;
    let ground = ti(ground);
    let ink = tokens::resolve(&theme_tokens(ThemeId::GraphiteDark)).ink3;
    let copper = state.setup.pcb.copper_layers;
    let rows = stack_rows(copper);
    // Rows are about a caption line high; a tall stack scrolls.
    let natural = rows.len() as f32 * (TS.caption * 1.4 + 3.0);
    let stack_h = if natural > STACK_MAX_H {
        Length::Fixed(STACK_MAX_H)
    } else {
        Length::Shrink
    };
    let mut stack = column![].spacing(3.0);
    for layer in rows {
        stack = stack.push(layer_row(layer, copper, i18n, ink));
    }
    let board = container(scrollable(stack).width(Length::Fill).height(stack_h))
        .padding([SP.xl, SP.l])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(ground.into()),
            border: iced::Border {
                radius: Radii::GRAPHITE.card.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });
    let paper = state.setup.sheet_line();
    let thickness = format!("{} mm", state.setup.pcb.thickness_mm);
    let bold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let board_line = row![
        text(t!(i18n, ids::DIALOG_NEW_PROJECT_PREVIEW_LAYERS, "count" => u32::from(copper)))
            .size(TS.label)
            .font(bold)
            .color(ti(tokens.text)),
        text(t!(i18n, ids::DIALOG_NEW_PROJECT_PREVIEW_BOARD, "thickness" => thickness))
            .size(TS.label)
            .color(ti(tokens.text)),
    ]
    .spacing(SP.s);
    let sheet_line = text(t!(
        i18n,
        ids::DIALOG_NEW_PROJECT_PREVIEW_SHEET,
        "paper" => paper
    ))
    .size(TS.label)
    .color(ti(tokens.text));
    let fill = tokens::resolve(tokens).sunken;
    let line = ti(tokens.border);
    container(
        column![
            text(t!(i18n, ids::DIALOG_NEW_PROJECT_PREVIEW_HEADING))
                .size(TS.section)
                .color(tokens::resolve(tokens).ink4),
            board,
            column![board_line, sheet_line].spacing(SP.s),
            Space::new().height(Length::Fill),
            text(t!(i18n, ids::DIALOG_NEW_PROJECT_PREVIEW_NOTE))
                .size(TS.body)
                .color(tokens::muted_text(tokens)),
        ]
        .spacing(SP.l)
        .height(Length::Fill),
    )
    .padding([SP.xl + SP.s, SP.xl])
    .width(Length::Fixed(PREVIEW_W))
    .height(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(fill.into()),
        border: iced::Border {
            width: 1.0,
            color: line,
            ..iced::Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}
