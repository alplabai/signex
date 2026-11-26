//! Properties of the tracks, vias, pads, areas or drawings selected on a
//! board (#2355, #2393, #2424). One panel per kind (`track`, `via`, `pad`,
//! `zone` for fills, regions, pours and keepouts, `graphic` for lines, arcs,
//! circles, rectangles and text frames) built from shared rows (`rows`)
//! and the shared Net Information section (`net_info`). A pure view over
//! facts the app copies from the board (`facts`); every edit is a
//! [`PcbPrimitiveMsg`] the app turns into one board command.
//!
//! The header is this module's own (#2523, mockup rows 95 to 102: header,
//! search, illustration, sections): the kind, a count pill, the lock icon,
//! a dimension's Value card (mockup row 101), then the search, or while
//! placing the paused callout. Each kind's view
//! opens with its illustration (`drawing`, linked to its rows by
//! `linked`). The board panel (#2353) and the component panel (#2354) carry
//! the same shape, and the three meet in `pcb_board_properties::view`,
//! which picks the panel.
//!
//! While a board placement tool is armed the same panel shows the object it
//! places next (#2393): its target has no items, and its edits set the
//! tool's values instead of the board.

mod arc;
mod board_text;
mod dimension;
mod dimension_style;
mod drawing;
mod facts;
mod graphic;
mod linked;
mod mask;
mod net_info;
mod pad;
mod pour_shape;
mod rows;
mod stack_rows;
mod track;
mod unstored;
mod via;
mod zone;

use std::path::PathBuf;

use iced::widget::{Space, column, container, row, svg};
use iced::{Element, Length, Padding, Theme, Widget as _};
use signex_engine::pcb::draw::ArcMode;
use signex_engine::pcb::primitive_edit::DimensionStyleEdit;
use signex_i18n::{Localized, MessageId, ids, t};
use signex_types::pcb::PadShape;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_stack::StackMode;
use signex_types::zone_pour_shape::{PourCorners, PourFillMode};
use signex_widgets::property_illustration::IllustrationMsg;
use signex_widgets::{button, text, text_input, tooltip};

pub(crate) use dimension::{is_dimension_length, typed_length_mm};
pub use facts::PairFacts;
pub use facts::ViaTypeChoice;
pub use facts::{
    ArcFacts, AreaLayerChoice, DimensionFacts, DimensionStyleFacts, Editability, GraphicFacts,
    LayerChoice, LayerSizeRow, MaskSideFacts, NetChoice, NetInfo, PadFacts, PcbPrimitiveFacts,
    PrimitiveCommon, PrimitiveField, PrimitiveFlag, PrimitiveTarget, ShapeChoice, TextFacts,
    TrackFacts, ViaFacts, ZoneFacts, ZoneKind, shared,
};
pub use linked::{EndChoice, end_choices, ends_tip};
pub use via::ViaMsg;
pub(crate) use via::class_short;
pub use zone::PourMsg;
pub(crate) use zone::unfilled_reason;

