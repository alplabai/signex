//! The area panel (#2393, #2523 m99): fills, solid regions, polygon pours
//! and keepouts. Under the illustration: Net Information and the net of a
//! copper area, the layer, a keepout's kinds; a pour's Pour section with its
//! fill rules, the thermal relief and corners it cannot use yet (greyed:
//! the fill draws no spokes and stores no corners), and the Copper card with
//! what the fill holds, Repour, Shelve or Unshelve and the board-wide menu.

use std::path::PathBuf;

use iced::widget::{Space, column, container, row};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::enable_section::{EnableSection, dependents_veil};
use signex_widgets::inert::Inert;
use signex_widgets::{Label, button, text};

use super::drawing::zone as dz;
use super::linked::{
    Segment, illustration, linked, linked_row, prefixed, read_only, segments, two_fields, unit,
    with_unit,
};
use super::rows::{
    Look, PAD_X, Section, chip, dropdown, field, fixed_value, labelled, live_checkbox, mm_field,
    net_row, stack, with_tip,
};
use super::stack_rows::caption;
use super::{
    PcbPrimitiveMsg, PrimitiveCommon, PrimitiveField as F, PrimitiveTarget, SP, TS, ZoneFacts,
    ZoneKind, format_number, net_info,
};
use crate::panels::enable_section::enable_section_style;
use crate::panels::{PanelContext, PanelMsg};
use crate::pcb_canvas::layer_style;
use crate::styles::tokens::{Overlay, overlay, resolve};
use crate::styles::{Radii, ti};

/// An event of a pour's Copper card (#2523).
#[derive(Debug, Clone, PartialEq)]
pub enum PourMsg {
    /// Repour: the board of `target`, which need not be the tab in view, is
    /// filled again and the target's pours reported. Nothing in the
    /// document changes.
    RepourPressed {
        target: PrimitiveTarget,
    },
    /// Shelve or Unshelve: one board undo step.
    ShelveToggled {
        target: PrimitiveTarget,
        shelved: bool,
    },
    /// The ⋯ button opened or closed the board-wide menu.
    MenuToggled,
    /// The menu's Repour all, Shelve all and Unshelve all: on `board`, the
    /// board the panel shows, which need not be the tab in view.
    RepourAllPressed {
        board: PathBuf,
    },
    ShelveAllPressed {
        board: PathBuf,
    },
    UnshelveAllPressed {
        board: PathBuf,
    },
}

fn pour_msg(msg: PourMsg) -> PanelMsg {
    PanelMsg::PcbPrimitive(PcbPrimitiveMsg::Pour(msg))
}

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a ZoneFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    let common = &f.common;
    let drawing = illustration(look, common, dz::illustration(f, &ctx.i18n));
    let net_section = if f.is_copper {
        net_info::section(look, common)
    } else {
        None
    };
    stack(vec![
        Some(drawing),
        net_section,
        properties(look, f),
        pour(look, f),
    ])
}

fn properties<'a>(look: Look<'a>, f: &'a ZoneFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_PROPERTIES);
    if f.is_copper {
        s.push(
            common,
            ids::PCB_PRIM_NET,
            linked(look, common, dz::NET, net_row(look, common)),
        );
    }
    let target = common.target.clone();
    let layers = dropdown(common, &f.layers, f.layer.clone(), move |choice| {
        PcbPrimitiveMsg::AreaLayerPicked {
            target: target.clone(),
            layer: choice.layer,
        }
    });
    let swatch = f.layer.as_ref().map_or_else(
        || {
            container(Space::new())
                .width(Length::Fixed(TS.body))
                .boxed()
        },
        |l| chip(layer_style::layer_colour(l.layer, look.tokens())),
    );
    s.push(
        common,
        ids::PCB_PRIM_LAYER,
        linked_row(
            look,
            common,
            dz::LAYER,
            ids::PCB_PRIM_LAYER,
            row![swatch, container(layers).width(Length::Fill)]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center)
                .boxed(),
        ),
    );
    if f.kind == Some(ZoneKind::Keepout) {
        let all_copper = read_only(look, look.t(ids::PCB_PRIM_KEEPOUT_ALL_COPPER));
        s.push(
            common,
            ids::PCB_PRIM_KEEPOUT_KINDS,
            linked_row(
                look,
                common,
                dz::KEEPS_OUT,
                ids::PCB_PRIM_KEEPOUT_KINDS,
                with_tip(look, all_copper, ids::PCB_PRIM_KEEPOUT_KINDS_UNSUPPORTED),
            ),
        );
    }
    s.finish()
}

