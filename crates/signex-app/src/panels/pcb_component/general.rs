//! The Location and Properties sections of the General tab.

use iced::widget::{Column, Row, Space, container};
use iced::{Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_engine::pcb::component::LocationField;
use signex_engine::pcb::designators::FootprintTextField;
use signex_i18n::{MessageId, ids, t};
use signex_types::format::parse_pcb_layer;
use signex_types::pcb_layer::BoardSide;
use signex_widgets::{button, text};

use super::rows::{self, Form};
use super::{ComponentField, PcbComponentFacts, PcbComponentMsg, PcbFootprintFacts, Shared};
use crate::panels::{PanelContext, PanelMsg, form_look};
use crate::pcb_canvas::layer_style;
use crate::styles::{PillTone, Radii, Spacing, TypeScale, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The key of the parameter the Description row edits.
pub(super) const DESCRIPTION_KEY: &str = "Description";

/// Push Location and Properties onto `col`, then the notes of a single
/// component's text.
pub(super) fn push_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    facts: &'a PcbComponentFacts,
    single: Option<&'a PcbFootprintFacts>,
) -> Column<Element<'a, PanelMsg>> {
    let form = Form::new(ctx, facts);
    let sections = [
        rows::section(
            &form,
            "pcb_component_location",
            ids::PCB_COMPONENT_SECTION_LOCATION,
            location(&form, facts),
        ),
        rows::section(
            &form,
            "pcb_component_properties",
            ids::PCB_COMPONENT_SECTION_PROPERTIES,
            properties(&form, facts, single),
        ),
    ];
    for section in sections.into_iter().flatten() {
        col = col.push(section);
    }
    if let Some(fp) = single {
        let tokens = &ctx.tokens;
        let (muted, warn) = (
            resolve(tokens).ink3,
            crate::styles::pill_colors(tokens, PillTone::Warn).1,
        );
        for (line, is_warning) in super::text::notes(fp, &ctx.i18n) {
            col = col.push_boxed(
                container(
                    text(line)
                        .size(TS.dense)
                        .color(if is_warning { warn } else { muted }),
                )
                .padding([SP.xs, SP.l])
                .width(Length::Fill),
            );
        }
    }
    col
}

fn location<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Vec<Element<'a, PanelMsg>> {
    let mut rows = Vec::new();
    for (field, label, value) in [
        (LocationField::X, ids::PCB_COMPONENT_X, &facts.x),
        (LocationField::Y, ids::PCB_COMPONENT_Y, &facts.y),
        (
            LocationField::Rotation,
            ids::PCB_COMPONENT_ROTATION,
            &facts.rotation,
        ),
    ] {
        let label = form.text(label);
        if form.shows(&label) {
            let field = ComponentField::Location(field);
            let control = draft_input(facts, field, value.same().map(|v| rows::number_text(*v)));
            rows.push(rows::row(form, label, control));
        }
    }
    let label = form.text(ids::PCB_COMPONENT_LOCKED);
    if form.shows(&label) {
        let toggled = (!facts.read_only).then(|| {
            PanelMsg::PcbComponent(PcbComponentMsg::LockToggled {
                board: facts.board.clone(),
                footprints: facts.footprints.clone(),
                locked: facts.locked != Shared::Same(true),
            })
        });
        let control = form.with_tip(
            rows::check(&facts.locked, toggled),
            ids::PCB_COMPONENT_LOCKED_TIP,
        );
        rows.push(rows::row(form, label, control));
    }
    rows
}

/// An input whose text is `field`'s draft, else `stored` (blank when the
/// components differ); editable while placement edits run.
pub(super) fn draft_input<'a>(
    facts: &'a PcbComponentFacts,
    field: ComponentField,
    stored: Option<String>,
) -> Element<'a, PanelMsg> {
    let shown = facts.draft(&field).cloned().or(stored).unwrap_or_default();
    if !facts.is_editable() {
        return rows::input(shown, None);
    }
    let (board, footprints) = (facts.board.clone(), facts.footprints.clone());
    let submitted = PanelMsg::PcbComponent(PcbComponentMsg::FieldSubmitted {
        board: board.clone(),
        footprints: footprints.clone(),
        field: field.clone(),
    });
    let typed = move |text| {
        PanelMsg::PcbComponent(PcbComponentMsg::FieldTyped {
            board: board.clone(),
            footprints: footprints.clone(),
            field: field.clone(),
            text,
        })
    };
    rows::input(shown, Some((Box::new(typed), submitted)))
}

