//! Mockup row 95: Start and End each keep X and Y on one row at the default
//! 300 px dock, with room for the value, and stack at a narrow dock.

use iced::{Size, Widget as _};
use pcb_geom::ItemRef;
use signex_i18n::{UserText, ids};
use signex_widgets::text;
use uuid::Uuid;

use super::drawing::fixture;
use super::linked::{prefixed, two_fields, unit};
use super::rows::{Look, labelled};
use super::{PrimitiveField as F, TS};
use crate::app::Signex;
use crate::layout_probe::{lay_out_inputs, natural};
use crate::panels::{PanelContext, PanelMsg};

const VALUE: &str = "12.704";
/// The row width inside a 300 px dock, its scrollbar taken off.
const DEFAULT_ROW: f32 = 288.0;
const NARROW_ROW: f32 = 220.0;

fn inputs(ctx: &PanelContext, width: f32) -> Vec<iced::Rectangle> {
    let look = Look::of(ctx);
    let common = fixture::common(vec![ItemRef::Via(Uuid::from_u128(1))]);
    let row = labelled(
        look,
        ids::PCB_PRIM_POINT_START,
        two_fields(
            prefixed(look, &common, F::StartX, Some(12.704), ids::PCB_PRIM_X),
            prefixed(look, &common, F::StartY, Some(12.704), ids::PCB_PRIM_Y),
            unit(look, ids::PCB_PRIM_UNIT_MM),
        ),
    );
    lay_out_inputs(row, Size::new(width, 200.0))
        .into_iter()
        .map(|input| input.bounds)
        .collect()
}

#[test]
fn x_and_y_share_a_row_at_the_default_dock_and_stack_when_narrow() {
    let (app, _boot) = Signex::new();
    let ctx = app.document_state.panel_ctx.clone();
    let value = natural::<PanelMsg>(text(UserText::new(VALUE)).size(TS.body).boxed(), VALUE);

    let wide = inputs(&ctx, DEFAULT_ROW);
    assert_eq!(wide.len(), 2, "{wide:?}");
    assert_eq!(wide[0].y, wide[1].y, "one row: {wide:?}");
    for field in &wide {
        assert!(field.width >= value.width, "{field:?} for {value:?}");
    }

    let narrow = inputs(&ctx, NARROW_ROW);
    assert!(narrow[1].y > narrow[0].y, "stacked: {narrow:?}");
}
