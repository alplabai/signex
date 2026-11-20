//! The Pad panel (mockup row 97): Net Information; Properties (designator, net, the
//! layer it sits on, position, rotation); Pad Stack (the illustration, the
//! copper as a Layer × size table, the hole, Paste and Solder mask); and
//! the rows the board does not store yet.
//!
//! A surface pad sits on one layer: its stack is one copper row, with no
//! stack tiles and no hole (maintainer decision on mockup rows 97 and 125).
//! A through-hole pad keeps the Simple / Top-Middle-Bottom / Full Stack
//! tiles (#2401) and shows a row per layer its stack sets.

use iced::widget::{Row, column, container, row};
use iced::{Element, Length, Theme, Widget as _};
use signex_i18n::{MessageId, UserText, ids};
use signex_types::pcb_stack::StackMode;
use signex_widgets::enable_section::EnableSection;
use signex_widgets::{CheckboxLabel as _, Label, checkbox, text};

use super::drawing::pad as drawing;
use super::linked::{Segment, segments};
use super::mask::{greyed_unless, subhead};
use super::rows::{
    Look, Section, chip, dropdown, field, labelled, mm_field, net_row, pair, stack, value_box,
    with_tip,
};
use super::stack_rows::{illustration, linked, mode_tiles, rows_column, unstored, unstored_row};
use super::{PadFacts, PcbPrimitiveMsg, PrimitiveField as F, SP, TS, format_number, net_info};
use crate::panels::enable_section::enable_section_style;
use crate::panels::{PanelContext, PanelMsg, form_look};
use crate::pcb_canvas::layer_style;

/// Width shares of the copper table's columns: layer, X, Y, shape.
const TABLE_PORTIONS: [u16; 4] = [3, 2, 2, 3];

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a PadFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    let common = &f.common;
    stack(vec![
        net_info::section(look, common),
        properties(look, f),
        pad_stack(look, f),
        not_stored(look, f),
    ])
}

/// A surface pad: one copper layer, no hole.
fn is_surface(f: &PadFacts) -> bool {
    !f.has_hole
}

fn properties<'a>(look: Look<'a>, f: &'a PadFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_PROPERTIES);
    s.push(
        common,
        ids::PCB_PRIM_DESIGNATOR,
        labelled(
            look,
            ids::PCB_PRIM_DESIGNATOR,
            field(common, F::Designator, f.designator.clone(), None, look),
        ),
    );
    s.push(common, ids::PCB_PRIM_NET, net_row(look, common));
    let layer: Element<'a, PanelMsg> = match f.layer.as_ref() {
        Some((name, colour_of)) => {
            let shown = value_box(look, UserText::new(name.clone()));
            let content = match colour_of {
                Some(layer) => row![
                    chip(layer_style::layer_colour(*layer, look.tokens())),
                    shown
                ]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center)
                .boxed(),
                None => shown,
            };
            with_tip(look, content, ids::PCB_PRIM_PAD_LAYER_UNSUPPORTED)
        }
        None => value_box(look, UserText::new(String::new())),
    };
    s.push(
        common,
        ids::PCB_PRIM_LAYER,
        linked(
            look,
            common,
            drawing::LAYER,
            labelled(look, ids::PCB_PRIM_LAYER, layer),
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
    let degrees = f.rotation.map(format_number);
    s.push(
        common,
        ids::PCB_PRIM_ROTATION,
        labelled(
            look,
            ids::PCB_PRIM_ROTATION,
            field(common, F::Rotation, degrees, None, look),
        ),
    );
    s.finish()
}

fn pad_stack<'a>(look: Look<'a>, f: &'a PadFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let surface = is_surface(f);
    let mut s = Section::new(look, ids::PCB_PRIM_PAD_STACK);
    if !surface {
        s.push(
            common,
            ids::PCB_PRIM_STACK_SIMPLE,
            mode_tiles(look, common, f.mode, true, ids::PCB_PRIM_STACK_TIP),
        );
    }
    s.push_row(illustration(look, common, drawing::illustration(look, f)));
    s.push(
        common,
        ids::PCB_PRIM_COPPER,
        subhead(look, ids::PCB_PRIM_COPPER),
    );
    s.push(common, ids::PCB_PRIM_COPPER, copper_table(look, f));
    if !surface {
        s.push(common, ids::PCB_PRIM_HOLE, hole_row(look, f));
    }
    s.push(common, ids::PCB_PRIM_PASTE, paste(look, f));
    s.push(common, ids::PCB_PRIM_SOLDER_TENTED, solder(look, f));
    s.finish()
}

