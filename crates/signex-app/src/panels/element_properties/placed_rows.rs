//! The row builders of a placed object's Properties (#2418): Position,
//! Rotation, Size, the lock note and the Parameters rows. Every per-kind view
//! builds its Location section from these, so a part, a label, a sheet
//! symbol and a junction edit their geometry the same way. A box shows the
//! panel's unit and the page origin's Y; a locked object's boxes are
//! read-only and the lock's reason is shown above them.

use super::super::*;
use crate::panels::{PlacedEditMsg, PlacedField, PlacedPanel};
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{Translatable, UserText, ids, t};
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
fn edit_box<'a>(
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

/// The X and Y rows of the selected object (Start X / Start Y for a wire or
/// a bus, whose position is its start).
pub(super) fn push_position_rows<'a>(
    mut c: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    look: RowLook,
) -> Column<Element<'a, PanelMsg>> {
    let placed = &ctx.placed;
    let (Some(item), Some(geometry)) = (placed.item, placed.geometry) else {
        return c;
    };
    let unit = ctx.unit;
    let unit_name = unit.to_string();
    let run = matches!(item.kind, SelectedKind::Wire | SelectedKind::Bus);
    let (x_id, y_id) = if run {
        (ids::PANELS_PLACED_START_X, ids::PANELS_PLACED_START_Y)
    } else {
        (ids::PANELS_PLACED_X, ids::PANELS_PLACED_Y)
    };
    let at = geometry.position;
    let x_label = t!(ctx.i18n, x_id, "unit" => unit_name.as_str()).into();
    let y_label = t!(ctx.i18n, y_id, "unit" => unit_name.as_str()).into();
    let x = length_field_text(at.x, unit);
    let y = length_field_text(ctx.shown_y(at.y), unit);
    c = c.push(edit_box(x_label, item, PlacedField::X, x, placed, look));
    c.push(edit_box(y_label, item, PlacedField::Y, y, placed, look))
}

/// The Width and Height rows of a sheet symbol.
pub(super) fn push_size_rows<'a>(
    mut c: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    look: RowLook,
) -> Column<Element<'a, PanelMsg>> {
    let placed = &ctx.placed;
    let (Some(item), Some((w, h))) = (placed.item, placed.geometry.and_then(|g| g.size)) else {
        return c;
    };
    let unit = ctx.unit;
    let unit_name = unit.to_string();
    let w_label = t!(ctx.i18n, ids::PANELS_PLACED_WIDTH, "unit" => unit_name.as_str()).into();
    let h_label = t!(ctx.i18n, ids::PANELS_PLACED_HEIGHT, "unit" => unit_name.as_str()).into();
    let (w, h) = (length_field_text(w, unit), length_field_text(h, unit));
    c = c.push(edit_box(w_label, item, PlacedField::Width, w, placed, look));
    c.push(edit_box(
        h_label,
        item,
        PlacedField::Height,
        h,
        placed,
        look,
    ))
}

/// A Rotation choice: a right angle and how it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RotationChoice {
    degrees: u32,
    label: String,
}

impl std::fmt::Display for RotationChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// The Rotation row of a part or a label: 0, 90, 180 or 270 degrees, the
/// stored angle shown as the nearest of them.
pub(super) fn push_rotation_row<'a>(
    c: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    look: RowLook,
) -> Column<Element<'a, PanelMsg>> {
    let placed = &ctx.placed;
    let (Some(item), Some(rotation)) = (placed.item, placed.geometry.and_then(|g| g.rotation))
    else {
        return c;
    };
    let choices: Vec<RotationChoice> = [0_u32, 90, 180, 270]
        .into_iter()
        .map(|degrees| RotationChoice {
            degrees,
            label: t!(ctx.i18n, ids::PANELS_PLACED_DEGREES, "degrees" => degrees).to_string(),
        })
        .collect();
    let nearest = ((rotation.rem_euclid(360.0) / 90.0).round() as usize) % 4;
    let current = choices[nearest].clone();
    let label = t!(ctx.i18n, ids::PANELS_PLACED_ROTATION);
    if placed.locked.is_some() {
        return c.push(form_input_row(
            label,
            &current.label,
            look.muted,
            look.input_bg,
        ));
    }
    c.push(form_pick_row(
        label,
        choices,
        current,
        move |choice: RotationChoice| {
            PanelMsg::PlacedEdit(PlacedEditMsg::RotationPicked {
                item,
                degrees: f64::from(choice.degrees),
            })
        },
        look.muted,
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
