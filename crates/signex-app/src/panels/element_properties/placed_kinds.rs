//! The Properties of the placed kinds with no richer surface of their own
//! (#2418): wire, bus, bus entry, junction, no-connect, No-ERC directive and
//! sheet entry. Each shows the shared Location section (editable Position,
//! the lock note) and the facts that kind has, read-only. A wire or bus
//! (mockup row 43) holds its whole segment in Location: Start and End, one
//! X / Y pair row each, each stretching its own end, and the Length derived
//! from them.

use super::super::*;
use super::illustrations::{RunShape, run, show};
use super::placed_rows::{
    RowLook, push_location_section, push_lock_note, push_position_rows, push_run_end_row,
};
use super::rows::{PanelStyle, labelled_row, sunken_value};
use crate::panels::{NetInspectorMsg, RunNet};
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
    if matches!(
        ctx.selected_kind,
        Some(SelectedKind::Wire | SelectedKind::Bus)
    ) {
        return push_run_sections(col, ctx, look);
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

/// A wire or bus: the caption, the illustration, then Location with the
/// Start and End pair rows (each stretches its own end, one undo step per
/// box) and the read-only Length derived from them, then the Net its index
/// names. The illustration lies the way the run lies on the sheet.
fn push_run_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    look: RowLook,
) -> Column<Element<'a, PanelMsg>> {
    let placed = &ctx.placed;
    let (Some(_), Some(geometry), Some(end)) = (placed.item, placed.geometry, placed.end) else {
        return push_location_section(col, ctx, look);
    };
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let unit = ctx.unit;
    let unit_name = unit.to_string();
    let start = geometry.position;
    let length_mm = (end.x - start.x).hypot(end.y - start.y);
    let length = format!("{} {unit_name}", length_field_text(length_mm, unit));
    let width = ctx.selected_wire_style.map(|(w, _)| w).unwrap_or_default();
    let shape = RunShape::of_segment(start, end);

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
    col = col.push(
        ctx.collapsible(
            "sel_location",
            ids::PANELS_PLACED_LOCATION,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = push_lock_note(c, ctx, look);
                c = push_position_rows(c, ctx, look);
                c = push_run_end_row(c, ctx, end);
                c.push(form_input_row(
                    t!(i18n, ids::PANELS_PLACED_LENGTH),
                    &length,
                    look.muted,
                    look.input_bg,
                ))
            },
        )
        .boxed(),
    );
    match &placed.run_net {
        Some(net) => col.push(run_net_section(ctx, net)),
        None => col,
    }
}

/// Net: the read-only Net Name of the run and, when a label or power port
/// names the net, a button that goes to it. A net has no single stored name
/// to edit here; the name is changed on what names it.
fn run_net_section<'a>(ctx: &'a PanelContext, net: &'a RunNet) -> Element<'a, PanelMsg> {
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    ctx.collapsible(
        "sel_run_net",
        ids::PROPERTIES_SECTION_NET,
        &ctx.collapsed_sections,
        move || {
            let value = sunken_value(None, net.name.clone(), None);
            let control = match &net.named_by {
                Some(anchor) => row![value, go_to_label(i18n, &net.name, anchor)]
                    .spacing(SP.s)
                    .align_y(iced::Alignment::Center)
                    .boxed(),
                None => value,
            };
            Column::new()
                .spacing(0)
                .width(Length::Fill)
                .push(labelled_row(t!(i18n, ids::PROPERTIES_NET_NAME), control))
        },
    )
    .boxed()
}

/// The button to the label that names `net`: the Net Inspector's own jump.
fn go_to_label<'a>(
    i18n: &signex_i18n::I18n,
    net: &str,
    anchor: &signex_net::NetAnchor,
) -> Element<'a, PanelMsg> {
    button(text(t!(i18n, ids::PROPERTIES_NET_GO_TO_LABEL)).size(TS.caption))
        .padding([SP.s, SP.m])
        .on_press(PanelMsg::NetInspector(NetInspectorMsg::MemberClicked {
            net: net.to_string(),
            anchor: anchor.clone(),
        }))
        .style(crate::panels::form_look::ghost_button)
        .boxed()
}
