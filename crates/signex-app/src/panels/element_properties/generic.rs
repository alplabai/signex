//! Read-only Properties of an element kind with no dedicated surface.

use super::super::*;
use super::rows::PanelStyle;
use iced::Widget;

pub(super) fn push_generic_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let PanelStyle { muted, primary, .. } = st;
    // Generic read-only properties for other types
    // `Locked` is the checkbox row below, not a read-only line.
    let info: Vec<(String, String)> = ctx
        .selection_info
        .iter()
        .filter(|(k, _)| k != "Type" && k != "Locked")
        .cloned()
        .collect();
    col = col.push(
        ctx.collapsible("sel_general", "Properties", &ctx.collapsed_sections, || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            for (key, value) in &info {
                c = c.push(prop_kv_row(key, value, muted, primary));
            }
            c
        })
        .boxed(),
    );
    col
}
