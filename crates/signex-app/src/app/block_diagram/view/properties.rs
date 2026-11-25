//! Properties of what is selected (mockup row 171): a block's name,
//! function, note, shape, position and size, group, rails, key specs,
//! colour, linked sheet and ports; a connection's ends, interface and
//! label; a group's name and colour; a port's facts. Text is committed with
//! Enter; every commit is one edit through the tab's commit path.

use iced::widget::{Column, Row, Space, column, container, pick_list, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{MessageId, UserText, ids};
use signex_types::block_diagram::{
    Block, BlockDiagram, BlockGroup, BlockPort, BlockShape, Connection, Endpoint, Interface,
    LinkState, PortDirection, PortSide,
};
use signex_types::theme::Color;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{Label, button, text, text_input};
use uuid::Uuid;

use super::{Look, SP, TS, caption};
use crate::app::block_diagram::TabEvent;
use crate::app::block_diagram::fields::{mm_text, rails_text, specs_text};
use crate::app::block_diagram::state::{BlockDiagramEditor, DiagramItem, PropertyField};
use crate::choice::{Choice, chosen};
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// A swatch's side, in body-text ems.
const SWATCH_EMS: f32 = 1.2;

type Items<'a> = Column<'a, TabEvent>;

pub(super) fn view<'a>(look: Look<'a>, editor: &'a BlockDiagramEditor) -> Element<'a, TabEvent> {
    let diagram = &editor.diagram;
    let mut list: Items<'a> = column![caption(look, ids::BLOCK_DIAGRAM_PROPERTIES)].spacing(SP.xs);
    let editable = !editor.is_read_only();
    list = match editor.single_selection() {
        Some(DiagramItem::Block(id)) => match diagram.block(id) {
            Some(block) => block_rows(look, editor, block, editable, list),
            None => list,
        },
        Some(DiagramItem::Port(id)) => match port_of(diagram, id) {
            Some((block, port)) => port_rows(look, block, port, list),
            None => list,
        },
        Some(DiagramItem::Connection(id)) => match diagram.connection(id) {
            Some(link) => connection_rows(look, editor, link, editable, list),
            None => list,
        },
        Some(DiagramItem::Group(id)) => match diagram.group(id) {
            Some(group) => group_rows(look, editor, group, editable, list),
            None => list,
        },
        None if editor.selection.is_empty() => {
            list.push(hint(look, ids::BLOCK_DIAGRAM_PROP_NOTHING))
        }
        None => list.push(hint_count(look, editor.selection.len())),
    };
    container(scrollable(list.padding(SP.s)).height(Length::Fill))
        .height(Length::Fill)
        .style(crate::styles::panel_region(look.tokens))
        .boxed()
}

fn port_of(diagram: &BlockDiagram, port: Uuid) -> Option<(&Block, &BlockPort)> {
    diagram
        .blocks
        .iter()
        .find_map(|b| b.port(port).map(|p| (b, p)))
}

fn hint<'a>(look: Look<'a>, id: MessageId) -> Element<'a, TabEvent> {
    text(look.t(id))
        .size(TS.dense)
        .color(ti(look.tokens.text_secondary))
        .boxed()
}

fn hint_count<'a>(look: Look<'a>, count: usize) -> Element<'a, TabEvent> {
    let line = signex_i18n::t!(look.i18n, ids::BLOCK_DIAGRAM_PROP_SEVERAL, "count" => count);
    text(line)
        .size(TS.dense)
        .color(ti(look.tokens.text_secondary))
        .boxed()
}

/// A name on the left, its value on the right.
fn labelled<'a>(
    look: Look<'a>,
    name: MessageId,
    value: Element<'a, TabEvent>,
) -> Element<'a, TabEvent> {
    row![
        text(look.t(name))
            .size(TS.dense)
            .color(resolve(look.tokens).ink3)
            .width(Length::FillPortion(2))
            .boxed(),
        container(value).width(Length::FillPortion(3)).boxed(),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// A read-only value.
fn fact<'a>(look: Look<'a>, name: MessageId, value: impl Into<Label<'a>>) -> Element<'a, TabEvent> {
    let value = text(value)
        .size(TS.dense)
        .color(ti(look.tokens.text))
        .boxed();
    labelled(look, name, value)
}

fn user(value: impl Into<String>) -> Label<'static> {
    UserText::new(value.into()).into()
}