/// One row of the copper table: the layer, its X and Y fields and the
/// shape.
struct CopperRow<'a> {
    label: Label<'a>,
    x: (F, Option<f64>),
    y: (F, Option<f64>),
}

/// The copper as Layer × (X-Size, Y-Size, Shape), typed in place: one row
/// for a surface pad's layer, "All layers" for a Simple stack, else a row
/// per layer the stack sets. The shape is one per pad: picked in any row,
/// it sets every layer.
fn copper_table<'a>(look: Look<'a>, f: &'a PadFacts) -> Element<'a, PanelMsg> {
    let common = &f.common;
    let rows: Vec<CopperRow<'a>> = if is_surface(f) {
        let name = f
            .layer
            .as_ref()
            .map(|(name, _)| name.clone())
            .unwrap_or_default();
        vec![CopperRow {
            label: UserText::new(name).into(),
            x: (F::SizeX, f.size_x),
            y: (F::SizeY, f.size_y),
        }]
    } else if f.mode == Some(StackMode::Simple) || f.layer_sizes.is_empty() {
        vec![CopperRow {
            label: look.t(ids::PCB_PRIM_ALL_LAYERS).into(),
            x: (F::SizeX, f.size_x),
            y: (F::SizeY, f.size_y),
        }]
    } else {
        f.layer_sizes
            .iter()
            .map(|r| CopperRow {
                label: UserText::new(r.label.clone()).into(),
                x: (F::LayerSize(r.layer), r.x),
                y: (F::LayerSizeY(r.layer), r.y),
            })
            .collect()
    };
    let several = rows.len() > 1;
    let head = |id: MessageId| text(look.t(id)).size(TS.dense).color(look.muted);
    let mut table = column![table_row(
        head(ids::PCB_PRIM_LAYER).boxed(),
        head(ids::PCB_PRIM_X_SIZE).boxed(),
        head(ids::PCB_PRIM_Y_SIZE).boxed(),
        head(ids::PCB_PRIM_SHAPE).boxed(),
    )]
    .spacing(SP.xs);
    for r in rows {
        let target = common.target.clone();
        let shape = dropdown(common, &f.shapes, f.shape.clone(), move |choice| {
            PcbPrimitiveMsg::ShapePicked {
                target: target.clone(),
                shape: choice.shape,
            }
        });
        let shape = if several {
            with_tip(look, shape, ids::PCB_PRIM_SHAPE_ALL_LAYERS_TIP)
        } else {
            shape
        };
        table = table.push(table_row(
            text(r.label).size(TS.body).color(look.ink).boxed(),
            linked(
                look,
                common,
                drawing::SIZE_X,
                mm_field(common, r.x.0, r.x.1, look),
            ),
            linked(
                look,
                common,
                drawing::SIZE_Y,
                mm_field(common, r.y.0, r.y.1, look),
            ),
            linked(look, common, drawing::SHAPE, shape),
        ));
    }
    container(table)
        .padding([SP.xs, super::rows::PAD_X])
        .width(Length::Fill)
        .boxed()
}

fn table_row<'a>(
    layer: Element<'a, PanelMsg>,
    x: Element<'a, PanelMsg>,
    y: Element<'a, PanelMsg>,
    shape: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    let mut cells: Row<Element<'a, PanelMsg>> =
        Row::new().spacing(SP.s).align_y(iced::Alignment::Center);
    for (cell, portion) in [layer, x, y, shape].into_iter().zip(TABLE_PORTIONS) {
        cells = cells.push(container(cell).width(Length::FillPortion(portion)).boxed());
    }
    cells.width(Length::Fill).boxed()
}