/// A pour's fill rules and its Copper card; nothing for other areas.
fn pour<'a>(look: Look<'a>, f: &'a ZoneFacts) -> Option<Element<'a, PanelMsg>> {
    if !f.is_pour {
        return None;
    }
    let common = &f.common;
    let mm = || unit(look, ids::PCB_PRIM_UNIT_MM);
    let mut s = Section::new(look, ids::PCB_PRIM_POUR_SETTINGS);
    s.push(
        common,
        ids::PCB_PRIM_POUR_FILL_MODE,
        linked_row(
            look,
            common,
            dz::FILL_MODE,
            ids::PCB_PRIM_POUR_FILL_MODE,
            fill_mode(look),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_CLEARANCE,
        linked_row(
            look,
            common,
            dz::CLEARANCE,
            ids::PCB_PRIM_CLEARANCE,
            with_unit(mm_field(common, F::Clearance, f.clearance, look), mm()),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_MIN_THICKNESS,
        labelled(
            look,
            ids::PCB_PRIM_MIN_THICKNESS,
            with_unit(
                mm_field(common, F::MinThickness, f.min_thickness, look),
                mm(),
            ),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_POUR_THERMAL,
        linked(look, common, dz::THERMAL, thermal(look, f)),
    );
    for (label, row) in corners(look, common) {
        s.push(common, label, row);
    }
    let priority = f.priority.map(|p| p.to_string());
    s.push(
        common,
        ids::PCB_PRIM_PRIORITY,
        labelled(
            look,
            ids::PCB_PRIM_PRIORITY,
            field(common, F::Priority, priority, None, look),
        ),
    );
    let islands = prefixed(
        look,
        common,
        F::MinIslandArea,
        f.min_island_area,
        ids::PCB_PRIM_POUR_BELOW,
    );
    s.push(
        common,
        ids::PCB_PRIM_POUR_REMOVE_ISLANDS,
        labelled(
            look,
            ids::PCB_PRIM_POUR_REMOVE_ISLANDS,
            with_unit(
                with_tip(look, islands, ids::PCB_PRIM_MIN_ISLAND_AREA),
                unit(look, ids::PCB_PRIM_UNIT_MM2),
            ),
        ),
    );
    let target = common.target.clone();
    s.push(
        common,
        ids::PCB_PRIM_REMOVE_DEAD_ISLANDS,
        container(live_checkbox(
            look,
            common,
            ids::PCB_PRIM_REMOVE_DEAD_ISLANDS,
            f.remove_dead_islands,
            move |on| PcbPrimitiveMsg::DeadIslandsToggled {
                target: target.clone(),
                on,
            },
        ))
        .padding([SP.xs, PAD_X])
        .width(Length::Fill)
        .boxed(),
    );
    // What a pour about to be placed holds is not known yet.
    if !common.target.is_placement() {
        s.push(
            common,
            ids::PCB_PRIM_POUR_COPPER,
            linked(look, common, dz::SHELVE, copper_card(look, f)),
        );
        s.push(
            common,
            ids::PCB_PRIM_POUR_COPPER,
            caption(look, look.t(ids::PCB_PRIM_POUR_SHELVE_NOTE)),
        );
    }
    s.finish()
}

/// Fill Mode: Solid, the only fill there is; Hatched greyed. No None
/// (mockup row 99): the outline-only state is Shelve.
fn fill_mode<'a>(look: Look<'a>) -> Element<'a, PanelMsg> {
    let modes = segments(vec![
        Segment {
            label: look.t(ids::PCB_PRIM_POUR_FILL_SOLID),
            is_on: true,
            msg: None,
        },
        Segment {
            label: look.t(ids::PCB_PRIM_POUR_FILL_HATCHED),
            is_on: false,
            msg: None,
        },
    ]);
    with_tip(look, modes, ids::PCB_PRIM_POUR_HATCHED_TIP)
}

/// The Thermal Relief subsection: the stored switch and Spoke / Gap, read
/// only and greyed, since the fill joins pads solidly.
fn thermal<'a>(look: Look<'a>, f: &'a ZoneFacts) -> Element<'a, PanelMsg> {
    let (on, spoke, gap) = match f.thermal {
        Some((on, spoke, gap)) => (on, Some(spoke), Some(gap)),
        None => (false, None, None),
    };
    let value = |v: Option<f64>| {
        read_only(
            look,
            UserText::new(v.map(format_number).unwrap_or_default()),
        )
    };
    let spoke_gap = labelled(
        greyed(look),
        ids::PCB_PRIM_POUR_SPOKE_GAP,
        two_fields(value(spoke), value(gap), unit(look, ids::PCB_PRIM_UNIT_MM)),
    );
    // The section veils its fields while off; while on they are veiled here.
    let spoke_gap = if on {
        veiled(look.tokens(), spoke_gap)
    } else {
        spoke_gap
    };
    let spoke_gap = with_tip(look, spoke_gap, ids::PCB_PRIM_POUR_THERMAL_TIP);
    EnableSection::new(look.t(ids::PCB_PRIM_POUR_THERMAL), on)
        .tip(look.t(ids::PCB_PRIM_POUR_THERMAL_TIP))
        .view(enable_section_style(look.tokens()), spoke_gap)
}

/// Corners and Corner Size: Sharp, the only corner the fill makes; the
/// others and the size are greyed until the board stores them.
fn corners<'a>(
    look: Look<'a>,
    common: &'a PrimitiveCommon,
) -> [(MessageId, Element<'a, PanelMsg>); 2] {
    let shapes = segments(
        [
            (ids::PCB_PRIM_POUR_CORNER_SHARP, true),
            (ids::PCB_PRIM_POUR_CORNER_ROUNDED, false),
            (ids::PCB_PRIM_POUR_CORNER_CHAMFER, false),
        ]
        .into_iter()
        .map(|(id, is_on)| Segment {
            label: look.t(id),
            is_on,
            msg: None,
        })
        .collect(),
    );
    let shapes = linked_row(
        look,
        common,
        dz::CORNERS,
        ids::PCB_PRIM_POUR_CORNERS,
        with_tip(look, shapes, ids::PCB_PRIM_POUR_CORNERS_TIP),
    );
    let size = labelled(
        greyed(look),
        ids::PCB_PRIM_POUR_CORNER_SIZE,
        with_unit(
            fixed_value(
                look,
                UserText::new(String::new()),
                ids::PCB_PRIM_POUR_CORNERS_TIP,
            ),
            unit(look, ids::PCB_PRIM_UNIT_MM),
        ),
    );
    [
        (ids::PCB_PRIM_POUR_CORNERS, shapes),
        (ids::PCB_PRIM_POUR_CORNER_SIZE, size),
    ]
}

/// `look` with the label ink of a greyed row.
fn greyed(look: Look<'_>) -> Look<'_> {
    Look {
        muted: resolve(look.tokens()).ink4,
        ..look
    }
}

/// `content` greyed under the panel veil and deaf to input.
fn veiled<'a>(tokens: &ThemeTokens, content: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
    match dependents_veil(ti(tokens.panel_bg), false) {
        Some(veil) => Inert::new(content, veil).boxed(),
        None => content,
    }
}

/// The Copper card: what the fill holds (or that the pour is shelved, or
/// why it has no fill), Repour, Shelve or Unshelve, and the ⋯ menu of
/// board-wide commands.
fn copper_card<'a>(look: Look<'a>, f: &'a ZoneFacts) -> Element<'a, PanelMsg> {
    let r = resolve(look.tokens());
    let shelved = f.shelved == Some(true);
    let value: Option<Label<'a>> = if shelved {
        Some(look.t(ids::PCB_PRIM_POUR_SHELVED_VALUE).into())
    } else if let Some(why) = f.unfilled {
        let why = t!(look.ctx.i18n, unfilled_reason(why)).into_string();
        Some(t!(look.ctx.i18n, ids::PCB_PRIM_POUR_UNFILLED, "why" => why).into())
    } else {
        f.copper.map(|(area, pieces)| {
            t!(
                look.ctx.i18n,
                ids::PCB_PRIM_POUR_COPPER_VALUE,
                "area" => format_number(area),
                "pieces" => pieces
            )
            .into()
        })
    };
    let mono = iced::Font {
        weight: iced::font::Weight::Medium,
        ..crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
    };
    let mut headline = row![
        text(look.t(ids::PCB_PRIM_POUR_COPPER))
            .size(TS.caption)
            .color(r.ink3)
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    if let Some(value) = value {
        headline = headline.push(
            text(value)
                .size(TS.body)
                .font(mono)
                .color(if shelved { r.ink4 } else { look.ink })
                .wrapping(iced::widget::text::Wrapping::None)
                .width(Length::Fill)
                .boxed(),
        );
    }
    let headline = with_tip(look, headline.boxed(), ids::PCB_PRIM_POUR_COPPER_TIP);
    let mut card = column![headline, card_actions(look, f)].spacing(SP.s);
    if f.menu_open {
        card = card.push(pour_menu(look, &f.common));
    }
    container(
        container(card)
            .padding([SP.s, SP.s])
            .width(Length::Fill)
            .style(card_style(look.tokens())),
    )
    .padding([SP.xs, PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// Repour (greyed while shelved), Shelve or Unshelve, and the ⋯ button.
fn card_actions<'a>(look: Look<'a>, f: &'a ZoneFacts) -> Element<'a, PanelMsg> {
    let shelved = f.shelved == Some(true);
    let repour = small_button(look, look.t(ids::PCB_PRIM_POUR_REPOUR), repour_message(f));
    let repour = if shelved {
        with_tip(look, repour, ids::PCB_PRIM_POUR_REPOUR_SHELVED_TIP)
    } else {
        repour
    };
    let shelve = if shelved {
        small_button(look, look.t(ids::PCB_PRIM_POUR_UNSHELVE), shelve_message(f))
    } else {
        with_tip(
            look,
            small_button(look, look.t(ids::PCB_PRIM_POUR_SHELVE), shelve_message(f)),
            ids::PCB_PRIM_POUR_SHELVE_TIP,
        )
    };
    let menu_button = button(text(look.t(ids::PCB_PRIM_POUR_MENU)).size(TS.body))
        .padding([0.0, SP.s])
        .on_press(pour_msg(PourMsg::MenuToggled))
        .style(menu_button_style(look.tokens(), f.menu_open));
    row![
        repour,
        shelve,
        Space::new().width(Length::Fill),
        with_tip(look, menu_button.boxed(), ids::PCB_PRIM_POUR_MENU_TIP),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// What Repour sends: nothing while the pour is shelved, so the button is
/// greyed until it is unshelved.
fn repour_message(f: &ZoneFacts) -> Option<PourMsg> {
    (f.shelved != Some(true)).then(|| PourMsg::RepourPressed {
        target: f.common.target.clone(),
    })
}

/// What Shelve (or, for a shelved pour, Unshelve) sends; nothing on a board
/// that cannot be edited.
fn shelve_message(f: &ZoneFacts) -> Option<PourMsg> {
    let common = &f.common;
    common.is_editable().then(|| PourMsg::ShelveToggled {
        target: common.target.clone(),
        shelved: f.shelved != Some(true),
    })
}

/// The ⋯ menu: Repour all, Shelve all, Unshelve all, on the board the panel
/// shows.
fn pour_menu<'a>(look: Look<'a>, common: &PrimitiveCommon) -> Element<'a, PanelMsg> {
    let editable = common.is_editable();
    let board = || common.target.board.clone();
    let item = |id: MessageId, msg: Option<PourMsg>| {
        button(
            text(look.t(id))
                .size(TS.body)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .padding([SP.xs, SP.s])
        .width(Length::Fill)
        .on_press_maybe(msg.map(pour_msg))
        .style(menu_item_style(look.tokens()))
        .boxed()
    };
    let items = column![
        item(
            ids::PCB_PRIM_POUR_REPOUR_ALL,
            Some(PourMsg::RepourAllPressed { board: board() })
        ),
        item(
            ids::PCB_PRIM_POUR_SHELVE_ALL,
            editable.then(|| PourMsg::ShelveAllPressed { board: board() })
        ),
        item(
            ids::PCB_PRIM_POUR_UNSHELVE_ALL,
            editable.then(|| PourMsg::UnshelveAllPressed { board: board() })
        ),
    ]
    .spacing(SP.xs);
    container(items)
        .padding(SP.xs)
        .width(Length::Fill)
        .style(crate::styles::context_menu(look.tokens()))
        .boxed()
}

/// A small card button (`.btn.sm`): caption text on the field fill with a
/// hairline; greyed and deaf without a message.
fn small_button<'a>(
    look: Look<'a>,
    label: impl Into<Label<'a>>,
    msg: Option<PourMsg>,
) -> Element<'a, PanelMsg> {
    let tokens = look.tokens();
    let r = resolve(tokens);
    let (fill, line, ink, ink4) = (r.field, ti(tokens.border), ti(tokens.text), r.ink4);
    let hover = overlay(tokens, Overlay::Hover);
    button(
        text(label)
            .size(TS.caption)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([SP.xs, SP.m])
    .on_press_maybe(msg.map(pour_msg))
    .style(move |_: &Theme, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(Background::Color(if hovered { hover } else { fill })),
            text_color: if status == button::Status::Disabled {
                ink4
            } else {
                ink
            },
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    })
    .boxed()
}

/// The ⋯ icon button (`.ib`): no frame at rest, the hover fill, and the
/// accent-soft fill while its menu is open.
fn menu_button_style(
    tokens: &ThemeTokens,
    is_open: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let r = resolve(tokens);
    let (soft, soft_ink, ink3) = (r.accent_soft, r.accent_soft_ink, r.ink3);
    let hover = overlay(tokens, Overlay::Hover);
    move |_: &Theme, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        let background = if is_open {
            Some(soft)
        } else if hovered {
            Some(hover)
        } else {
            None
        };
        button::Style {
            background: background.map(Background::Color),
            text_color: if is_open { soft_ink } else { ink3 },
            border: Border {
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    }
}

/// A menu row (`.pv-mi`): ink text, the hover fill; greyed when disabled.
fn menu_item_style(
    tokens: &ThemeTokens,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    let (ink, ink4) = (ti(tokens.text), resolve(tokens).ink4);
    let hover = overlay(tokens, Overlay::Hover);
    move |_: &Theme, status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(hover)),
        text_color: if status == button::Status::Disabled {
            ink4
        } else {
            ink
        },
        border: Border {
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// The card frame (`.gcard`): the panel fill, a hairline, card corners.
pub(super) fn card_style(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let (bg, line) = (ti(tokens.panel_bg), ti(tokens.border));
    move |_: &Theme| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Why a pour has no fill, as Properties and Messages say it.
pub(crate) fn unfilled_reason(why: pcb_geom::Unfilled) -> MessageId {
    match why {
        pcb_geom::Unfilled::NoCopperLayer => ids::PCB_POUR_UNFILLED_NO_LAYER,
        pcb_geom::Unfilled::Outline => ids::PCB_POUR_UNFILLED_OUTLINE,
        pcb_geom::Unfilled::Clearance => ids::PCB_POUR_UNFILLED_CLEARANCE,
    }
}

#[cfg(test)]
mod tests {
    use pcb_geom::{ItemRef, PickKind};
    use uuid::Uuid;

    use super::*;
    use crate::panels::pcb_primitive_properties::Editability;
    use crate::panels::pcb_primitive_properties::drawing::fixture;

    fn pour(shelved: Option<bool>) -> ZoneFacts {
        ZoneFacts {
            common: fixture::common(vec![ItemRef::Zone {
                zone: Uuid::from_u128(1),
                kind: PickKind::Polygons,
            }]),
            kind: Some(ZoneKind::Pour),
            is_copper: true,
            is_pour: true,
            layer: None,
            layers: Vec::new(),
            clearance: Some(0.0),
            min_thickness: Some(0.0),
            priority: Some(0),
            min_island_area: Some(0.0),
            remove_dead_islands: Some(false),
            copper: None,
            shelved,
            thermal: Some((false, 0.0, 0.0)),
            menu_open: false,
            unfilled: None,
        }
    }

    #[test]
    fn repour_is_greyed_while_the_pour_is_shelved() {
        let open = pour(Some(false));
        assert_eq!(
            repour_message(&open),
            Some(PourMsg::RepourPressed {
                target: open.common.target.clone()
            })
        );
        assert_eq!(repour_message(&pour(Some(true))), None);
    }

    #[test]
    fn shelve_toggles_the_pour_and_is_greyed_on_a_read_only_board() {
        let open = pour(Some(false));
        let Some(PourMsg::ShelveToggled { shelved, target }) = shelve_message(&open) else {
            panic!("Shelve sends a toggle");
        };
        assert!(shelved);
        assert_eq!(target, open.common.target);
        let Some(PourMsg::ShelveToggled { shelved, .. }) = shelve_message(&pour(Some(true))) else {
            panic!("Unshelve sends a toggle");
        };
        assert!(!shelved);
        let mut read_only = pour(Some(false));
        read_only.common.editability = Editability::ReadOnly;
        assert_eq!(shelve_message(&read_only), None);
    }
}
