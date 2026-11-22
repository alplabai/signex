//! The Via panel (mockup row 96): Net Information; Definition (net, the via
//! type the via is, its name, span and drill pair, position); Via Stack
//! (the illustration, Diameter, Drill and Finished Hole); Protection
//! (IPC-4761); Solder Mask Expansion; and the rows the board does not store
//! yet.
//!
//! A via picks a via type; span, drill pair, stack and protection come from
//! it. A Diameter or Drill typed away from the type's is this via's override,
//! and typing the type's value clears it. The types are derived from the
//! board's vias until the Layer Stack Manager stores them
//! (`signex_engine::pcb::via_types`), so tenting stays editable here.

use iced::widget::{column, pick_list, row};
use iced::{Element, Length, Widget as _};
use signex_engine::pcb::via_types::ViaTypeKey;
use signex_i18n::{MessageId, UserText, ids, t};
use signex_types::pcb_layer::CopperLayer;
use signex_types::pcb_stack::StackMode;
use signex_types::pcb_via_protection::{Ipc4761, PlugSide, ViaFill};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{Label, button, chips};

use super::drawing::via as drawing;
use super::linked::{Segment, segments};
use super::mask::{flag_box, greyed_unless, via_mask_section};
use super::rows::{Look, Section, labelled, mm_field, net_row, pair, stack, value_box, with_tip};
use super::stack_rows::{
    caption, illustration, later_link, linked, mode_tiles, pill, size_rows, tagged, unstored,
    unstored_row, warn_callout,
};
use super::{
    Editability, PcbPrimitiveMsg, PrimitiveField as F, PrimitiveFlag, PrimitiveTarget, SP, TS,
    ViaFacts, format_number, net_info,
};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::ti;
use crate::styles::tokens::{hover_overlay, resolve};

/// An event of the Via panel's own rows (mockup row 96).
#[derive(Debug, Clone, PartialEq)]
pub enum ViaMsg {
    /// A via type was picked: the vias take its span, sizes and tenting.
    TypePicked {
        target: PrimitiveTarget,
        key: ViaTypeKey,
    },
    /// A reset arrow: `field` (Diameter or Drill) back to the type's value.
    SizeReset { target: PrimitiveTarget, field: F },
    /// A layer chip: the copper layer the illustration's top view shows.
    ViewLayerPicked(CopperLayer),
}

fn via_msg(msg: ViaMsg) -> PanelMsg {
    PanelMsg::PcbPrimitive(PcbPrimitiveMsg::Via(msg))
}

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a ViaFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    let common = &f.common;
    stack(vec![
        net_info::section(look, common),
        definition(look, f),
        via_stack(look, f),
        protection(look, f),
        via_mask_section(look, f),
        not_stored(look, f),
    ])
}

