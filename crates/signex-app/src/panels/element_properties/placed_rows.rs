//! The row builders of a placed object's Properties (#2418): Position,
//! Rotation, Size, the lock note and the Parameters rows. Every per-kind view
//! builds its Location section from these, so a part, a label, a sheet
//! symbol and a junction edit their geometry the same way. Position and Size
//! are one row each, an X / Y (W / H) pair with the panel's unit inside the
//! fields and Y from the page origin; Rotation is the 4-step control
//! (#2523). A locked object shows sunken read-only values under the lock's
//! reason.

use super::super::*;
use super::rows::{pair_row, rotation_row, sunken_value, unit_input, unit_suffix};
use crate::panels::{PlacedEditMsg, PlacedField, PlacedPanel};
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{MessageId, Translatable, UserText, ids, t};
use signex_types::measure::length_field_text;
use signex_types::schematic::{SelectedItem, SelectedKind};
use signex_widgets::Label;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The colours the rows draw with.
#[derive(Clone, Copy)]
pub(super) struct RowLook {
    pub muted: Color,
    pub input_bg: Color,
}

/// One editable box: the draft while typed, else the shown value. Enter
/// commits it; a locked object gets the read-only value box instead.
pub(super) fn edit_box<'a>(
    label: Label<'a>,
    item: SelectedItem,
    field: PlacedField,
    shown: String,
    placed: &PlacedPanel,
    look: RowLook,
) -> Element<'a, PanelMsg> {
    if placed.locked.is_some() {
        return form_input_row(label, &shown, look.muted, look.input_bg);
    }
    let value = placed.text(item, &field, shown);
    let submitted = PanelMsg::PlacedEdit(PlacedEditMsg::Submitted {
        item,
        field: field.clone(),
    });
    let input = signex_widgets::text_input(signex_widgets::raw(""), value)
        .on_input(move |text| {
            PanelMsg::PlacedEdit(PlacedEditMsg::Typed {
                item,
                field: field.clone(),
                text,
            })
        })
        .on_submit(submitted)
        .size(TS.body)
        .padding(SP.s)
        .style(crate::panels::form_look::text_input_style)
        .width(Length::FillPortion(PROPERTY_CONTROL_PORTION));
    container(
        row![property_label(label, look.muted), input]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// The lock's reason, above the disabled rows; nothing when unlocked.
pub(super) fn push_lock_note<'a>(
    mut c: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    look: RowLook,
) -> Column<Element<'a, PanelMsg>> {
    if let Some(reason) = &ctx.placed.locked {
        let note = t!(ctx.i18n, ids::PANELS_PLACED_LOCKED, "reason" => reason.as_str());
        c = c.push(
            container(text(note).size(TS.caption).color(look.muted))
                .padding([SP.s, SP.m])
                .width(Length::Fill)
                .boxed(),
        );
    }
    c
}

/// One field of a Position or Size pair: the draft while typed, else the
/// shown value; Enter commits it. A locked object shows the sunken value.
fn pair_field<'a>(
    axis: Label<'a>,
    item: SelectedItem,
    field: PlacedField,
    shown: String,
    unit: Label<'a>,
    placed: &PlacedPanel,
) -> Element<'a, PanelMsg> {
    if placed.locked.is_some() {
        return sunken_value(Some(axis), shown, Some(unit));
    }
    let value = placed.text(item, &field, shown);
    let submitted = PanelMsg::PlacedEdit(PlacedEditMsg::Submitted {
        item,
        field: field.clone(),
    });
    unit_input(
        Some(axis),
        value,
        Some(unit),
        move |text| {
            PanelMsg::PlacedEdit(PlacedEditMsg::Typed {
                item,
                field: field.clone(),
                text,
            })
        },
        submitted,
    )
}

/// A row of two length fields of `item`, each `(axis, field, shown)`.
fn length_pair_row<'a>(
    ctx: &'a PanelContext,
    label: MessageId,
    item: SelectedItem,
    [
        (first_axis, first, first_shown),
        (second_axis, second, second_shown),
    ]: [(MessageId, PlacedField, String); 2],
) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let unit = || Label::from(unit_suffix(i18n, ctx.unit));
    let axis = |id| Label::from(t!(i18n, id));
    pair_row(
        t!(i18n, label),
        pair_field(
            axis(first_axis),
            item,
            first,
            first_shown,
            unit(),
            &ctx.placed,
        ),
        pair_field(
            axis(second_axis),
            item,
            second,
            second_shown,
            unit(),
            &ctx.placed,
        ),
    )
}

