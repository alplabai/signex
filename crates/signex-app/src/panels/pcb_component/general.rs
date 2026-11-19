//! The Location and Properties sections of the General tab (Graphite m94):
//! Location carries the lock in its header, then Position (X and Y on one
//! row) and Rotation; Properties holds the side as a Top / Bottom segmented
//! control, Designator, Comment, and the read-only Description and Area.

use iced::widget::{Column, Row, Space, container, svg};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_engine::pcb::component::LocationField;
use signex_engine::pcb::designators::FootprintTextField;
use signex_i18n::{Localized, MessageId, ids, t};
use signex_types::pcb_layer::{BoardSide, CopperLayer, PcbLayer};
use signex_widgets::{button, text, text_input};

use super::rows::{self, Form, InputEdit, SectionSpec};
use super::{
    ComponentField, LAYER_FIELD, PcbComponentFacts, PcbComponentMsg, PcbFootprintFacts, Shared,
};
use crate::panels::{PanelContext, PanelMsg, form_look};
use crate::styles::board_palette::BoardPalette;
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The key of the parameter the Description row shows.
const DESCRIPTION_KEY: &str = "Description";

/// The lock glyph of the Location header (the mockup's `i-lock`).
const LOCK_ICON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><rect x="3.5" y="7" width="9" height="6.5" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.3"/><path d="M5.5 7V5a2.5 2.5 0 015 0v2" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>"#;

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
        location_section(&form, facts),
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

/// Location, its header carrying the lock.
fn location_section<'a>(
    form: &Form<'a>,
    facts: &'a PcbComponentFacts,
) -> Option<Element<'a, PanelMsg>> {
    let lock_shown = form.shows(&form.text(ids::PCB_COMPONENT_LOCKED));
    let spec = SectionSpec {
        trailing: lock_shown.then(|| lock_control(form, facts)),
        keep_empty: lock_shown,
        ..SectionSpec::new(
            "pcb_component_location",
            ids::PCB_COMPONENT_SECTION_LOCATION,
        )
    };
    rows::section_with(form, spec, location(form, facts))
}

/// The lock glyph, "Locked" and its checkbox, with the tooltip that says
/// what a lock stops.
fn lock_control<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Element<'a, PanelMsg> {
    let ink = resolve(&form.ctx.tokens).ink4;
    let toggled = (!facts.read_only).then(|| {
        PanelMsg::PcbComponent(PcbComponentMsg::LockToggled {
            board: facts.board.clone(),
            footprints: facts.footprints.clone(),
            locked: facts.locked != Shared::Same(true),
        })
    });
    let icon = svg(svg::Handle::from_memory(LOCK_ICON))
        .width(TS.caption)
        .height(TS.caption)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    let control = Row::new()
        .push_boxed(icon)
        .push_boxed(
            text(t!(form.ctx.i18n, ids::PCB_COMPONENT_LOCKED))
                .size(TS.caption)
                .color(ink),
        )
        .push(rows::check(&facts.locked, toggled))
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center)
        .boxed();
    form.with_tip(control, ids::PCB_COMPONENT_LOCKED_TIP)
}

/// Position (X and Y with their prefixes and one unit) and Rotation.
fn location<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Vec<Element<'a, PanelMsg>> {
    let mut rows = Vec::new();
    let label = form.text(ids::PCB_COMPONENT_POSITION);
    if form.shows(&label) {
        let coordinate = |field: LocationField, prefix: MessageId, value: &Shared<f64>| {
            let field = ComponentField::Location(field);
            let shown = shown_value(facts, &field, value.same().map(|v| rows::number_text(*v)));
            prefixed(
                form,
                t!(form.ctx.i18n, prefix),
                shown,
                draft_edit(facts, field),
            )
        };
        let control = Row::new()
            .push(coordinate(
                LocationField::X,
                ids::PCB_COMPONENT_X_PREFIX,
                &facts.x,
            ))
            .push(coordinate(
                LocationField::Y,
                ids::PCB_COMPONENT_Y_PREFIX,
                &facts.y,
            ))
            .push(rows::unit(form, ids::PCB_COMPONENT_UNIT_MM))
            .spacing(SP.xs)
            .align_y(iced::Alignment::Center)
            .boxed();
        rows.push(rows::row(form, label, control));
    }
    let label = form.text(ids::PCB_COMPONENT_ROTATION);
    if form.shows(&label) {
        let field = ComponentField::Location(LocationField::Rotation);
        let stored = facts.rotation.same().map(|v| rows::number_text(*v));
        let mixed =
            (facts.rotation == Shared::Mixed).then(|| t!(form.ctx.i18n, ids::PCB_COMPONENT_MIXED));
        let control = draft_input_with(facts, field, stored, mixed);
        rows.push(rows::row(
            form,
            label,
            rows::with_unit(form, control, ids::PCB_COMPONENT_UNIT_DEGREES),
        ));
    }
    rows
}

/// A bare input after its catalog prefix ("X", "Y"), in one field frame.
fn prefixed<'a>(
    form: &Form<'a>,
    prefix: Localized,
    shown: String,
    edit: Option<InputEdit<'a>>,
) -> Element<'a, PanelMsg> {
    let mut input = text_input(signex_i18n::UserText::new(String::new()), shown)
        .size(TS.body)
        .padding([SP.s, SP.xs])
        .style(rows::bare_input_style)
        .width(Length::Fill);
    if let Some((on_input, on_submit)) = edit {
        input = input.on_input(on_input).on_submit(on_submit);
    }
    let content = Row::new()
        .push_boxed(
            text(prefix)
                .size(TS.caption)
                .color(resolve(&form.ctx.tokens).ink4),
        )
        .push_boxed(input)
        .spacing(SP.xs)
        .align_y(iced::Alignment::Center)
        .boxed();
    rows::framed(form, content)
}