fn definition<'a>(look: Look<'a>, f: &'a ViaFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_DEFINITION).collapsible(
        "pcb_prim.via.definition",
        false,
        None,
    );
    s.push(common, ids::PCB_PRIM_NET, net_row(look, common));
    let live = common.is_editable() && !common.target.is_placement();
    let target = common.target.clone();
    let list = pick_list(
        f.via_type.clone(),
        f.types.as_slice(),
        |c: &super::ViaTypeChoice| c.label.clone(),
    )
    .graphite()
    .padding(SP.s)
    .width(Length::Fill);
    let list = if live {
        list.on_select(move |c: super::ViaTypeChoice| {
            via_msg(ViaMsg::TypePicked {
                target: target.clone(),
                key: c.key,
            })
        })
        .boxed()
    } else {
        list.boxed()
    };
    s.push(
        common,
        ids::PCB_PRIM_VIA_TYPE,
        linked(
            look,
            common,
            drawing::TYPE,
            labelled(
                look,
                ids::PCB_PRIM_VIA_TYPE,
                with_tip(look, list, ids::PCB_PRIM_VIA_TYPE_TIP),
            ),
        ),
    );
    let mut links = column![later_link(
        look,
        look.t(ids::PCB_PRIM_EDIT_IN_LAYER_STACK),
        ids::PCB_PRIM_LAYER_STACK_LATER,
    )];
    if f.types
        .iter()
        .all(|t| t.kind == signex_types::pcb::ViaType::Through)
    {
        links = links.push(later_link(
            look,
            look.t(ids::PCB_PRIM_DEFINE_VIA_TYPES),
            ids::PCB_PRIM_LAYER_STACK_LATER,
        ));
    }
    s.push(
        common,
        ids::PCB_PRIM_VIA_TYPE,
        labelled_blank(look, links.spacing(SP.xs).boxed()),
    );
    let from_type = |value: Option<String>| {
        tagged(
            look,
            value_box(look, UserText::new(value.unwrap_or_default())),
            Some(ids::PCB_PRIM_FROM_TYPE),
        )
    };
    s.push(
        common,
        ids::PCB_PRIM_VIA_NAME,
        labelled(look, ids::PCB_PRIM_VIA_NAME, from_type(f.span.clone())),
    );
    let span = match &f.span_ends {
        (Some(top), Some(bottom)) => Some(
            t!(
                look.ctx.i18n,
                ids::PCB_PRIM_SPAN_ENDS,
                "from" => top.label.clone(),
                "to" => bottom.label.clone()
            )
            .into_string(),
        ),
        _ => None,
    };
    s.push(
        common,
        ids::PCB_PRIM_SPAN,
        linked(
            look,
            common,
            drawing::SPAN,
            labelled(look, ids::PCB_PRIM_SPAN, from_type(span)),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_DRILL_PAIR,
        labelled(
            look,
            ids::PCB_PRIM_DRILL_PAIR,
            from_type(f.drill_pair.clone()),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_POSITION,
        labelled(
            look,
            ids::PCB_PRIM_POSITION,
            pair(
                mm_field(common, F::X, f.x, look),
                mm_field(common, F::Y, f.y, look),
            ),
        ),
    );
    if common.editability == Editability::ItemLocked {
        s.push_row(warn_callout(
            look,
            look.t(ids::PCB_PRIM_VIA_LOCKED),
            Some((
                look.t(ids::PCB_PRIM_UNLOCK_VIA),
                PcbPrimitiveMsg::LockToggled {
                    target: common.target.clone(),
                    locked: false,
                },
            )),
        ));
    }
    s.finish()
}

/// A row with an empty label column, for links under a field.
fn labelled_blank<'a>(look: Look<'a>, value: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
    super::rows::labelled_text(look, UserText::new(String::new()), value)
}

fn via_stack<'a>(look: Look<'a>, f: &'a ViaFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s =
        Section::new(look, ids::PCB_PRIM_VIA_STACK).collapsible("pcb_prim.via.stack", false, None);
    s.push(
        common,
        ids::PCB_PRIM_STACK_SIMPLE,
        mode_tiles(look, common, f.mode, true, ids::PCB_PRIM_STACK_TIP),
    );
    s.push_row(illustration(look, common, drawing::illustration(look, f)));
    if let Some(chips) = layer_chips(look, f) {
        s.push_row(chips);
    }
    if !common.target.is_placement() && f.overridden == Some(true) {
        s.push_row(caption(look, look.t(ids::PCB_PRIM_OVERRIDE_NOTE)));
    }
    let type_size = f.type_size;
    if f.mode == Some(StackMode::Simple) || f.layer_sizes.is_empty() {
        let row = size_row(look, f, F::Diameter, f.diameter, type_size.map(|t| t.0));
        s.push(common, ids::PCB_PRIM_DIAMETER, row);
    } else {
        size_rows(look, common, &f.layer_sizes, false, true, &mut s);
    }
    let row = size_row(look, f, F::HoleSize, f.hole, type_size.map(|t| t.1));
    s.push(common, ids::PCB_PRIM_DRILL, row);
    // The Via tool's form sets the drill; a placed via's finished hole types
    // the drill that leaves it.
    let finished = if common.target.is_placement() {
        let shown = f.finished_hole.map(format_number).unwrap_or_default();
        value_box(look, UserText::new(shown))
    } else {
        mm_field(common, F::FinishedHole, f.finished_hole, look)
    };
    s.push(
        common,
        ids::PCB_PRIM_FINISHED_HOLE,
        labelled(
            look,
            ids::PCB_PRIM_FINISHED_HOLE,
            with_tip(
                look,
                tagged(look, finished, Some(ids::PCB_PRIM_FROM_DRILL)),
                ids::PCB_PRIM_FINISHED_HOLE_TIP,
            ),
        ),
    );
    if let (Some(hole), Some(min)) = (f.hole, f.min_drill)
        && hole < min - 1e-9
    {
        let words = t!(
            look.ctx.i18n,
            ids::PCB_PRIM_MIN_DRILL_WARN,
            "drill" => format_number(hole),
            "min" => format_number(min)
        );
        s.push_row(warn_callout(look, words, None));
    }
    s.finish()
}

