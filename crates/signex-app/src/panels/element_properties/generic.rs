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
        ctx.collapsible(
            "sel_general",
            signex_i18n::ids::PROPERTIES_SECTION_PROPERTIES,
            &ctx.collapsed_sections,
            || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                for (key, value) in &info {
                    // raw: the key is worded by signex-engine's selection info (#2060).
                    c = c.push(prop_kv_row(raw(key.clone()), value, muted, primary));
                }
                c
            },
        )
        .boxed(),
    );
    col
}
