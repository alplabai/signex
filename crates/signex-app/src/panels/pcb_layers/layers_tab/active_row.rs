//! The Active | Set row (mockup row 103): one field-look card with the active layer,
//! picked from the board's layers as a tab under the canvas picks it, and
//! the layer set the view shows. The Set dropdown is the one place a set is
//! picked; the more menu saves and deletes sets.

use iced::widget::pick_list::{Status as PickStatus, Style as PickStyle};
use iced::widget::tooltip::Position;
use iced::widget::{Space, container, pick_list, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme, Widget};
use signex_i18n::{Arg, I18n, ids, t};
use signex_renderer::pcb::{BuiltinLayerSet, LayerSetId};
use signex_types::pcb_layer::PcbLayer;
use signex_widgets::PickListPlaceholder as _;
use signex_widgets::dropdown_style::{DropdownLook, GraphiteDropdown as _};
use signex_widgets::text;

use super::super::metrics::{ACTIVE_CARD, DIVIDER, ROW_GAP};
use super::super::rows::{caps, dot_face, medium};
use super::super::{LayersPanelMsg, PcbLayersPanel, SP, TS};
use crate::app::PcbViewMsg;
use crate::panels::graphite::hint_tooltip;
use crate::panels::{PanelContext, PanelMsg};
use crate::pcb_canvas::layer_style;
use crate::styles::tokens::resolve;
use crate::styles::{field, ti};

/// A layer set entry of the Set dropdown.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::panels::pcb_layers) enum SetChoice {
    Builtin(BuiltinLayerSet),
    Saved(String),
}

/// The sets to pick from: the built-in ones, then the saved ones.
pub(in crate::panels::pcb_layers) fn set_choices(panel: &PcbLayersPanel) -> Vec<SetChoice> {
    BuiltinLayerSet::ALL
        .into_iter()
        .map(SetChoice::Builtin)
        .chain(panel.saved_sets.iter().cloned().map(SetChoice::Saved))
        .collect()
}

/// A set's row in the Set dropdown.
pub(in crate::panels::pcb_layers) fn set_label(i18n: &I18n, choice: &SetChoice) -> String {
    match choice {
        SetChoice::Builtin(set) => {
            i18n.message_plain(ids::PCB_LAYER_SET, &[("set", Arg::from(set.token()))])
        }
        SetChoice::Saved(name) => name.clone(),
    }
}

/// What the closed Set dropdown reads: "Set: All", or the saved set's name.
pub(in crate::panels::pcb_layers) fn set_current(panel: &PcbLayersPanel) -> String {
    let i18n = &panel.i18n;
    let set = match &panel.layer_set {
        Some(LayerSetId::Saved(name)) => name.clone(),
        Some(LayerSetId::Builtin(set)) => {
            i18n.message_plain(ids::PCB_LAYER_SET_SHORT, &[("set", Arg::from(set.token()))])
        }
        None => i18n.message_plain(ids::PCB_LAYER_SET_SHORT, &[("set", Arg::from("all"))]),
    };
    i18n.message_plain(ids::PCB_LAYERS_SET_CURRENT, &[("set", Arg::from(set))])
}

/// A set picked in the Set dropdown: a built-in one is a board view event,
/// a saved one goes through the panel.
pub(in crate::panels::pcb_layers) fn set_picked(choice: SetChoice) -> PanelMsg {
    match choice {
        SetChoice::Builtin(set) => PanelMsg::PcbLayers(PcbViewMsg::BuiltinLayerSetShown(set)),
        SetChoice::Saved(name) => PanelMsg::LayersPanel(LayersPanelMsg::SavedSetShown(name)),
    }
}

/// A layer picked in the active layer dropdown.
pub(in crate::panels::pcb_layers) fn active_picked(layer: PcbLayer) -> PanelMsg {
    PanelMsg::PcbLayers(PcbViewMsg::ActiveLayerPicked(layer))
}

/// A dropdown drawn into the card: no field fill or border of its own, the
/// hover wash, and the placeholder in the chevron's ink.
fn bare(theme: &Theme, status: PickStatus) -> PickStyle {
    let look = DropdownLook::of_theme(theme);
    let mut style = look.field_style(status);
    let hovered = matches!(status, PickStatus::Hovered | PickStatus::Opened { .. });
    style.background = Background::Color(if hovered {
        look.hover
    } else {
        Color::TRANSPARENT
    });
    style.border = Border {
        radius: style.border.radius,
        ..Border::default()
    };
    style.placeholder_color = style.handle_color;
    style
}

pub(super) fn view<'a>(panel: &'a PcbLayersPanel, ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &panel.i18n;
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let caption = text(caps(i18n, ids::PCB_LAYERS_ACTIVE))
        .size(TS.caption)
        .color(r.ink4);
    let dot = dot_face(panel.active.map(|layer| panel.colour(layer)), false, ctx);
    let label = move |layer: &PcbLayer| layer_style::layer_name(i18n, *layer).into_string();
    let active = pick_list(panel.active, panel.board_layers.clone(), label)
        .graphite()
        .placeholder_label(t!(i18n, ids::PCB_LAYERS_NO_ACTIVE))
        .on_select(active_picked)
        .font(medium())
        .padding([SP.xs, SP.xs])
        .style(bare)
        .width(Length::Fill);
    let active = hint_tooltip(
        active.boxed(),
        t!(i18n, ids::PCB_LAYERS_ACTIVE_LAYER),
        tokens,
        Position::Top,
    );
    let line = ti(tokens.border);
    let divider = container(Space::new())
        .width(1.0)
        .height(DIVIDER)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        });
    let sets = pick_list(
        None::<SetChoice>,
        set_choices(panel),
        move |choice: &SetChoice| set_label(i18n, choice),
    )
    .graphite()
    // The caption carries a saved set's own name.
    .placeholder_label(signex_i18n::UserText::new(set_current(panel)))
    .on_select(set_picked)
    .padding([SP.xs, SP.xs])
    .style(bare);
    let sets = hint_tooltip(
        sets.boxed(),
        t!(i18n, ids::PCB_LAYERS_SETS),
        tokens,
        Position::Top,
    );
    let card = container(
        row![caption, dot, active, divider, sets]
            .spacing(ROW_GAP)
            .align_y(Alignment::Center),
    )
    .padding(Padding {
        left: SP.m,
        right: SP.xs,
        ..Padding::ZERO
    })
    .width(Length::Fill)
    .center_y(ACTIVE_CARD)
    .style(field(tokens, false));
    container(card)
        .padding(Padding {
            top: SP.xs,
            bottom: SP.s,
            left: SP.m,
            right: SP.m,
        })
        .width(Length::Fill)
        .boxed()
}
