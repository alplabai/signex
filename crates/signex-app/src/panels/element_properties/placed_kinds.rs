//! The Properties of the placed kinds with no richer surface of their own
//! (#2418): wire, bus, bus entry, junction, no-connect, No-ERC directive and
//! sheet entry. Each shows the shared Location section (editable Position,
//! the lock note) and the facts that kind has, read-only.

use super::super::*;
use super::placed_rows::{RowLook, push_location_section};
use super::rows::PanelStyle;
use iced::Widget;
use signex_i18n::{MessageId, ids, t};
use signex_types::schematic::SelectedKind;

/// The info rows `kind` shows under Properties, by their selection-info key
/// and the label they show; its position is the Location section's.
fn facts_of(kind: Option<SelectedKind>) -> &'static [(&'static str, MessageId)] {
    match kind {
        Some(SelectedKind::Wire) => &[
            ("End", ids::PANELS_PLACED_END),
            ("Length", ids::PANELS_PLACED_LENGTH),
        ],
        Some(SelectedKind::Bus) => &[("End", ids::PANELS_PLACED_END)],
        Some(SelectedKind::SheetPin) => &[
            ("Name", ids::PANELS_PLACED_NAME),
            ("Direction", ids::PANELS_PLACED_DIRECTION),
            ("Sheet", ids::PANELS_PLACED_SHEET),
            ("Sheet File", ids::PANELS_PLACED_SHEET_FILE),
        ],
        // A bus entry, junction, no-connect or No-ERC directive is all
        // position; the other kinds have their own views.
        _ => &[],
    }
}

pub(super) fn push_placed_kind_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let PanelStyle {
        muted,
        primary,
        palette,
        ..
    } = st;
    let look = RowLook {
        muted,
        input_bg: palette.input_bg,
    };
    col = push_location_section(col, ctx, look);
    let facts: Vec<(String, String)> = facts_of(ctx.selected_kind)
        .iter()
        .filter_map(|(key, label)| {
            let (_, value) = ctx.selection_info.iter().find(|(k, _)| k == key)?;
            Some((t!(ctx.i18n, *label).to_string(), value.clone()))
        })
        .collect();
    if facts.is_empty() {
        return col;
    }
    let title = t!(ctx.i18n, ids::PANELS_PLACED_PROPERTIES).to_string();
    col.push(
        ctx.collapsible("sel_general", &title, &ctx.collapsed_sections, move || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            for (label, value) in &facts {
                c = c.push(prop_kv_row(label, value, muted, primary));
            }
            c
        })
        .boxed(),
    )
}