use super::{PanelContext, PanelMsg, form_look};
use crate::styles::{PillTone, Radii, Spacing, TypeScale, pill_colors, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Widget id of the field Tab focuses while placing (#2393): a via's
/// Diameter, a track's or arc's Width, a pour's Clearance, a pad's X-Size,
/// a string's Text, a dimension's Line Width.
pub static PLACEMENT_INPUT_ID: std::sync::LazyLock<iced::widget::Id> =
    std::sync::LazyLock::new(|| iced::widget::Id::new("signex.pcb_placement_field"));

/// Whether `field` is the one Tab focuses while placing. Each form has one.
pub fn is_placement_focus(field: PrimitiveField) -> bool {
    matches!(
        field,
        PrimitiveField::Diameter
            | PrimitiveField::Width
            | PrimitiveField::Clearance
            | PrimitiveField::SizeX
            | PrimitiveField::Text
            | PrimitiveField::LineWidth
    )
}

/// An event of a primitive panel.
#[derive(Debug, Clone, PartialEq)]
pub enum PcbPrimitiveMsg {
    FieldTyped {
        target: PrimitiveTarget,
        field: PrimitiveField,
        text: String,
    },
    /// Enter in a field: its draft is applied.
    FieldSubmitted {
        target: PrimitiveTarget,
        field: PrimitiveField,
    },
    LayerPicked {
        target: PrimitiveTarget,
        layer: CopperLayer,
    },
    /// An area's Layer: copper, or a keep-out layer for a keepout.
    AreaLayerPicked {
        target: PrimitiveTarget,
        layer: PcbLayer,
    },
    NetPicked {
        target: PrimitiveTarget,
        net: u32,
    },
    /// A pour's Remove Dead Copper box was ticked or cleared (#2216).
    DeadIslandsToggled {
        target: PrimitiveTarget,
        on: bool,
    },
    /// A pour's Corners segment was picked (#2578): one undo step.
    PourCornersPicked {
        target: PrimitiveTarget,
        corners: PourCorners,
    },
    /// A pour's Fill Mode segment was picked (#2579): one undo step.
    PourFillPicked {
        target: PrimitiveTarget,
        fill: PourFillMode,
    },
    ShapePicked {
        target: PrimitiveTarget,
        shape: PadShape,
    },
    /// Rule or Manual picked for the mask expansion `field` (#2378).
    MaskModePicked {
        target: PrimitiveTarget,
        field: PrimitiveField,
        manual: bool,
    },
    /// An on / off setting ticked or cleared (#2378).
    FlagToggled {
        target: PrimitiveTarget,
        flag: PrimitiveFlag,
        on: bool,
    },
    /// The pick button: the next click on the board names the net.
    NetPickStarted {
        target: PrimitiveTarget,
    },
    /// The net name link: select the net's copper.
    NetLinkPressed {
        board: PathBuf,
        net: u32,
    },
    /// The class link: select the copper of every net of the class.
    ClassLinkPressed {
        board: PathBuf,
        class: String,
    },
    ShowMoreToggled,
    SearchTyped(String),
    /// The Lock box of tracks or vias (#2407).
    LockToggled {
        target: PrimitiveTarget,
        locked: bool,
    },
    /// A via's From or To layer was picked (#2407): the span it then has.
    SpanPicked {
        target: PrimitiveTarget,
        top: CopperLayer,
        bottom: CopperLayer,
    },
    /// A Simple, Top-Middle-Bottom or Full Stack tile (#2401).
    StackModePicked {
        target: PrimitiveTarget,
        mode: StackMode,
    },
    /// The pointer entered or left a linked row or illustration callout
    /// (#2523).
    IllustrationHovered(IllustrationMsg),
    /// The "Not stored yet" group was opened or closed.
    UnstoredToggled,
    /// A number picked from a segmented row (a quick rotation, a
    /// dimension's precision): one undo step, as a typed value.
    ValuePicked {
        target: PrimitiveTarget,
        field: PrimitiveField,
        value: f64,
    },
    /// A dimension's arrow style, font, bold, italic, text position, unit,
    /// unit suffix or trailing zeros was picked (#2868): one undo step.
    DimensionStylePicked {
        target: PrimitiveTarget,
        edit: DimensionStyleEdit,
    },
    /// The armed Arc tool's Placement was picked: how the next arc is drawn.
    ArcModePicked(ArcMode),
    /// The Via panel's via type, reset arrows and layer chips (mockup row 96).
    Via(ViaMsg),
    /// A pour's Copper card: Repour, Shelve and the board-wide menu (#2523).
    Pour(PourMsg),
}

/// The panel for `facts`: header, then the kind's sections.
pub fn view<'a>(ctx: &'a PanelContext, facts: &'a PcbPrimitiveFacts) -> Element<'a, PanelMsg> {
    let body = match facts {
        PcbPrimitiveFacts::Track(f) => track::view(ctx, f),
        PcbPrimitiveFacts::Via(f) => via::view(ctx, f),
        PcbPrimitiveFacts::Pad(f) => pad::view(ctx, f),
        PcbPrimitiveFacts::Zone(f) => zone::view(ctx, f),
        PcbPrimitiveFacts::Arc(f) => arc::view(ctx, f),
        PcbPrimitiveFacts::Text(f) => board_text::view(ctx, f),
        PcbPrimitiveFacts::Dimension(f) => dimension::view(ctx, f),
        PcbPrimitiveFacts::Graphic(f) => graphic::view(ctx, f),
    };
    let common = facts.common();
    let mut notes = column![].spacing(SP.xs);
    let faint = resolve(&ctx.tokens).ink4;
    let note = |id| text(t!(ctx.i18n, id)).size(TS.caption).color(faint);
    match common.editability {
        Editability::ReadOnly => notes = notes.push(note(ids::PCB_PRIM_READ_ONLY)),
        Editability::Locked => notes = notes.push(note(ids::PCB_PRIM_PAD_LOCKED)),
        // A locked item says so in the warn callout under the search (a
        // via in its Definition section).
        Editability::ItemLocked | Editability::Editable => {}
    }
    if common.picking_net {
        notes = notes.push(note(ids::PCB_PRIM_PICKING_NET));
    }
    if !common.drafts.is_empty() {
        notes = notes.push(note(ids::PCB_PRIM_ENTER_HINT));
    }
    column![
        header(ctx, facts),
        body,
        container(notes).padding([SP.s, SP.l]).width(Length::Fill),
    ]
    .width(Length::Fill)
    .boxed()
}

/// The header (mockup rows 95-102): the kind, its count pill (a pad's
/// "{component}-{pad}", "Next to place" while placing), a Shelved pill for
/// a shelved pour, the lock icon; under it a dimension's Value card (mockup
/// row 101), then the search, or while placing the
/// placing callout; and for a locked item the warn callout with Unlock.
fn header<'a>(ctx: &'a PanelContext, facts: &'a PcbPrimitiveFacts) -> Element<'a, PanelMsg> {
    let look = rows::Look::of(ctx);
    let common = facts.common();
    let placing = common.target.is_placement();
    let semibold = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let mut title_row = row![
        text(look.t(title(facts)))
            .size(TS.label)
            .font(semibold)
            .color(look.ink),
        pill(look, count_pill(look, facts), PillTone::Accent),
    ]
    .spacing(SP.s + SP.xs)
    .align_y(iced::Alignment::Center);
    if let PcbPrimitiveFacts::Zone(f) = facts
        && f.shelved == Some(true)
    {
        title_row = title_row.push(pill(
            look,
            look.t(ids::PCB_PRIM_SHELVED_PILL),
            PillTone::Warn,
        ));
    }
    title_row = title_row.push(Space::new().width(Length::Fill).boxed());
    if !placing {
        title_row = title_row.push(lock_icon(look, facts));
    }
    let mut col = column![
        container(title_row)
            .padding(Padding {
                top: SP.m,
                right: SP.l,
                bottom: 0.0,
                left: SP.l,
            })
            .width(Length::Fill)
    ];
    // Mockup row 101: a dimension's measured Value is the headline card,
    // above the search.
    if let PcbPrimitiveFacts::Dimension(f) = facts
        && let Some(card) = dimension::value_card(look, f)
    {
        col = col.push(card);
    }
    if placing {
        let line = if common.placement_paused {
            ids::PCB_PRIM_PLACING_PAUSED
        } else {
            ids::PCB_PRIM_PLACING_HINT
        };
        col = col.push(callout(look, PillTone::Accent, false, look.t(line), None));
    } else {
        col = col.push(search(look, common));
    }
    let via = matches!(facts, PcbPrimitiveFacts::Via(_));
    if !placing && !via && common.editability == Editability::ItemLocked {
        col = col.push(locked_callout(look, common));
    }
    col.width(Length::Fill).boxed()
}

/// The warn callout of a locked item, with Unlock.
fn locked_callout<'a>(look: rows::Look<'a>, common: &PrimitiveCommon) -> Element<'a, PanelMsg> {
    let unlock = button(text(look.t(ids::PCB_PRIM_UNLOCK)).size(TS.caption))
        .padding([SP.xs, SP.s])
        .style(form_look::ghost_button)
        .on_press(PanelMsg::PcbPrimitive(PcbPrimitiveMsg::LockToggled {
            target: common.target.clone(),
            locked: false,
        }));
    callout(
        look,
        PillTone::Warn,
        true,
        look.t(ids::PCB_PRIM_ITEM_LOCKED),
        Some(unlock.boxed()),
    )
}