/// Diameter or Drill, always typed: "from type" while it is the type's value;
/// bold with a reset arrow while it differs.
fn size_row<'a>(
    look: Look<'a>,
    f: &'a ViaFacts,
    field: F,
    value: Option<f64>,
    of_type: Option<f64>,
) -> Element<'a, PanelMsg> {
    let common = &f.common;
    let differs = match (value, of_type) {
        (Some(v), Some(t)) => (v - t).abs() > 1e-9,
        (None, Some(_)) => true,
        _ => false,
    };
    let own = differs && !common.target.is_placement();
    let input = if own && value.is_some() {
        bold_mm_field(look, f, field, value)
    } else {
        mm_field(common, field, value, look)
    };
    let mut value_row = row![input].spacing(SP.s).align_y(iced::Alignment::Center);
    if own {
        let icon = iced::widget::svg(crate::icons::icon_undo(look.ctx.theme_id))
            .width(Length::Fixed(TS.label))
            .height(Length::Fixed(TS.label));
        let mut reset = button(icon)
            .padding([SP.xs, SP.s])
            .style(crate::panels::form_look::ghost_button);
        if common.is_editable() {
            reset = reset.on_press(via_msg(ViaMsg::SizeReset {
                target: common.target.clone(),
                field,
            }));
        }
        value_row = value_row.push(with_tip(look, reset.boxed(), ids::PCB_PRIM_RESET_TO_TYPE));
    }
    let tag = (!differs && !common.target.is_placement()).then_some(ids::PCB_PRIM_FROM_TYPE);
    let (label, link) = if field == F::Diameter {
        (ids::PCB_PRIM_DIAMETER, drawing::DIAMETER)
    } else {
        (ids::PCB_PRIM_DRILL, drawing::DRILL)
    };
    linked(
        look,
        common,
        link,
        labelled(look, label, tagged(look, value_row.boxed(), tag)),
    )
}

/// A via's own Diameter or Drill, overriding its type's: set in bold.
fn bold_mm_field<'a>(
    look: Look<'a>,
    f: &'a ViaFacts,
    field: F,
    value: Option<f64>,
) -> Element<'a, PanelMsg> {
    let bold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    super::rows::input(&f.common, field, value.map(format_number), look)
        .style(crate::panels::form_look::text_input_style)
        .font(bold)
        .boxed()
}

/// The layer chips over the illustration: one per size row of a
/// Top-Middle-Bottom or Full Stack via; none for a Simple one.
fn layer_chips<'a>(look: Look<'a>, f: &'a ViaFacts) -> Option<Element<'a, PanelMsg>> {
    let first = f.layer_sizes.first()?;
    let tokens = look.tokens();
    let r = resolve(tokens);
    let style = chips::ChipStyle {
        fill: r.field,
        edge: r.line_soft,
        hover: hover_overlay(tokens),
        ink: ti(tokens.text),
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    };
    let selected = f
        .view_layer
        .filter(|l| f.layer_sizes.iter().any(|row| row.layer == *l))
        .unwrap_or(first.layer);
    let options = f
        .layer_sizes
        .iter()
        .map(|row| (row.layer, Label::from(UserText::new(row.label.clone()))));
    let bar = chips::choice_chips(
        options,
        selected,
        style,
        chips::ChipMetrics::from_text_size(TS.body),
        |layer| via_msg(ViaMsg::ViewLayerPicked(layer)),
    );
    Some(
        iced::widget::container(bar)
            .padding([SP.xs, super::rows::PAD_X])
            .width(Length::Fill)
            .boxed(),
    )
}

/// The protection class choice in the (read-only) Type dropdown.
#[derive(Debug, Clone, PartialEq)]
struct ClassChoice {
    class: Ipc4761,
    label: String,
}

