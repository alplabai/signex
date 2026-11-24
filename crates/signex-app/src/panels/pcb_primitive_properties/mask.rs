//! The mask rows of the track and via panels (#2378, mockup rows 95 and
//! 96): what the board stores per track and via, edited live, in
//! collapsible sections that stay closed while the rule decides and name
//! Rule, Manual or Mixed in their header. A setting the selection does not
//! share shows no tile chosen and a blank field. A side with no opening
//! (tented) greys its rows. Expansion reads to three decimals ("0.050",
//! mockup rows 95 and 96).

use iced::widget::{column, container, row};
use iced::{Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, MessageId, ids};
use signex_types::pcb::MaskExpansion;
use signex_widgets::enable_section::dependents_veil;
use signex_widgets::inert::Inert;
use signex_widgets::{CheckboxLabel as _, Label, checkbox, text};

use super::linked::{Segment, segments};
use super::rows::{Look, Section, field as field_input, fixed_mm, labelled, with_tip};
use super::{PcbPrimitiveMsg, PrimitiveCommon, PrimitiveField, PrimitiveFlag, SP, TS, ViaFacts};
use crate::panels::{PanelMsg, form_look};
use crate::styles::ti;

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

/// `content` greyed and deaf to input unless `live`.
pub(super) fn greyed_unless<'a>(
    look: Look<'a>,
    live: bool,
    content: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    match dependents_veil(ti(look.tokens().panel_bg), live) {
        Some(veil) => Inert::new(content, veil).boxed(),
        None => content,
    }
}

/// The header summary of a mask section: Rule, Manual, or Mixed when the
/// selection differs.
pub(super) fn mask_summary(look: Look<'_>, masks: &[Option<MaskExpansion>]) -> Localized {
    let manual: Option<Vec<bool>> = masks
        .iter()
        .map(|m| m.map(|m| m.manual_mm().is_some()))
        .collect();
    match manual.as_deref() {
        Some(all) if all.iter().all(|m| *m) => look.t(ids::PCB_PRIM_MANUAL),
        Some(all) if all.iter().all(|m| !*m) => look.t(ids::PCB_PRIM_RULE),
        _ => look.t(ids::PCB_PRIM_MIXED),
    }
}

/// Whether every mask of `masks` follows the rule: the section opens closed.
fn all_rule(masks: &[Option<MaskExpansion>]) -> bool {
    masks.iter().all(|m| matches!(m, Some(MaskExpansion::Rule)))
}

/// A collapsible section titled `title` and folded by `key`, closed by
/// default while `closed`, with `summary` in its header (mockup row 95).
fn collapsible<'a>(
    look: Look<'a>,
    title: MessageId,
    key: &'static str,
    closed: bool,
    summary: Localized,
) -> Section<'a> {
    Section::new(look, title).collapsible(key, closed, Some(Label::from(summary)))
}

/// The Source (Rule / Manual) and Expansion rows of one mask `field`. No
/// tile is chosen when the selection differs; Expansion is greyed while the
/// rule decides. Both are greyed unless `open` (the side has an opening).
pub(super) fn source_rows<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    field: PrimitiveField,
    mask: Option<MaskExpansion>,
    open: bool,
    s: &mut Section<'a>,
) {
    let live = open && common.is_editable();
    let cell = |id: MessageId, manual: bool| {
        let is_on = mask.is_some_and(|m| m.manual_mm().is_some() == manual);
        Segment {
            label: look.t(id),
            is_on,
            msg: (live && !is_on).then(|| PcbPrimitiveMsg::MaskModePicked {
                target: common.target.clone(),
                field,
                manual,
            }),
        }
    };
    let source = segments(vec![
        cell(ids::PCB_PRIM_RULE, false),
        cell(ids::PCB_PRIM_MANUAL, true),
    ]);
    s.push(
        common,
        ids::PCB_PRIM_SOURCE,
        greyed_unless(look, open, labelled(look, ids::PCB_PRIM_SOURCE, source)),
    );
    // A typed number makes a mixed selection manual; the rule's value is
    // not shown here.
    let manual = mask.and_then(MaskExpansion::manual_mm);
    let typed = manual.is_some() || mask.is_none();
    s.push(
        common,
        ids::PCB_PRIM_EXPANSION,
        greyed_unless(
            look,
            open && typed,
            labelled(
                look,
                ids::PCB_PRIM_EXPANSION,
                field_input(common, field, manual.map(fixed_mm), None, look),
            ),
        ),
    );
}

