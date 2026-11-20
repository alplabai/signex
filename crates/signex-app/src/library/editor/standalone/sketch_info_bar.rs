//! The sketch info bar (m187, m179): what is being sketched, on which plane
//! and at which grid step, and the Parameters button that opens the
//! parameters table. One bar on both hosts, at the canvas's top right under
//! the mode switcher, clear of the DOF chips at the top left.

use iced::widget::{Space, column, container, row};
use iced::{Alignment, Border, Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_sketch::plane::PlaneKind;
use signex_sketch::unit::Quantity;
use signex_types::sketch::SketchHost;
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, theme_ext};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::parameters_table::SketchParametersMsg;
use crate::library::editor::footprint::parameters_table::state::display_value;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::styles::{DialogButtonKind, Radii, dialog_button};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The bar's distance from the canvas top, in body text heights: under the
/// mode switcher on the same edge.
const TOP_CLEARANCE_EM: f32 = 3.5;

/// The plane new sketch geometry goes on, as the caption's selector: the
/// first plane, Top when the sketch has none yet.
fn plane_key(editor: &FootprintDocument) -> &'static str {
    let first = editor
        .primitive()
        .sketch
        .as_ref()
        .and_then(|s| s.planes.first());
    match first.map(|p| &p.kind) {
        Some(PlaneKind::BodyTop { .. }) => "body",
        Some(PlaneKind::BoardTop) | None => "top",
    }
}

/// The bar of `editor` in Sketch mode.
pub fn overlay<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let caption_id = match editor.state.sketch_host() {
        SketchHost::Footprint => ids::SKETCH_INFO_FOOTPRINT,
        SketchHost::Board => ids::SKETCH_INFO_BOARD,
    };
    let (grid, _) = display_value(Quantity::length(editor.state.snap_options.grid_step_mm));
    let caption = t!(
        i18n,
        caption_id,
        "plane" => plane_key(editor),
        "grid" => grid
    );
    let parameters = LibraryMessage::PrimitiveEditorEvent {
        path: editor.path.clone(),
        msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchParameters(
            SketchParametersMsg::TableToggled,
        )),
    };
    let kind = if editor.state.parameters_table.is_some() {
        DialogButtonKind::Accent
    } else {
        DialogButtonKind::Secondary
    };
    let bar = row![
        text(caption)
            .size(TS.caption)
            .color(theme_ext::text_secondary(tokens)),
        dialog_button(
            t!(i18n, ids::SKETCH_INFO_PARAMETERS),
            kind,
            Some(parameters),
            tokens,
        ),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let fill = theme_ext::to_color(&tokens.panel_bg);
    let line = theme_ext::border_color(tokens);
    let bar = container(bar)
        .padding([SP.xs, SP.m])
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(fill.into()),
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..Default::default()
        });
    container(column![
        Space::new().height(TS.body * TOP_CLEARANCE_EM),
        bar
    ])
    .padding([0.0, SP.m])
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Right)
    .boxed()
}
