//! The selection states of the footprint Properties body: a sketch entity
//! (the Slot header on a slot edge, its role sub-form or its own header,
//! then the Entity section) and a selected pad (the pad form).

use iced::widget::{Column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_sketch::solver::dof::DofColor;

use super::super::{FootprintPadSummary, FootprintSketchEntitySummary, PanelMsg};
use super::body::{Ctx, banner, caption, ghost, is_open, object_header, section};
use super::pad::PadEditTarget;
use super::props_kv_row;
use crate::styles::{PillTone, Spacing};

const SP: Spacing = Spacing::DEFAULT;

const ENTITY_KEY: &str = "fp_sketch_entity";

/// A selected pad outside placement: the pad form, bound to that pad.
pub(super) fn view_selected_pad<'a>(
    col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
    pad: &'a FootprintPadSummary,
) -> Column<Element<'a, PanelMsg>> {
    super::pad::view_pad_form(
        col,
        cx.fp,
        PadEditTarget::Selected(pad.idx),
        cx.palette,
        cx.tokens,
        cx.collapsed,
        cx.i18n,
    )
}

/// The catalog name of a solver DOF state, and the tone it is shown in.
fn dof_state(dof: DofColor) -> (signex_i18n::MessageId, PillTone) {
    match dof {
        DofColor::Under => (ids::EDITORS_SKETCH_UNDER_CONSTRAINED, PillTone::Info),
        DofColor::Full => (ids::EDITORS_FOOTPRINT_DOF_FULLY_CONSTRAINED, PillTone::Ok),
        DofColor::Over => (ids::EDITORS_SKETCH_OVER_CONSTRAINED, PillTone::Err),
    }
}

/// "4 constraints attached", and the entity's DOF state when it has one.
fn entity_status(entity: &FootprintSketchEntitySummary, i18n: &I18n) -> Localized {
    let constraints = entity.attached_constraint_count;
    match entity.dof_state {
        Some(dof) => t!(
            i18n,
            ids::EDITORS_FP_PROPS_ENTITY_STATUS_DOF,
            "constraints" => constraints,
            "state" => t!(i18n, dof_state(dof).0).into_string()
        ),
        None => t!(i18n, ids::EDITORS_FP_PROPS_ENTITY_STATUS, "constraints" => constraints),
    }
}

/// A selected sketch entity: the Slot header when it is a slot edge, then
/// the role sub-form when its role has one (it carries its own header and
/// illustration), else the entity header unless the Slot header stands;
/// then the Entity section.
pub(super) fn view_sketch_entity<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
    entity: &'a FootprintSketchEntitySummary,
) -> Column<Element<'a, PanelMsg>> {
    let Ctx {
        fp, tokens, i18n, ..
    } = cx;
    let kind =
        t!(i18n, ids::EDITORS_FP_PROPS_ENTITY_KIND, "kind" => entity.kind_label).into_string();
    let subform = fp.selected_sketch_entity_id.and_then(|id| {
        super::subforms::view_role_subform(fp, id, cx.palette, tokens, cx.collapsed, i18n)
    });
    // An edge of a slot heads with the slot; the selection stays the edge
    // (mockup row 179).
    let slot = fp.board.as_ref().and_then(|board| board.selected_slot);
    if let Some((length, width)) = slot {
        col = col.push(slot_header(length, width, cx));
    }
    col = match subform {
        Some(form) => col.push(form),
        None if slot.is_some() => col,
        None => col.push(object_header(
            t!(i18n, ids::EDITORS_FP_PROPS_ENTITY_HEADING, "kind" => kind.as_str()),
            Some(entity_status(entity, i18n).into()),
            tokens,
        )),
    };
    let title = t!(i18n, ids::EDITORS_FP_PROPS_SECTION_ENTITY);
    col = col.push(section(
        ENTITY_KEY,
        &title,
        None,
        cx.collapsed,
        tokens,
        i18n,
    ));
    if !is_open(ENTITY_KEY, cx.collapsed) {
        return col;
    }
    entity_rows(col, cx, entity, kind)
}