/// The text a field's input shows: the user's unsubmitted draft, else the
/// stored value, else empty.
pub(super) fn shown_value(
    facts: &PcbComponentFacts,
    field: &ComponentField,
    stored: Option<String>,
) -> String {
    facts.draft(field).cloned().or(stored).unwrap_or_default()
}

/// What typing into `field` sends while placement edits run: the draft per
/// keystroke, the edit on Enter. `None` while they do not.
pub(super) fn draft_edit<'a>(
    facts: &'a PcbComponentFacts,
    field: ComponentField,
) -> Option<InputEdit<'a>> {
    if !facts.is_editable() {
        return None;
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
    Some((Box::new(typed), submitted))
}

/// An input whose text is `field`'s draft, else `stored` (blank when the
/// components differ); editable while placement edits run.
pub(super) fn draft_input<'a>(
    facts: &'a PcbComponentFacts,
    field: ComponentField,
    stored: Option<String>,
) -> Element<'a, PanelMsg> {
    draft_input_with(facts, field, stored, None)
}

/// [`draft_input`] showing `mixed` in italics while it is empty.
pub(super) fn draft_input_with<'a>(
    facts: &'a PcbComponentFacts,
    field: ComponentField,
    stored: Option<String>,
    mixed: Option<Localized>,
) -> Element<'a, PanelMsg> {
    let shown = shown_value(facts, &field, stored);
    rows::input_with(shown, mixed, draft_edit(facts, field))
}

fn properties<'a>(
    form: &Form<'a>,
    facts: &'a PcbComponentFacts,
    single: Option<&'a PcbFootprintFacts>,
) -> Vec<Element<'a, PanelMsg>> {
    let mut rows = Vec::new();
    let label = form.text(ids::PCB_COMPONENT_LAYER);
    if form.shows(&label) {
        rows.push(rows::linked_row(
            form,
            label,
            LAYER_FIELD,
            layer_picker(form, facts),
        ));
    }
    for field in [FootprintTextField::Designator, FootprintTextField::Value] {
        rows.extend(super::text::text_row(form, facts, single, field));
    }
    rows.extend(super::text::comment_note(form, facts, single));
    let label = form.text(ids::PCB_COMPONENT_DESCRIPTION);
    if form.shows(&label) {
        rows.push(rows::row(
            form,
            label,
            rows::read_only(form, description(facts)),
        ));
    }
    let label = form.text(ids::PCB_COMPONENT_AREA);
    if form.shows(&label) {
        let area = match facts.area_mm2 {
            Shared::Same(Some(mm2)) => t!(
                form.ctx.i18n, ids::PCB_COMPONENT_AREA_VALUE,
                "mm2" => format!("{mm2:.2}")
            )
            .into_string(),
            Shared::Same(None) | Shared::Mixed => String::new(),
        };
        rows.push(rows::row(form, label, rows::read_only(form, area)));
    }
    rows
}

/// The Description the components share: their Description parameter (its
/// edit is on the Parameters tab), else the footprint's own description;
/// blank when they differ.
fn description(facts: &PcbComponentFacts) -> String {
    let shared = match facts.parameter(DESCRIPTION_KEY) {
        Some(parameter) => parameter,
        None => &facts.settings.description,
    };
    shared.same().cloned().unwrap_or_default()
}

/// Top Layer and Bottom Layer as a segmented control; the side the
/// components sit on is raised with its copper colour, neither when they
/// differ. Picking the other side flips them.
fn layer_picker<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Element<'a, PanelMsg> {
    let palette = BoardPalette::new();
    let cell = |side: BoardSide, copper: CopperLayer, label: MessageId| {
        let active = facts.side == Shared::Same(side);
        let mut content = Row::<Element<'a, PanelMsg>>::new()
            .spacing(SP.xs)
            .align_y(iced::Alignment::Center);
        if active {
            content = content.push(dot(palette.layer(PcbLayer::Copper(copper))));
        }
        content = content.push_boxed(text(t!(form.ctx.i18n, label)).size(TS.caption));
        let picked = (facts.is_editable() && !active).then(|| {
            PanelMsg::PcbComponent(PcbComponentMsg::SidePicked {
                board: facts.board.clone(),
                footprints: facts.footprints.clone(),
                side,
            })
        });
        button(container(content).center_x(Length::Fill))
            .padding([SP.xs, SP.s])
            .width(Length::Fill)
            .on_press_maybe(picked)
            .style(move |theme: &Theme, status| {
                let hovered = matches!(status, iced::widget::button::Status::Hovered);
                form_look::segment_cell(theme, active, hovered && !active)
            })
            .boxed()
    };
    container(
        Row::new()
            .push(cell(
                BoardSide::Top,
                CopperLayer::Top,
                ids::PCB_COMPONENT_LAYER_TOP,
            ))
            .push(cell(
                BoardSide::Bottom,
                CopperLayer::Bottom,
                ids::PCB_COMPONENT_LAYER_BOTTOM,
            ))
            .spacing(SP.xs)
            .width(Length::Fill),
    )
    .padding(SP.xs)
    .width(Length::Fill)
    .style(form_look::segmented_frame)
    .boxed()
}

/// A small round swatch of a layer's colour.
fn dot<'a>(colour: iced::Color) -> Element<'a, PanelMsg> {
    container(Space::new())
        .width(SP.m)
        .height(SP.m)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(colour)),
            border: Border {
                radius: SP.m.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