/// A text field: the draft while one is typed, else `current`.
fn field<'a>(
    look: Look<'a>,
    editor: &'a BlockDiagramEditor,
    name: MessageId,
    which: PropertyField,
    current: String,
    editable: bool,
) -> Element<'a, TabEvent> {
    let shown = editor.drafts.get(&which).cloned().unwrap_or(current);
    let mut input = text_input(look.t(name), shown)
        .style(crate::panels::form_look::text_input_style)
        .size(TS.dense)
        .padding(SP.xs);
    if editable {
        input = input
            .on_input(move |typed| TabEvent::FieldEdited(which, typed))
            .on_submit(TabEvent::FieldCommitted(which));
    }
    labelled(look, name, input.boxed())
}

fn block_rows<'a>(
    look: Look<'a>,
    editor: &'a BlockDiagramEditor,
    block: &'a Block,
    editable: bool,
    list: Items<'a>,
) -> Items<'a> {
    let f = |name, which, current: String| field(look, editor, name, which, current, editable);
    let mut list = list
        .push(f(
            ids::BLOCK_DIAGRAM_PROP_NAME,
            PropertyField::BlockName,
            block.name.clone(),
        ))
        .push(f(
            ids::BLOCK_DIAGRAM_PROP_FUNCTION,
            PropertyField::BlockFunction,
            block.function.clone(),
        ))
        .push(f(
            ids::BLOCK_DIAGRAM_PROP_NOTE,
            PropertyField::BlockNote,
            block.note.clone(),
        ))
        .push(labelled(
            look,
            ids::BLOCK_DIAGRAM_PROP_SHAPE,
            shape_picker(look, block, editable),
        ))
        .push(f(
            ids::BLOCK_DIAGRAM_PROP_X,
            PropertyField::BlockX,
            mm_text(block.position.x),
        ))
        .push(f(
            ids::BLOCK_DIAGRAM_PROP_Y,
            PropertyField::BlockY,
            mm_text(block.position.y),
        ))
        .push(f(
            ids::BLOCK_DIAGRAM_PROP_WIDTH,
            PropertyField::BlockWidth,
            mm_text(block.width_mm),
        ))
        .push(f(
            ids::BLOCK_DIAGRAM_PROP_HEIGHT,
            PropertyField::BlockHeight,
            mm_text(block.height_mm),
        ))
        .push(labelled(
            look,
            ids::BLOCK_DIAGRAM_PROP_GROUP,
            group_picker(look, &editor.diagram, block, editable),
        ))
        .push(f(
            ids::BLOCK_DIAGRAM_PROP_RAILS,
            PropertyField::BlockRails,
            rails_text(&block.rails),
        ))
        .push(f(
            ids::BLOCK_DIAGRAM_PROP_SPECS,
            PropertyField::BlockSpecs,
            specs_text(&block.specs),
        ))
        .push(labelled(
            look,
            ids::BLOCK_DIAGRAM_PROP_COLOR,
            swatches(look, editable),
        ));
    list = match &block.sheet {
        Some(link) => {
            let state = match link.state {
                LinkState::Linked => ids::BLOCK_DIAGRAM_SHEET_LINKED,
                LinkState::Unlinked => ids::BLOCK_DIAGRAM_SHEET_UNLINKED,
            };
            list.push(fact(
                look,
                ids::BLOCK_DIAGRAM_PROP_SHEET,
                user(link.path.clone()),
            ))
            .push(fact(
                look,
                ids::BLOCK_DIAGRAM_PROP_SHEET_STATE,
                look.t(state),
            ))
        }
        None => list.push(fact(
            look,
            ids::BLOCK_DIAGRAM_PROP_SHEET,
            look.t(ids::BLOCK_DIAGRAM_SHEET_NONE),
        )),
    };
    list = list.push(caption(look, ids::BLOCK_DIAGRAM_PROP_PORTS));
    if block.ports.is_empty() {
        return list.push(hint(look, ids::BLOCK_DIAGRAM_PROP_NO_PORTS));
    }
    for port in &block.ports {
        let line = signex_i18n::t!(
            look.i18n,
            ids::BLOCK_DIAGRAM_PORT_LINE,
            "name" => port.name.as_str(),
            "direction" => look.t(direction_label(port.direction)).into_string()
        );
        list = list.push(
            text(line)
                .size(TS.dense)
                .color(ti(look.tokens.text))
                .boxed(),
        );
    }
    list
}

fn port_rows<'a>(
    look: Look<'a>,
    block: &'a Block,
    port: &'a BlockPort,
    list: Items<'a>,
) -> Items<'a> {
    list.push(fact(
        look,
        ids::BLOCK_DIAGRAM_PROP_NAME,
        user(port.name.clone()),
    ))
    .push(fact(
        look,
        ids::BLOCK_DIAGRAM_PROP_BLOCK,
        user(block.name.clone()),
    ))
    .push(fact(
        look,
        ids::BLOCK_DIAGRAM_PROP_SIDE,
        look.t(side_label(port.side)),
    ))
    .push(fact(
        look,
        ids::BLOCK_DIAGRAM_PROP_DIRECTION,
        look.t(direction_label(port.direction)),
    ))
}