fn properties<'a>(
    form: &Form<'a>,
    facts: &'a PcbComponentFacts,
    single: Option<&'a PcbFootprintFacts>,
) -> Vec<Element<'a, PanelMsg>> {
    let mut rows = Vec::new();
    let label = form.text(ids::PCB_COMPONENT_LAYER);
    if form.shows(&label) {
        rows.push(rows::row(form, label, layer_picker(form, facts)));
    }
    for field in [FootprintTextField::Designator, FootprintTextField::Value] {
        rows.extend(super::text::text_row(form, facts, single, field));
    }
    let label = form.text(ids::PCB_COMPONENT_AREA);
    if form.shows(&label) {
        let area = match facts.area_mm2 {
            Shared::Same(Some(mm2)) => t!(
                form.ctx.i18n, ids::PCB_COMPONENT_AREA_VALUE,
                "mm2" => format!("{mm2:.3}")
            )
            .into_string(),
            Shared::Same(None) | Shared::Mixed => String::new(),
        };
        rows.push(rows::row(form, label, rows::value(area)));
    }
    rows.extend(description(form, facts));
    // The board model has no field for these yet (#2354 follow-up).
    let not_stored: [MessageId; 7] = [
        ids::PCB_COMPONENT_TYPE,
        ids::PCB_COMPONENT_DESIGN_ITEM_ID,
        ids::PCB_COMPONENT_SOURCE,
        ids::PCB_COMPONENT_HEIGHT,
        ids::PCB_COMPONENT_BODY_OPACITY,
        ids::PCB_COMPONENT_LOCK_PRIMITIVES,
        ids::PCB_COMPONENT_LOCK_STRINGS,
    ];
    for label in not_stored {
        rows.extend(rows::unsupported(
            form,
            label,
            ids::PCB_COMPONENT_NOT_STORED,
        ));
    }
    rows
}

/// The Description row edits the components' Description parameter; without
/// one there is nothing to edit.
fn description<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Option<Element<'a, PanelMsg>> {
    let label = form.text(ids::PCB_COMPONENT_DESCRIPTION);
    if !form.shows(&label) {
        return None;
    }
    let Some(value) = facts.parameter(DESCRIPTION_KEY) else {
        return rows::unsupported(
            form,
            ids::PCB_COMPONENT_DESCRIPTION,
            ids::PCB_COMPONENT_NO_DESCRIPTION,
        );
    };
    let field = ComponentField::Parameter(DESCRIPTION_KEY.to_string());
    let control = draft_input(facts, field, value.same().cloned());
    Some(rows::row(form, label, control))
}

/// Top Layer and Bottom Layer as two tiles, each with its copper colour; the
/// tile the components sit on is lit, neither when they differ. Picking the
/// other side flips them.
fn layer_picker<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Element<'a, PanelMsg> {
    let tile = |side: BoardSide, token: &str, label: MessageId| {
        let swatch = parse_pcb_layer(token)
            .map(|layer| layer_style::layer_colour(layer, &form.ctx.tokens))
            .unwrap_or(Color::TRANSPARENT);
        let active = facts.side == Shared::Same(side);
        let content = Row::<Element<'a, PanelMsg>>::new()
            .push(chip(swatch))
            .push_boxed(text(t!(form.ctx.i18n, label)).size(TS.body))
            .spacing(SP.s)
            .align_y(iced::Alignment::Center);
        let mut tile = button(content)
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .style(form_look::option_tile(active));
        if facts.is_editable() && !active {
            tile = tile.on_press(PanelMsg::PcbComponent(PcbComponentMsg::SidePicked {
                board: facts.board.clone(),
                footprints: facts.footprints.clone(),
                side,
            }));
        }
        tile.boxed()
    };
    Row::new()
        .push(tile(
            BoardSide::Top,
            "TopCopper",
            ids::PCB_COMPONENT_LAYER_TOP,
        ))
        .push(tile(
            BoardSide::Bottom,
            "BottomCopper",
            ids::PCB_COMPONENT_LAYER_BOTTOM,
        ))
        .spacing(SP.xs)
        .boxed()
}

/// A small square of the layer's colour.
fn chip<'a>(colour: Color) -> Element<'a, PanelMsg> {
    container(Space::new())
        .width(TS.body)
        .height(TS.body)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(colour)),
            border: Border {
                width: 1.0,
                color: Color { a: 0.5, ..colour },
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