/// The header title: the kind, a zone its real kind, a drawing its own.
fn title(facts: &PcbPrimitiveFacts) -> MessageId {
    match facts {
        PcbPrimitiveFacts::Track(_) => ids::PCB_PRIM_TRACK_TITLE,
        PcbPrimitiveFacts::Via(_) => ids::PCB_PRIM_VIA_TITLE,
        PcbPrimitiveFacts::Pad(_) => ids::PCB_PRIM_PAD_TITLE,
        PcbPrimitiveFacts::Zone(f) => match f.kind {
            Some(ZoneKind::Fill) => ids::PCB_PRIM_FILL_TITLE,
            Some(ZoneKind::Region) => ids::PCB_PRIM_REGION_TITLE,
            Some(ZoneKind::Pour) => ids::PCB_PRIM_POUR_TITLE,
            Some(ZoneKind::Keepout) => ids::PCB_PRIM_KEEPOUT_TITLE,
            Some(ZoneKind::Cutout) => ids::PCB_PRIM_CUTOUT_TITLE,
            None => ids::PCB_PRIM_AREA_TITLE,
        },
        PcbPrimitiveFacts::Arc(_) => ids::PCB_PRIM_ARC_TITLE,
        PcbPrimitiveFacts::Text(_) => ids::PCB_PRIM_TEXT_TITLE,
        PcbPrimitiveFacts::Dimension(_) => ids::PCB_PRIM_DIMENSION_TITLE,
        PcbPrimitiveFacts::Graphic(f) => graphic::title(f.kind),
    }
}