/// A connection end as "Block" or "Block · Port".
fn end_text(diagram: &BlockDiagram, end: Endpoint) -> String {
    let Some(block) = diagram.block(end.block) else {
        return end.block.to_string();
    };
    match end.port.and_then(|p| block.port(p)) {
        Some(port) => format!("{} · {}", block.name, port.name),
        None => block.name.clone(),
    }
}

fn connection_rows<'a>(
    look: Look<'a>,
    editor: &'a BlockDiagramEditor,
    link: &'a Connection,
    editable: bool,
    list: Items<'a>,
) -> Items<'a> {
    let diagram = &editor.diagram;
    let mut list = list
        .push(fact(
            look,
            ids::BLOCK_DIAGRAM_PROP_FROM,
            user(end_text(diagram, link.from)),
        ))
        .push(fact(
            look,
            ids::BLOCK_DIAGRAM_PROP_TO,
            user(end_text(diagram, link.to)),
        ))
        .push(labelled(
            look,
            ids::BLOCK_DIAGRAM_PROP_INTERFACE,
            interface_picker(look, link, editable),
        ));
    if let Interface::Other(name) = &link.interface {
        list = list.push(field(
            look,
            editor,
            ids::BLOCK_DIAGRAM_PROP_INTERFACE_NAME,
            PropertyField::ConnectionInterface,
            name.clone(),
            editable,
        ));
    }
    list = list.push(field(
        look,
        editor,
        ids::BLOCK_DIAGRAM_PROP_LABEL,
        PropertyField::ConnectionLabel,
        link.label.clone(),
        editable,
    ));
    match &link.derived_from {
        Some(source) => list.push(fact(
            look,
            ids::BLOCK_DIAGRAM_PROP_DERIVED,
            user(source.clone()),
        )),
        None => list,
    }
}

fn group_rows<'a>(
    look: Look<'a>,
    editor: &'a BlockDiagramEditor,
    group: &'a BlockGroup,
    editable: bool,
    list: Items<'a>,
) -> Items<'a> {
    let members = editor
        .diagram
        .blocks
        .iter()
        .filter(|b| b.group == Some(group.id))
        .count();
    let count = signex_i18n::t!(look.i18n, ids::BLOCK_DIAGRAM_GROUP_BLOCKS, "count" => members);
    list.push(field(
        look,
        editor,
        ids::BLOCK_DIAGRAM_PROP_NAME,
        PropertyField::GroupName,
        group.name.clone(),
        editable,
    ))
    .push(fact(look, ids::BLOCK_DIAGRAM_PROP_BLOCKS, count))
    .push(labelled(
        look,
        ids::BLOCK_DIAGRAM_PROP_COLOR,
        swatches(look, editable),
    ))
}

fn shape_label(shape: BlockShape) -> MessageId {
    match shape {
        BlockShape::Rectangle => ids::BLOCK_DIAGRAM_SHAPE_RECTANGLE,
        BlockShape::RoundedRectangle => ids::BLOCK_DIAGRAM_SHAPE_ROUNDED,
        BlockShape::Ellipse => ids::BLOCK_DIAGRAM_SHAPE_ELLIPSE,
        BlockShape::Hexagon => ids::BLOCK_DIAGRAM_SHAPE_HEXAGON,
    }
}

fn side_label(side: PortSide) -> MessageId {
    match side {
        PortSide::Left => ids::BLOCK_DIAGRAM_SIDE_LEFT,
        PortSide::Right => ids::BLOCK_DIAGRAM_SIDE_RIGHT,
        PortSide::Top => ids::BLOCK_DIAGRAM_SIDE_TOP,
        PortSide::Bottom => ids::BLOCK_DIAGRAM_SIDE_BOTTOM,
    }
}

fn direction_label(direction: PortDirection) -> MessageId {
    match direction {
        PortDirection::Input => ids::BLOCK_DIAGRAM_DIRECTION_INPUT,
        PortDirection::Output => ids::BLOCK_DIAGRAM_DIRECTION_OUTPUT,
        PortDirection::Bidirectional => ids::BLOCK_DIAGRAM_DIRECTION_BOTH,
    }
}