/// The Position row of the selected object: X and Y in one row (Start for
/// a wire or a bus, whose position is its start).
pub(super) fn push_position_rows<'a>(
    c: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    _look: RowLook,
) -> Column<Element<'a, PanelMsg>> {
    let placed = &ctx.placed;
    let (Some(item), Some(geometry)) = (placed.item, placed.geometry) else {
        return c;
    };
    let unit = ctx.unit;
    let label = if matches!(item.kind, SelectedKind::Wire | SelectedKind::Bus) {
        ids::PROPERTIES_START
    } else {
        ids::PROPERTIES_POSITION
    };
    let at = geometry.position;
    let x = length_field_text(at.x, unit);
    let y = length_field_text(ctx.shown_y(at.y), unit);
    c.push(length_pair_row(
        ctx,
        label,
        item,
        [
            (ids::PROPERTIES_AXIS_X, PlacedField::X, x),
            (ids::PROPERTIES_AXIS_Y, PlacedField::Y, y),
        ],
    ))
}

/// The End row of a wire or bus: X and Y in one row, as Start is. Each
/// stretches the end only.
pub(super) fn push_run_end_row<'a>(
    c: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    end: signex_types::schematic::Point,
) -> Column<Element<'a, PanelMsg>> {
    let Some(item) = ctx.placed.item else {
        return c;
    };
    let unit = ctx.unit;
    let x = length_field_text(end.x, unit);
    let y = length_field_text(ctx.shown_y(end.y), unit);
    c.push(length_pair_row(
        ctx,
        ids::PROPERTIES_END,
        item,
        [
            (ids::PROPERTIES_AXIS_X, PlacedField::EndX, x),
            (ids::PROPERTIES_AXIS_Y, PlacedField::EndY, y),
        ],
    ))
}

/// The Size row of a sheet symbol: W and H in one row.
pub(super) fn push_size_rows<'a>(
    c: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    _look: RowLook,
) -> Column<Element<'a, PanelMsg>> {
    let placed = &ctx.placed;
    let (Some(item), Some((w, h))) = (placed.item, placed.geometry.and_then(|g| g.size)) else {
        return c;
    };
    let unit = ctx.unit;
    let (w, h) = (length_field_text(w, unit), length_field_text(h, unit));
    c.push(length_pair_row(
        ctx,
        ids::PROPERTIES_SIZE,
        item,
        [
            (ids::PROPERTIES_AXIS_WIDTH, PlacedField::Width, w),
            (ids::PROPERTIES_AXIS_HEIGHT, PlacedField::Height, h),
        ],
    ))
}

/// The Rotation row of a part or a label: the 4-step control, the stored
/// angle shown as its nearest step; read-only while the object is locked.
pub(super) fn push_rotation_row<'a>(
    c: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    _look: RowLook,
) -> Column<Element<'a, PanelMsg>> {
    let placed = &ctx.placed;
    let (Some(item), Some(rotation)) = (placed.item, placed.geometry.and_then(|g| g.rotation))
    else {
        return c;
    };
    c.push(rotation_row(
        &ctx.i18n,
        &ctx.tokens,
        rotation,
        placed.locked.is_some(),
        move |degrees| PanelMsg::PlacedEdit(PlacedEditMsg::RotationPicked { item, degrees }),
    ))
}

/// The Parameters section of a placed part: one editable row per custom
/// parameter, committed on Enter (read-only while the part is locked).
pub(super) fn push_parameters_section<'a>(
    col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    params: Vec<(String, String)>,
    look: RowLook,
) -> Column<Element<'a, PanelMsg>> {
    let header = if params.is_empty() {
        Translatable::new(ids::PANELS_PLACED_PARAMETERS_NONE)
    } else {
        Translatable::new(ids::PANELS_PLACED_PARAMETERS).arg("count", params.len())
    };
    let placed = &ctx.placed;
    col.push(
        ctx.collapsible(
            "sel_parameters",
            header,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                if params.is_empty() {
                    let none = t!(ctx.i18n, ids::PANELS_PLACED_NO_PARAMETERS);
                    return c.push(
                        container(text(none).size(TS.body).color(look.muted))
                            .padding([6.0, SP.m])
                            .boxed(),
                    );
                }
                let Some(item) = placed.item.filter(|i| i.kind == SelectedKind::Symbol) else {
                    for (name, value) in &params {
                        let name = UserText::new(name.clone());
                        c = c.push(form_input_row(name, value, look.muted, look.input_bg));
                    }
                    return c;
                };
                for (name, value) in params {
                    let field = PlacedField::Parameter(name.clone());
                    let label = UserText::new(name).into();
                    c = c.push(edit_box(label, item, field, value, placed, look));
                }
                c
            },
        )
        .boxed(),
    )
}

/// The Location section every placed kind shows: the lock note, Position,
/// and Rotation and Size where the kind has them.
pub(super) fn push_location_section<'a>(
    col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    look: RowLook,
) -> Column<Element<'a, PanelMsg>> {
    if ctx.placed.item.is_none() {
        return col;
    }
    col.push(
        ctx.collapsible(
            "sel_location",
            ids::PANELS_PLACED_LOCATION,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = push_lock_note(c, ctx, look);
                c = push_position_rows(c, ctx, look);
                c = push_rotation_row(c, ctx, look);
                push_size_rows(c, ctx, look)
            },
        )
        .boxed(),
    )
}
