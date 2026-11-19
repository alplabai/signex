//! The Properties of the placed kinds with no richer surface of their own
//! (#2418): wire, bus, bus entry, junction, no-connect, No-ERC directive and
//! sheet entry. Each shows the shared Location section (editable Position,
//! the lock note) and the facts that kind has, read-only. A wire or bus
//! (#43) holds its whole segment in Location: Start and End, both editable,
//! and the Length derived from them.

use super::super::*;
use super::illustrations::{RunShape, run, show};
use super::placed_rows::{
    RowLook, edit_box, push_location_section, push_lock_note, push_position_rows,
};
use super::rows::PanelStyle;
use crate::panels::PlacedField;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{MessageId, ids, t};
use signex_types::measure::length_field_text;
use signex_types::schematic::SelectedKind;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The info rows `kind` shows under Properties, by their selection-info key
/// and the label they show; its position is the Location section's.
fn facts_of(kind: Option<SelectedKind>) -> &'static [(&'static str, MessageId)] {
    match kind {
        Some(SelectedKind::SheetPin) => &[
            ("Name", ids::PANELS_PLACED_NAME),
            ("Direction", ids::PANELS_PLACED_DIRECTION),
            ("Sheet", ids::PANELS_PLACED_SHEET),
            ("Sheet File", ids::PANELS_PLACED_SHEET_FILE),
        ],
        // A wire or bus keeps its end and length in Location; a bus entry,
        // junction, no-connect or No-ERC directive is all position; the
        // other kinds have their own views.
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
    if let Some(kind @ (SelectedKind::Wire | SelectedKind::Bus)) = ctx.selected_kind {
        let shape = if kind == SelectedKind::Bus {
            RunShape::Bus
        } else {
            RunShape::Wire
        };
        return push_run_sections(col, ctx, shape, look);
    }
    col = push_location_section(col, ctx, look);
    let facts: Vec<(MessageId, String)> = facts_of(ctx.selected_kind)
        .iter()
        .filter_map(|(key, label)| {
            let (_, value) = ctx.selection_info.iter().find(|(k, _)| k == key)?;
            Some((*label, value.clone()))
        })
        .collect();
    if facts.is_empty() {
        return col;
    }
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    col.push(
        ctx.collapsible(
            "sel_general",
            ids::PANELS_PLACED_PROPERTIES,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                for (label, value) in &facts {
                    c = c.push(prop_kv_row(t!(i18n, *label), value, muted, primary));
                }
                c
            },
        )
        .boxed(),
    )
}

/// A wire or bus: the caption, the illustration, then Location with Start
/// X / Y, End X / Y (both editable, one undo step each) and the read-only
/// Length derived from them.
fn push_run_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    shape: RunShape,
    look: RowLook,
) -> Column<Element<'a, PanelMsg>> {
    let placed = &ctx.placed;
    let (Some(item), Some(geometry), Some(end)) = (placed.item, placed.geometry, placed.end) else {
        return push_location_section(col, ctx, look);
    };
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let unit = ctx.unit;
    let unit_name = unit.to_string();
    let start = geometry.position;
    let length_mm = (end.x - start.x).hypot(end.y - start.y);
    let length = format!("{} {unit_name}", length_field_text(length_mm, unit));
    let width = ctx.selected_wire_style.map(|(w, _)| w).unwrap_or_default();

    col = col.push(
        container(
            text(t!(i18n, ids::PROPERTIES_LENGTH_DERIVED))
                .size(TS.caption)
                .color(look.muted),
        )
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .boxed(),
    );
    col = col.push(show(run(shape, width, length.clone(), i18n), ctx));
    col.push(
        ctx.collapsible(
            "sel_location",
            ids::PANELS_PLACED_LOCATION,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = push_lock_note(c, ctx, look);
                c = push_position_rows(c, ctx, look);
                let end_x = t!(i18n, ids::PROPERTIES_END_X, "unit" => unit_name.as_str());
                let end_y = t!(i18n, ids::PROPERTIES_END_Y, "unit" => unit_name.as_str());
                let x = length_field_text(end.x, unit);
                let y = length_field_text(ctx.shown_y(end.y), unit);
                c = c.push(edit_box(
                    end_x.into(),
                    item,
                    PlacedField::EndX,
                    x,
                    placed,
                    look,
                ));
                c = c.push(edit_box(
                    end_y.into(),
                    item,
                    PlacedField::EndY,
                    y,
                    placed,
                    look,
                ));
                c.push(form_input_row(
                    t!(i18n, ids::PANELS_PLACED_LENGTH),
                    &length,
                    look.muted,
                    look.input_bg,
                ))
            },
        )
        .boxed(),
    )
}