/// The count pill: how many are selected, one pad as "{component}-{pad}",
/// "Next to place" while placing.
fn count_pill(look: rows::Look<'_>, facts: &PcbPrimitiveFacts) -> Localized {
    let i18n = &*look.ctx.i18n;
    let common = facts.common();
    if common.target.is_placement() {
        return look.t(ids::PCB_PRIM_PLACING);
    }
    let count = common.target.items.len();
    let id = match facts {
        PcbPrimitiveFacts::Track(_) => ids::PCB_PRIM_TRACK_COUNT,
        PcbPrimitiveFacts::Via(_) => ids::PCB_PRIM_VIA_COUNT,
        PcbPrimitiveFacts::Pad(f) => {
            if let (1, Some(component), Some(pad)) = (
                count,
                f.component_designator.as_ref(),
                f.designator.as_ref(),
            ) {
                return t!(
                    i18n,
                    ids::PCB_PRIM_PAD_PILL,
                    "component" => component.clone(),
                    "pad" => pad.clone()
                );
            }
            ids::PCB_PRIM_PAD_COUNT
        }
        PcbPrimitiveFacts::Zone(_) => ids::PCB_PRIM_AREA_COUNT,
        PcbPrimitiveFacts::Arc(_) => ids::PCB_PRIM_ARC_COUNT,
        PcbPrimitiveFacts::Text(_) => ids::PCB_PRIM_TEXT_COUNT,
        PcbPrimitiveFacts::Dimension(_) => ids::PCB_PRIM_DIMENSION_COUNT,
        PcbPrimitiveFacts::Graphic(_) => ids::PCB_PRIM_GRAPHIC_COUNT,
    };
    t!(i18n, id, "count" => count)
}

