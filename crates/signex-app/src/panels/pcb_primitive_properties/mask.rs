//! The lock and mask rows of the track and via panels (#2378): what the
//! board stores per track and via, edited live. A setting the selection does
//! not share shows blank (a mixed box, no tile chosen).

use iced::widget::{column, row};
use iced::{Element, Length, Theme, Widget as _};
use signex_i18n::{MessageId, ids};
use signex_types::pcb::MaskExpansion;
use signex_widgets::{CheckboxLabel as _, button, checkbox, text};

use super::rows::{Look, Section, labelled, mm_field, value_box, with_tip};
use super::{PcbPrimitiveMsg, PrimitiveCommon, PrimitiveField, PrimitiveFlag, SP, TS};
use crate::panels::{PanelMsg, form_look};

/// A check box for `flag`, labelled `label`, with `tip`. The Lock box acts
/// while the selection is locked; every other box only while it is editable.
pub(super) fn flag_box<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    flag: PrimitiveFlag,
    label: MessageId,
    checked: Option<bool>,
    tip: MessageId,
) -> Element<'a, PanelMsg> {
    let live = if flag == PrimitiveFlag::Locked {
        common.can_toggle_lock()
    } else {
        common.is_editable()
    };
    let mut boxed = checkbox::<PanelMsg, Theme, iced::Renderer>(checked.unwrap_or(false))
        .labelled(look.t(label))
        .size(TS.body)
        .text_size(TS.body)
        .style(if checked.is_none() {
            form_look::mixed_checkbox_style
        } else {
            form_look::checkbox_style
        });
    if live {
        let target = common.target.clone();
        boxed = boxed.on_toggle(move |on| {
            PanelMsg::PcbPrimitive(PcbPrimitiveMsg::FlagToggled {
                target: target.clone(),
                flag,
                on,
            })
        });
    }
    with_tip(look, boxed.boxed(), tip)
}

/// The Lock row of a track or via.
pub(super) fn lock_row<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    locked: Option<bool>,
) -> Element<'a, PanelMsg> {
    labelled(
        look,
        ids::PCB_PRIM_LOCK,
        flag_box(
            look,
            common,
            PrimitiveFlag::Locked,
            ids::PCB_PRIM_LOCK,
            locked,
            ids::PCB_PRIM_LOCK_TIP,
        ),
    )
}

/// Rule / Manual tiles and, under them, the manual expansion in mm (blank
/// while the rule decides or the selection differs; typing a number makes it
/// manual).
pub(super) fn mask_value<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    field: PrimitiveField,
    mask: Option<MaskExpansion>,
) -> Element<'a, PanelMsg> {
    let tile = |id: MessageId, manual: bool| {
        let active = mask.is_some_and(|m| m.manual_mm().is_some() == manual);
        let mut b = button(text(look.t(id)).size(TS.body))
            .padding([SP.xs, SP.s])
            .style(form_look::option_tile(active));
        if common.is_editable() && !active {
            b = b.on_press(PanelMsg::PcbPrimitive(PcbPrimitiveMsg::MaskModePicked {
                target: common.target.clone(),
                field,
                manual,
            }));
        }
        b.boxed()
    };
    let tiles = row![
        tile(ids::PCB_PRIM_RULE, false),
        tile(ids::PCB_PRIM_MANUAL, true)
    ]
    .spacing(SP.xs);
    let value = match mask {
        Some(MaskExpansion::Rule) => value_box(look, look.t(ids::PCB_PRIM_RULE)),
        Some(MaskExpansion::Manual { mm }) => mm_field(common, field, Some(mm), look),
        None => mm_field(common, field, None, look),
    };
    column![tiles, value]
        .spacing(SP.xs)
        .width(Length::Fill)
        .boxed()
}

/// A track's paste or solder mask section: one expansion row.
pub(super) fn track_mask_section<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    title: MessageId,
    field: PrimitiveField,
    mask: Option<MaskExpansion>,
) -> Option<Element<'a, PanelMsg>> {
    let mut s = Section::new(look, title);
    s.push(
        common,
        ids::PCB_PRIM_EXPANSION,
        labelled(
            look,
            ids::PCB_PRIM_EXPANSION,
            mask_value(look, common, field, mask),
        ),
    );
    s.finish()
}

/// A via's solder mask section: each side's expansion and tenting, then
/// From Hole Edge.
pub(super) fn via_mask_section<'a>(
    look: Look<'a>,
    f: &'a super::ViaFacts,
) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_SOLDER_MASK);
    let sides = [
        (
            ids::PCB_PRIM_TOP,
            PrimitiveField::SolderMaskTop,
            PrimitiveFlag::TentedTop,
            f.top,
        ),
        (
            ids::PCB_PRIM_BOTTOM,
            PrimitiveField::SolderMaskBottom,
            PrimitiveFlag::TentedBottom,
            f.bottom,
        ),
    ];
    for (side, field, tent, facts) in sides {
        s.push(
            common,
            side,
            labelled(look, side, mask_value(look, common, field, facts.expansion)),
        );
        s.push(
            common,
            ids::PCB_PRIM_TENTED,
            labelled(
                look,
                ids::PCB_PRIM_TENTED,
                flag_box(
                    look,
                    common,
                    tent,
                    side,
                    facts.tented,
                    ids::PCB_PRIM_TENTED_TIP,
                ),
            ),
        );
    }
    s.push(
        common,
        ids::PCB_PRIM_FROM_HOLE_EDGE,
        labelled(
            look,
            ids::PCB_PRIM_FROM_HOLE_EDGE,
            flag_box(
                look,
                common,
                PrimitiveFlag::FromHoleEdge,
                ids::PCB_PRIM_FROM_HOLE_EDGE,
                f.from_hole_edge,
                ids::PCB_PRIM_FROM_HOLE_EDGE_TIP,
            ),
        ),
    );
    s.finish()
}