/// The catalog name of an IPC-4761 class, as the Type dropdown and the
/// Protection pill show it.
pub(super) fn class_label(class: Ipc4761) -> MessageId {
    match class {
        Ipc4761::Open => ids::PCB_PRIM_IPC_OPEN,
        Ipc4761::IA => ids::PCB_PRIM_IPC_IA,
        Ipc4761::IB => ids::PCB_PRIM_IPC_IB,
        Ipc4761::II => ids::PCB_PRIM_IPC_II,
        Ipc4761::IIIA => ids::PCB_PRIM_IPC_IIIA,
        Ipc4761::IIIB => ids::PCB_PRIM_IPC_IIIB,
        Ipc4761::IVA => ids::PCB_PRIM_IPC_IVA,
        Ipc4761::IVB => ids::PCB_PRIM_IPC_IVB,
        Ipc4761::V => ids::PCB_PRIM_IPC_V,
        Ipc4761::VIA => ids::PCB_PRIM_IPC_VIA,
        Ipc4761::VIB => ids::PCB_PRIM_IPC_VIB,
        Ipc4761::VII => ids::PCB_PRIM_IPC_VII,
        Ipc4761::Custom => ids::PCB_PRIM_IPC_CUSTOM,
    }
}

fn protection<'a>(look: Look<'a>, f: &'a ViaFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_PROTECTION).collapsible(
        "pcb_prim.via.protection",
        false,
        None,
    );
    let class = f.protection.map(|p| p.class());
    if let Some(class) = class {
        s.push_row(labelled_blank(
            look,
            tagged(
                look,
                row![pill(look, look.t(class_label(class)))].boxed(),
                Some(ids::PCB_PRIM_FROM_TYPE),
            ),
        ));
    }
    let choices: Vec<ClassChoice> = Ipc4761::ALL
        .into_iter()
        .map(|class| ClassChoice {
            class,
            label: look.t(class_label(class)).into_string(),
        })
        .collect();
    let selected = class.and_then(|c| choices.iter().find(|x| x.class == c).cloned());
    let list = pick_list(selected, choices, |c: &ClassChoice| c.label.clone())
        .graphite()
        .padding(SP.s)
        .width(Length::Fill);
    s.push(
        common,
        ids::PCB_PRIM_PROTECTION_TYPE,
        linked(
            look,
            common,
            drawing::PROTECTION,
            labelled(
                look,
                ids::PCB_PRIM_PROTECTION_TYPE,
                with_tip(
                    look,
                    greyed_unless(look, false, list.boxed()),
                    ids::PCB_PRIM_PROTECTION_TYPE_TIP,
                ),
            ),
        ),
    );
    let fill = f.protection.map(|p| p.fill);
    let fixed = |cells: Vec<(MessageId, bool)>| {
        let cells = cells
            .into_iter()
            .map(|(id, is_on)| Segment {
                label: look.t(id),
                is_on,
                msg: None,
            })
            .collect();
        with_tip(look, segments(cells), ids::PCB_PRIM_FILL_CAP_UNSTORED)
    };
    s.push(
        common,
        ids::PCB_PRIM_FILL,
        labelled(
            look,
            ids::PCB_PRIM_FILL,
            fixed(vec![
                (ids::PCB_PRIM_NONE, fill == Some(ViaFill::None)),
                (ids::PCB_PRIM_FILL_PLUG, fill == Some(ViaFill::Plugged)),
                (ids::PCB_PRIM_FILL_RESIN, fill == Some(ViaFill::Resin)),
                (
                    ids::PCB_PRIM_FILL_CONDUCTIVE,
                    fill == Some(ViaFill::Conductive),
                ),
            ]),
        ),
    );
    if let Some(p) = f.protection.filter(|p| p.fill == ViaFill::Plugged) {
        s.push(
            common,
            ids::PCB_PRIM_PLUG_FROM,
            labelled(
                look,
                ids::PCB_PRIM_PLUG_FROM,
                fixed(vec![
                    (ids::PCB_PRIM_TOP, p.plug_from == PlugSide::Top),
                    (ids::PCB_PRIM_BOTTOM, p.plug_from == PlugSide::Bottom),
                    (ids::PCB_PRIM_BOTH, p.plug_from == PlugSide::Both),
                ]),
            ),
        );
    }
    let capped = f.protection.map(|p| p.capped);
    s.push(
        common,
        ids::PCB_PRIM_CAP,
        labelled(
            look,
            ids::PCB_PRIM_CAP,
            fixed(vec![
                (ids::PCB_PRIM_NONE, capped == Some(false)),
                (ids::PCB_PRIM_CAPPED, capped == Some(true)),
            ]),
        ),
    );
    let side = |flag: PrimitiveFlag, label: MessageId, tented: Option<bool>, reaches: bool| {
        greyed_unless(
            look,
            reaches,
            flag_box(look, common, flag, label, tented, ids::PCB_PRIM_TENTED_TIP),
        )
    };
    let (top, bottom) = f.reaches;
    s.push(
        common,
        ids::PCB_PRIM_TENTED,
        linked(
            look,
            common,
            drawing::TENT,
            labelled(
                look,
                ids::PCB_PRIM_TENTED,
                row![
                    side(
                        PrimitiveFlag::TentedTop,
                        ids::PCB_PRIM_TOP,
                        f.top.tented,
                        top
                    ),
                    side(
                        PrimitiveFlag::TentedBottom,
                        ids::PCB_PRIM_BOTTOM,
                        f.bottom.tented,
                        bottom
                    ),
                ]
                .spacing(SP.m)
                .boxed(),
            ),
        ),
    );
    if !bottom {
        s.push_row(caption(look, look.t(ids::PCB_PRIM_NO_MASK_BOTTOM)));
    }
    if !top {
        s.push_row(caption(look, look.t(ids::PCB_PRIM_NO_MASK_TOP)));
    }
    s.push_row(caption(look, look.t(ids::PCB_PRIM_PROTECTION_FROM_TYPE)));
    s.push_row(
        iced::widget::container(later_link(
            look,
            look.t(ids::PCB_PRIM_EDIT_VIA_TYPES),
            ids::PCB_PRIM_LAYER_STACK_LATER,
        ))
        .padding([0.0, super::rows::PAD_X])
        .boxed(),
    );
    s.finish()
}