/// "Slot" over "12.00 × 3.00 mm".
fn slot_header<'a>(length: f64, width: f64, cx: Ctx<'a, '_>) -> Element<'a, PanelMsg> {
    let size = t!(
        cx.i18n, ids::SKETCH_BAKE_SLOT_SIZE,
        "length" => format!("{length:.2}"), "width" => format!("{width:.2}")
    );
    object_header(
        t!(cx.i18n, ids::BOARD_SKETCH_SLOT),
        Some(size.into()),
        cx.tokens,
    )
}

fn entity_rows<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
    entity: &'a FootprintSketchEntitySummary,
    kind: String,
) -> Column<Element<'a, PanelMsg>> {
    use crate::library::messages::RoleTag;
    let Ctx {
        fp, tokens, i18n, ..
    } = cx;
    let (muted, input_bg) = (cx.palette.muted, cx.palette.input_bg);
    let key = |id: signex_i18n::MessageId| t!(i18n, id).into_string();
    col = props_kv_row(col, muted, input_bg, &key(ids::EDITORS_FP_PROPS_KIND), kind);
    if let Some(id) = fp.selected_sketch_entity_id {
        // The host's roles (#2518): a board offers its five bake targets, a
        // footprint its fifteen roles.
        let host = fp.sketch_host;
        let labels = crate::library::editor::footprint::sketch_mode::roles::role_labels(host, i18n);
        let label_of = move |r: &RoleTag| {
            labels
                .iter()
                .find(|(role, _)| role == r)
                .map(|(_, label)| label.clone())
                .unwrap_or_default()
        };
        col = col.push(super::rows::pick_row_labelled(
            &key(ids::EDITORS_FP_PROPS_ROLE),
            host.roles(),
            fp.selected_sketch_role,
            label_of,
            move |role| PanelMsg::FpEditorSetRole { id, role },
            muted,
        ));
        if !fp.selected_sketch_is_point && host.offers_role(RoleTag::Pad) {
            col = col.push(caption(
                t!(i18n, ids::EDITORS_FOOTPRINT_ROLE_PAD_HINT),
                tokens,
            ));
        }
    }
    if let Some([x, y]) = entity.position_mm {
        let x_label = key(ids::EDITORS_PAD_POSITION_X);
        let y_label = key(ids::EDITORS_PAD_POSITION_Y);
        col = props_kv_row(col, muted, input_bg, &x_label, format!("{x:.3}"));
        col = props_kv_row(col, muted, input_bg, &y_label, format!("{y:.3}"));
    }
    let construction = if entity.construction {
        ids::EDITORS_FP_PROPS_YES
    } else {
        ids::EDITORS_FP_PROPS_NO
    };
    col = props_kv_row(
        col,
        muted,
        input_bg,
        &key(ids::EDITORS_FP_PROPS_CONSTRUCTION),
        key(construction),
    );
    col = props_kv_row(
        col,
        muted,
        input_bg,
        &key(ids::EDITORS_FP_PROPS_ATTACHED),
        i18n.count(entity.attached_constraint_count as i64),
    );
    // An arc of a rounded-rectangle pad's corner can take its own radius:
    // the Properties face of the canvas's "Unlink radius".
    if entity.kind_label == "Arc"
        && let Some(id) = fp.selected_sketch_entity_id
    {
        col = col.push(
            container(ghost(
                t!(i18n, ids::EDITORS_FOOTPRINT_UNLINK_CORNER_RADIUS),
                Some(PanelMsg::FpEditorUnlinkCornerRadius { arc_entity_id: id }),
                tokens,
            ))
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .boxed(),
        );
    }
    if let Some(dof) = entity.dof_state {
        let (_, tone) = dof_state(dof);
        let text = match dof {
            DofColor::Under => ids::EDITORS_FP_PROPS_BANNER_UNDER,
            DofColor::Full => ids::EDITORS_FP_PROPS_BANNER_FULL,
            DofColor::Over => ids::EDITORS_FP_PROPS_BANNER_OVER,
        };
        col = col.push(banner(t!(i18n, text), tone, tokens));
    }
    col
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
