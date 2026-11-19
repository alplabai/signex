//! The Snap & Grid page: the grid and snap illustration, Snap Options and
//! the Grid Manager.

use iced::Element;
use iced::widget::Column;

use super::Ctx;
use crate::panels::PanelMsg;

pub(super) fn view<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    col = col.push(super::super::illustration::view_illustration(
        super::snap_illustration::illustration(cx.fp, cx.i18n),
        &cx.fp.illustration_hover,
        cx.tokens,
    ));
    col = super::super::snap_options::view_snap_options(col, cx, false);
    super::super::managers::view_grid_manager(col, cx)
}
