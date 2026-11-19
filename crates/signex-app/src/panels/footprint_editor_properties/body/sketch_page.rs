//! The Sketch page: the solve state, the sketch parameters and the solve
//! warnings, and outside Sketch mode a pointer to the sketch itself.

use iced::Element;
use iced::widget::Column;
use signex_i18n::{ids, t};

use super::Ctx;
use super::header::caption;
use crate::panels::{FootprintModeKind, PanelMsg};

pub(super) fn view<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    col = super::super::sections::view_sketch_sections(col, cx);
    if cx.fp.mode_kind != FootprintModeKind::Sketch {
        col = col.push(caption(
            t!(cx.i18n, ids::EDITORS_FP_PROPS_OPEN_SKETCH),
            cx.tokens,
        ));
    }
    col
}