/// The name of a known interface, by its token; the token itself for one
/// with no message.
fn interface_label(token: &str) -> MessageId {
    match token {
        "signal" => ids::BLOCK_DIAGRAM_INTERFACE_SIGNAL,
        "bus" => ids::BLOCK_DIAGRAM_INTERFACE_BUS,
        "power" => ids::BLOCK_DIAGRAM_INTERFACE_POWER,
        "analog" => ids::BLOCK_DIAGRAM_INTERFACE_ANALOG,
        "spi" => ids::BLOCK_DIAGRAM_INTERFACE_SPI,
        "i2c" => ids::BLOCK_DIAGRAM_INTERFACE_I2C,
        "uart" => ids::BLOCK_DIAGRAM_INTERFACE_UART,
        "usb" => ids::BLOCK_DIAGRAM_INTERFACE_USB,
        "lvds" => ids::BLOCK_DIAGRAM_INTERFACE_LVDS,
        _ => ids::BLOCK_DIAGRAM_INTERFACE_OTHER,
    }
}

fn shape_picker<'a>(look: Look<'a>, block: &Block, editable: bool) -> Element<'a, TabEvent> {
    let choices = Choice::named(&BlockShape::ALL, shape_label, look.i18n);
    let selected = chosen(&choices, &block.shape);
    let list = pick_list(selected, choices, |c: &Choice<BlockShape>| c.label.clone()).graphite();
    let list = if editable {
        list.on_select(|c: Choice<BlockShape>| TabEvent::ShapePicked(c.value))
    } else {
        list
    };
    list.width(Length::Fill).boxed()
}

fn interface_picker<'a>(
    look: Look<'a>,
    link: &Connection,
    editable: bool,
) -> Element<'a, TabEvent> {
    let tokens: Vec<&'static str> = Interface::known()
        .filter_map(|i| i.known_token())
        .chain(std::iter::once(OTHER_TOKEN))
        .collect();
    let choices = Choice::named(&tokens, interface_label, look.i18n);
    let current = link.interface.known_token().unwrap_or(OTHER_TOKEN);
    let selected = chosen(&choices, &current);
    let list = pick_list(selected, choices, |c: &Choice<&'static str>| {
        c.label.clone()
    })
    .graphite();
    let list = if editable {
        list.on_select(|c: Choice<&'static str>| TabEvent::InterfacePicked(c.value))
    } else {
        list
    };
    list.width(Length::Fill).boxed()
}

/// The token Properties lists for "another interface"; picking it keeps
/// the name typed below it.
const OTHER_TOKEN: &str = "other";

fn group_picker<'a>(
    look: Look<'a>,
    diagram: &BlockDiagram,
    block: &Block,
    editable: bool,
) -> Element<'a, TabEvent> {
    let none = Choice {
        value: None,
        label: look.i18n.message_plain(ids::BLOCK_DIAGRAM_GROUP_NONE, &[]),
    };
    let choices: Vec<Choice<Option<Uuid>>> = std::iter::once(none)
        .chain(diagram.groups.iter().map(|g| Choice {
            value: Some(g.id),
            label: g.name.clone(),
        }))
        .collect();
    let selected = chosen(&choices, &block.group);
    let list = pick_list(selected, choices, |c: &Choice<Option<Uuid>>| {
        c.label.clone()
    })
    .graphite();
    let list = if editable {
        list.on_select(|c: Choice<Option<Uuid>>| TabEvent::GroupPicked(c.value))
    } else {
        list
    };
    list.width(Length::Fill).boxed()
}

/// The theme's colour, then the accent and status colours of the theme.
fn swatches<'a>(look: Look<'a>, editable: bool) -> Element<'a, TabEvent> {
    let tokens = look.tokens;
    let colors: [Option<Color>; 5] = [
        None,
        Some(tokens.accent),
        Some(tokens.success),
        Some(tokens.warning),
        Some(tokens.error),
    ];
    let mut line: Row<'a, TabEvent> = Row::new().spacing(SP.xs);
    for color in colors {
        line = line.push(chip(look, color, editable));
    }
    line.boxed()
}

/// One swatch; the theme's colour shows as the panel with its border.
fn chip<'a>(look: Look<'a>, color: Option<Color>, editable: bool) -> Element<'a, TabEvent> {
    let side = TS.body * SWATCH_EMS;
    let line = ti(look.tokens.border);
    let fill = color.map_or(ti(look.tokens.panel_bg), ti);
    let face = button(Space::new().width(side).height(side))
        .padding(0)
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                color: line,
                width: crate::styles::HAIRLINE,
                radius: (side / 4.0).into(),
            },
            ..Default::default()
        });
    let face = if editable {
        face.on_press(TabEvent::ColorPicked(color))
    } else {
        face
    };
    face.boxed()
}