/// Hole and Plated. Plating follows the pad type, which the footprint
/// sets: read-only here.
fn hole_row<'a>(look: Look<'a>, f: &'a PadFacts) -> Element<'a, PanelMsg> {
    let common = &f.common;
    let plated = checkbox::<PanelMsg, Theme, iced::Renderer>(f.plated == Some(true))
        .labelled(look.t(ids::PCB_PRIM_PLATED))
        .size(TS.body)
        .text_size(TS.body)
        .style(if f.plated.is_none() {
            form_look::mixed_checkbox_style
        } else {
            form_look::checkbox_style
        });
    labelled(
        look,
        ids::PCB_PRIM_HOLE,
        row![
            container(linked(
                look,
                common,
                drawing::HOLE,
                mm_field(common, F::HoleSize, f.hole, look),
            ))
            .width(Length::Fill),
            with_tip(look, plated.boxed(), ids::PCB_PRIM_PLATED_TIP),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .boxed(),
    )
}

/// Rule / Manual and Expansion of a pad mask: what the board does not
/// store per pad yet, so greyed with that reason.
fn mask_rows<'a>(look: Look<'a>, unit: Option<MessageId>) -> Element<'a, PanelMsg> {
    let source = segments(vec![
        Segment {
            label: look.t(ids::PCB_PRIM_RULE),
            is_on: true,
            msg: None,
        },
        Segment {
            label: look.t(ids::PCB_PRIM_MANUAL),
            is_on: false,
            msg: None,
        },
    ]);
    let expansion = value_box(
        look,
        unit.map_or_else(
            || UserText::new(String::new()).into(),
            |u| Label::from(look.t(u)),
        ),
    );
    let rows = rows_column(vec![
        labelled(look, ids::PCB_PRIM_SOURCE, source),
        labelled(look, ids::PCB_PRIM_EXPANSION, expansion),
    ]);
    with_tip(
        look,
        greyed_unless(look, false, rows),
        ids::PCB_PRIM_MASK_UNSUPPORTED,
    )
}

/// The Paste subsection: its switch heads it, ticked for a surface pad.
fn paste<'a>(look: Look<'a>, f: &'a PadFacts) -> Element<'a, PanelMsg> {
    let header = EnableSection::new(look.t(ids::PCB_PRIM_PASTE), is_surface(f))
        .tip(look.t(ids::PCB_PRIM_MASK_UNSUPPORTED));
    linked(
        look,
        &f.common,
        drawing::PASTE,
        header.view(
            enable_section_style(look.tokens()),
            mask_rows(look, Some(ids::PCB_PRIM_PERCENT)),
        ),
    )
}

/// The Solder mask subsection, headed by its Tented switch (off: the pad
/// has an opening).
fn solder<'a>(look: Look<'a>, f: &'a PadFacts) -> Element<'a, PanelMsg> {
    let header = EnableSection::new(look.t(ids::PCB_PRIM_SOLDER_TENTED), false)
        .tip(look.t(ids::PCB_PRIM_MASK_UNSUPPORTED));
    linked(
        look,
        &f.common,
        drawing::MASK,
        header.view(enable_section_style(look.tokens()), mask_rows(look, None)),
    )
}

/// Electrical Type, Propagation Delay, Pin Package Length, Jumper, Relief
/// and Testpoint: the board does not store them per pad yet.
fn not_stored<'a>(look: Look<'a>, f: &'a PadFacts) -> Option<Element<'a, PanelMsg>> {
    let dash = || look.t(ids::PCB_PRIM_NO_VALUE);
    let rows = [
        (
            ids::PCB_PRIM_ELECTRICAL_TYPE,
            look.t(ids::PCB_PRIM_ELECTRICAL_LOAD),
            ids::PCB_PRIM_ELECTRICAL_UNSUPPORTED,
        ),
        (
            ids::PCB_PRIM_PROPAGATION_DELAY,
            dash(),
            ids::PCB_PRIM_DELAY_UNKNOWN,
        ),
        (
            ids::PCB_PRIM_PIN_PACKAGE_LENGTH,
            dash(),
            ids::PCB_PRIM_PIN_LENGTH_UNSUPPORTED,
        ),
        (
            ids::PCB_PRIM_JUMPER,
            dash(),
            ids::PCB_PRIM_JUMPER_UNSUPPORTED,
        ),
        (
            ids::PCB_PRIM_RELIEF,
            look.t(ids::PCB_PRIM_DIRECT),
            ids::PCB_PRIM_RELIEF_UNSUPPORTED,
        ),
        (
            ids::PCB_PRIM_TESTPOINT,
            dash(),
            ids::PCB_PRIM_TESTPOINT_UNSUPPORTED,
        ),
    ];
    unstored(
        look,
        &f.common,
        rows.into_iter()
            .map(|(label, value, tip)| unstored_row(look, label, value, tip))
            .collect(),
    )
}