/// A track's paste or solder mask section (mockup row 95): closed while the rule
/// decides. The solder section leads with Tented Top / Bottom, greyed:
/// tracks do not store tenting yet.
pub(super) fn track_mask_section<'a>(
    look: Look<'a>,
    common: &PrimitiveCommon,
    title: MessageId,
    field: PrimitiveField,
    mask: Option<MaskExpansion>,
) -> Option<Element<'a, PanelMsg>> {
    let key = if field == PrimitiveField::PasteMask {
        "track-paste-mask"
    } else {
        "track-solder-mask"
    };
    let mut s = collapsible(
        look,
        title,
        key,
        all_rule(&[mask]),
        mask_summary(look, &[mask]),
    );
    if field == PrimitiveField::SolderMask {
        let unstored = |side: MessageId| {
            checkbox::<PanelMsg, Theme, iced::Renderer>(false)
                .labelled(look.t(side))
                .size(TS.body)
                .text_size(TS.body)
                .style(form_look::checkbox_style)
                .boxed()
        };
        let sides = row![unstored(ids::PCB_PRIM_TOP), unstored(ids::PCB_PRIM_BOTTOM)]
            .spacing(SP.m)
            .boxed();
        s.push(
            common,
            ids::PCB_PRIM_TENTED,
            labelled(
                look,
                ids::PCB_PRIM_TENTED,
                with_tip(
                    look,
                    greyed_unless(look, false, sides),
                    ids::PCB_PRIM_TRACK_TENT_UNSTORED,
                ),
            ),
        );
    }
    source_rows(look, common, field, mask, true, &mut s);
    s.finish()
}

/// A via's solder mask section (mockup row 96): each side's Source and Expansion,
/// greyed while that side has no opening (tented, or not reached), then
/// From Hole Edge, greyed while neither side has one. Tenting itself is set
/// in Protection.
pub(super) fn via_mask_section<'a>(
    look: Look<'a>,
    f: &'a ViaFacts,
) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let masks = [f.top.expansion, f.bottom.expansion];
    let mut s = collapsible(
        look,
        ids::PCB_PRIM_SOLDER_MASK,
        "via-solder-mask",
        all_rule(&masks),
        mask_summary(look, &masks),
    );
    let sides = [
        (
            ids::PCB_PRIM_TOP,
            PrimitiveField::SolderMaskTop,
            f.top,
            f.reaches.0,
        ),
        (
            ids::PCB_PRIM_BOTTOM,
            PrimitiveField::SolderMaskBottom,
            f.bottom,
            f.reaches.1,
        ),
    ];
    let mut any_open = false;
    for (side, field, facts, reaches) in sides {
        let open = reaches && facts.tented != Some(true);
        any_open |= open;
        s.push(common, side, subhead(look, side));
        source_rows(look, common, field, facts.expansion, open, &mut s);
    }
    let hole_edge = container(flag_box(
        look,
        common,
        PrimitiveFlag::FromHoleEdge,
        ids::PCB_PRIM_FROM_HOLE_EDGE,
        f.from_hole_edge,
        ids::PCB_PRIM_FROM_HOLE_EDGE_TIP,
    ))
    .padding([SP.xs, super::rows::PAD_X])
    .width(Length::Fill)
    .boxed();
    s.push(
        common,
        ids::PCB_PRIM_FROM_HOLE_EDGE,
        greyed_unless(look, any_open, hole_edge),
    );
    s.push(
        common,
        ids::PCB_PRIM_TENTING_MOVED,
        super::stack_rows::caption(look, look.t(ids::PCB_PRIM_TENTING_MOVED)),
    );
    s.finish()
}

/// A small caption heading the rows of one side or group.
pub(super) fn subhead<'a>(look: Look<'a>, id: MessageId) -> Element<'a, PanelMsg> {
    column![text(look.t(id)).size(TS.dense).color(look.muted)]
        .padding([SP.xs, super::rows::PAD_X])
        .width(Length::Fill)
        .boxed()
}