/// Stacked or Staggered, for a microvia type: greyed, the board does not
/// store it yet.
fn microvia_row<'a>(look: Look<'a>) -> Element<'a, PanelMsg> {
    let cells = [
        ids::PCB_PRIM_MICROVIA_STACKED,
        ids::PCB_PRIM_MICROVIA_STAGGERED,
    ]
    .into_iter()
    .map(|id| Segment {
        label: look.t(id),
        is_on: false,
        msg: None,
    })
    .collect();
    labelled(
        look,
        ids::PCB_PRIM_MICROVIA,
        with_tip(
            look,
            greyed_unless(look, false, segments(cells)),
            ids::PCB_PRIM_MICROVIA_UNSTORED,
        ),
    )
}

/// Microvia (for a microvia type), Thermal Relief, Tolerance, Testpoint and
/// Propagation Delay: the board does not store them per via yet.
fn not_stored<'a>(look: Look<'a>, f: &'a ViaFacts) -> Option<Element<'a, PanelMsg>> {
    let dash = || look.t(ids::PCB_PRIM_NO_VALUE);
    let micro = f
        .via_type
        .as_ref()
        .is_some_and(|t| t.kind == signex_types::pcb::ViaType::Micro);
    let microvia = micro.then(|| microvia_row(look));
    unstored(
        look,
        &f.common,
        microvia
            .into_iter()
            .chain([
                unstored_row(
                    look,
                    ids::PCB_PRIM_THERMAL_RELIEF,
                    look.t(ids::PCB_PRIM_DIRECT),
                    ids::PCB_PRIM_RELIEF_UNSUPPORTED,
                ),
                unstored_row(
                    look,
                    ids::PCB_PRIM_TOLERANCE,
                    dash(),
                    ids::PCB_PRIM_TOLERANCE_UNSUPPORTED,
                ),
                unstored_row(
                    look,
                    ids::PCB_PRIM_TESTPOINT,
                    look.t(ids::PCB_PRIM_TESTPOINT_SUMMARY),
                    ids::PCB_PRIM_TESTPOINT_UNSUPPORTED,
                ),
                unstored_row(
                    look,
                    ids::PCB_PRIM_PROPAGATION_DELAY,
                    dash(),
                    ids::PCB_PRIM_DELAY_UNKNOWN,
                ),
            ])
            .collect(),
    )
}

#[cfg(test)]
#[path = "via_tests.rs"]
mod tests;