/// A rounded pill in the caption size, toned `tone`.
fn pill<'a>(look: rows::Look<'a>, label: Localized, tone: PillTone) -> Element<'a, PanelMsg> {
    container(
        text(label)
            .size(TS.caption)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([0.0, SP.s + SP.xs])
    .style(crate::styles::pill(look.tokens(), tone))
    .boxed()
}

/// The lock icon at the header's right (mockup rows 95, 96): live for tracks and
/// vias (filled while locked, a dash for a selection partly locked);
/// a pad's lock is its footprint's; the other kinds store no lock and grey
/// it with the reason.
fn lock_icon<'a>(look: rows::Look<'a>, facts: &'a PcbPrimitiveFacts) -> Element<'a, PanelMsg> {
    let common = facts.common();
    let r = resolve(look.tokens());
    let (locked, live) = match facts {
        PcbPrimitiveFacts::Track(f) => (f.locked, true),
        PcbPrimitiveFacts::Via(f) => (f.locked, true),
        _ => (Some(false), false),
    };
    let tip: Localized = match facts {
        PcbPrimitiveFacts::Track(_) | PcbPrimitiveFacts::Via(_) => match common.locked_count {
            Some((locked, total)) if locked > 0 => t!(
                look.ctx.i18n,
                ids::PCB_PRIM_LOCKED_COUNT,
                "locked" => locked,
                "total" => total
            ),
            _ => look.t(ids::PCB_PRIM_LOCK_TIP),
        },
        PcbPrimitiveFacts::Pad(_) => look.t(ids::PCB_PRIM_PAD_LOCK_TIP),
        PcbPrimitiveFacts::Zone(_) => look.t(ids::PCB_PRIM_AREA_LOCK_UNSUPPORTED),
        PcbPrimitiveFacts::Arc(_)
        | PcbPrimitiveFacts::Text(_)
        | PcbPrimitiveFacts::Dimension(_)
        | PcbPrimitiveFacts::Graphic(_) => look.t(ids::PCB_PRIM_LOCK_UNSUPPORTED),
    };
    let is_locked = locked == Some(true);
    let ink = match facts {
        _ if is_locked => r.accent_soft_ink,
        PcbPrimitiveFacts::Track(_) | PcbPrimitiveFacts::Via(_) | PcbPrimitiveFacts::Pad(_) => {
            r.ink3
        }
        _ => r.ink4,
    };
    let theme = look.ctx.theme_id;
    let handle = if locked.is_none() {
        crate::icons::icon_chrome_lock_mixed(theme)
    } else {
        crate::icons::icon_chrome_lock(theme)
    };
    let icon = svg(handle)
        .width(Length::Fixed(TS.label))
        .height(Length::Fixed(TS.label))
        .style(move |_: &Theme, _| iced::widget::svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    let fill = r.accent_soft;
    let press = (live && common.can_toggle_lock()).then(|| {
        PanelMsg::PcbPrimitive(PcbPrimitiveMsg::LockToggled {
            target: common.target.clone(),
            locked: !is_locked,
        })
    });
    let icon_button =
        button(icon)
            .padding(SP.xs)
            .on_press_maybe(press)
            .style(move |_: &Theme, status| iced::widget::button::Style {
                background: (is_locked || matches!(status, iced::widget::button::Status::Hovered))
                    .then_some(iced::Background::Color(fill)),
                border: iced::Border {
                    radius: Radii::GRAPHITE.control.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::button::Style::default()
            });
    tip_text(look, icon_button.boxed(), tip)
}

/// `content` with `tip` under it on hover.
fn tip_text<'a>(
    look: rows::Look<'a>,
    content: Element<'a, PanelMsg>,
    tip: Localized,
) -> Element<'a, PanelMsg> {
    tooltip(
        content,
        container(text(tip).size(TS.body))
            .padding([SP.xs, SP.s])
            .style(crate::styles::context_menu(look.tokens())),
        iced::widget::tooltip::Position::Bottom,
    )
    .boxed()
}

/// The search field: a magnifier and the text, in one field frame.
fn search<'a>(look: rows::Look<'a>, common: &'a PrimitiveCommon) -> Element<'a, PanelMsg> {
    let faint = resolve(look.tokens()).ink4;
    let icon = svg(crate::icons::icon_chrome_search(look.ctx.theme_id))
        .width(Length::Fixed(TS.body))
        .height(Length::Fixed(TS.body))
        .style(move |_: &Theme, _| iced::widget::svg::Style {
            color: Some(faint),
            ..Default::default()
        });
    let input = text_input(look.t(ids::PCB_PRIM_SEARCH), &common.search)
        .on_input(|text| PanelMsg::PcbPrimitive(PcbPrimitiveMsg::SearchTyped(text)))
        .size(TS.body)
        .padding([SP.xs, SP.xs])
        .style(|theme: &Theme, status| {
            let mut style = form_look::text_input_style(theme, status);
            style.background = iced::Background::Color(iced::Color::TRANSPARENT);
            style.border = iced::Border::default();
            style
        })
        .width(Length::Fill);
    let field = container(
        row![icon, input]
            .spacing(SP.xs)
            .align_y(iced::Alignment::Center),
    )
    .padding([0.0, SP.s])
    .width(Length::Fill)
    .style(crate::styles::field(look.tokens(), false));
    container(field)
        .padding(Padding {
            top: SP.s + SP.xs,
            right: SP.l,
            bottom: SP.xs,
            left: SP.l,
        })
        .width(Length::Fill)
        .boxed()
}

/// A callout band under the header: the accent one says
/// placing is paused, the warn one that the item is locked.
fn callout<'a>(
    look: rows::Look<'a>,
    tone: PillTone,
    lock: bool,
    message: Localized,
    action: Option<Element<'a, PanelMsg>>,
) -> Element<'a, PanelMsg> {
    let (fill, ink) = pill_colors(look.tokens(), tone);
    let theme = look.ctx.theme_id;
    let handle = if lock {
        crate::icons::icon_chrome_lock(theme)
    } else {
        crate::icons::icon_chrome_info(theme)
    };
    let icon = svg(handle)
        .width(Length::Fixed(TS.body))
        .height(Length::Fixed(TS.body))
        .style(move |_: &Theme, _| iced::widget::svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    let mut line = row![
        icon,
        text(message)
            .size(TS.caption)
            .color(ink)
            .width(Length::Fill),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    if let Some(action) = action {
        line = line.push(action);
    }
    let band = container(line)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(iced::Background::Color(fill)),
            border: iced::Border {
                radius: Radii::GRAPHITE.button.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });
    container(band)
        .padding([SP.xs, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// A length or angle as a field shows it: up to four decimals, no trailing
/// zeros.
pub fn format_number(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_string()
    } else {
        text.to_string()
    }
}

/// A typed length in mm or angle in degrees: a number, optionally followed
/// by its unit.
pub fn parse_number(text: &str) -> Option<f64> {
    let trimmed = text.trim();
    let bare = trimmed
        .strip_suffix("mm")
        .or_else(|| trimmed.strip_suffix('°'))
        .unwrap_or(trimmed)
        .trim();
    bare.parse::<f64>().ok().filter(|v| v.is_finite())
}

#[cfg(test)]
mod two_field_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_format_short_and_parse_with_or_without_a_unit() {
        assert_eq!(format_number(0.25), "0.25");
        assert_eq!(format_number(1.0), "1");
        assert_eq!(format_number(-0.00001), "0");
        assert_eq!(parse_number(" 0.3mm "), Some(0.3));
        assert_eq!(parse_number("45°"), Some(45.0));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number("inf"), None);
    }
}
