//! Properties of a selected placed part (mockup row 42): the ⋯ menu, the clicked
//! pin's section, the library banner, the "Part on the sheet" drawing, then
//! General, Location, Graphical, Parameters, Footprint and Pins.

use super::super::*;
use super::illustrations::part::{
    DESIGNATOR, DNP, LOCKED, MIRROR_X, MIRROR_Y, PartLabels, PartSketch, VALUE, part_illustration,
};
use super::part_panel::{illustration_box, link_row, linked_row};
use super::rows::{PanelStyle, info_row};
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{ids, t};
use signex_types::measure::length_field_text;
use signex_widgets::CheckboxLabel;
use signex_widgets::property_illustration::FieldId;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Height of the "Part on the sheet" drawing, in caption sizes.
const PART_ILLUSTRATION_EM: f32 = 13.0;

pub(super) fn push_symbol_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    uuid: Option<uuid::Uuid>,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let PanelStyle {
        muted,
        primary,
        border_c,
        palette,
    } = st;
    let Some(id) = uuid else {
        return super::pins::push_focused_pin_section(col, ctx, muted);
    };
    let get = |key: &str| info_row(&ctx.selection_info, key);
    let lib_id = get("Library ID");
    let library_status = super::library_rows::LibraryStatus {
        drifted: get("Library Drift") == "Yes",
        older_instances: get("Library Older Version").parse().unwrap_or(0),
        this_one_current: get("Library This One Current") == "Yes",
        i18n: &ctx.i18n,
        missing: get("Library Status") == "Symbol missing",
    };
    let is_pin_focused = ctx
        .selection_info
        .iter()
        .any(|(k, _)| k == signex_engine::info_keys::FOCUSED_PIN);

    col = super::part_panel::push_part_menu(col, ctx, id, &lib_id);
    col = super::pins::push_focused_pin_section(col, ctx, muted);
    if let Some(banner) = super::library_rows::library_banner(id, library_status, &ctx.tokens) {
        col = col.push(banner);
    }
    // The pin's own drawing stands in for the part's while a pin is focused.
    if !is_pin_focused {
        col = col.push(part_drawing(ctx));
    }
    col = col.push(
        ctx.collapsible(
            "sel_general",
            ids::PROPERTIES_SECTION_GENERAL,
            &ctx.collapsed_sections,
            move || general_rows(ctx, id, muted, palette.input_bg),
        )
        .boxed(),
    );

    // Location section: Position and Rotation, one undo step per edit.
    let look = super::placed_rows::RowLook {
        muted,
        input_bg: palette.input_bg,
    };
    col = super::placed_rows::push_location_section(col, ctx, look);

    col = col.push(
        ctx.collapsible(
            "sel_graphical",
            ids::PROPERTIES_SECTION_GRAPHICAL,
            &ctx.collapsed_sections,
            move || graphical_grid(ctx, id, primary),
        )
        .boxed(),
    );

    // Parameters section: the custom fields of the instance, each edited
    // in place and committed on Enter.
    let params: Vec<(String, String)> = ctx
        .selection_info
        .iter()
        .filter_map(|(k, v)| {
            k.strip_prefix("Param: ")
                .map(|name| (name.to_string(), v.clone()))
        })
        .collect();
    col = super::placed_rows::push_parameters_section(col, ctx, params, look);

    col = super::footprint_section::push_footprint_section(col, ctx);
    // Mockup row 197: the notebook blocks about the part.
    let designator = info_row(&ctx.selection_info, "Reference");
    col = super::design_notes::push_design_notes_section(col, ctx, &designator);
    super::pins::push_pins_section(col, ctx, primary, border_c)
}

/// "Part on the sheet": the part's drawing with its field callouts.
fn part_drawing<'a>(ctx: &PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let sketch = PartSketch::from_info(&ctx.selection_info);
    let mirror = match (sketch.mirror_x, sketch.mirror_y) {
        (true, true) => Some(ids::PROPERTIES_PART_ILLUSTRATION_MIRROR_XY),
        (true, false) => Some(ids::PROPERTIES_MIRROR_X),
        (false, true) => Some(ids::PROPERTIES_MIRROR_Y),
        (false, false) => None,
    };
    let position = ctx.placed.geometry.map(|geometry| {
        let at = geometry.position;
        let x = length_field_text(at.x, ctx.unit);
        let y = length_field_text(ctx.shown_y(at.y), ctx.unit);
        t!(i18n, ids::PROPERTIES_PART_ILLUSTRATION_POSITION, "x" => x, "y" => y).into()
    });
    let labels = PartLabels {
        locked: t!(i18n, ids::PROPERTIES_LOCKED).into(),
        dnp: t!(i18n, ids::PROPERTIES_DNP).into(),
        mirror: mirror.map(|id| t!(i18n, id).into()),
        position,
    };
    illustration_box(
        part_illustration(&sketch, labels).height_em(PART_ILLUSTRATION_EM),
        ctx,
    )
}

/// The General rows, in the order people edit them: Designator, Value,
/// Description, Footprint and its override, Datasheet, Library, Symbol,
/// Body, then the part actions.
fn general_rows<'a>(
    ctx: &'a PanelContext,
    id: uuid::Uuid,
    muted: Color,
    input_bg: Color,
) -> Column<Element<'a, PanelMsg>> {
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let get = |key: &str| info_row(&ctx.selection_info, key);
    let reference = get("Reference");
    let value = get("Value");
    let description = get("Description");
    let datasheet = get("Datasheet");
    // The library row's documents (#1231): every ("Document: LABEL", value)
    // tuple, the primary first.
    let documents = ctx.selection_info.iter().filter_map(|(k, v)| {
        k.strip_prefix(crate::library::row_documents::DOCUMENT_KEY)
            .map(|label| (label.to_string(), v.clone()))
    });

    let mut c = Column::new().spacing(0).width(Length::Fill);
    c = c.push(linked_row(
        form_edit_row(
            t!(i18n, ids::PROPERTIES_DESIGNATOR),
            &reference,
            muted,
            move |s| PanelMsg::EditSymbolDesignator(id, s),
        ),
        DESIGNATOR,
        ctx,
    ));
    c = c.push(linked_row(
        form_edit_row(t!(i18n, ids::PROPERTIES_VALUE), &value, muted, move |s| {
            PanelMsg::EditSymbolValue(id, s)
        }),
        VALUE,
        ctx,
    ));
    // What an `=` value evaluates to, or why not (#1986).
    if let Some(note) = value_note(ctx, &get("Value Shows"), &get("Value Error"), muted) {
        c = c.push(note);
    }
    if !description.is_empty() {
        c = c.push(form_input_row(
            t!(i18n, ids::PROPERTIES_DESCRIPTION),
            &description,
            muted,
            input_bg,
        ));
    }
    c = super::library_rows::push_footprint_row(
        c,
        id,
        &get("Footprint"),
        &ctx.footprint_choices,
        muted,
        i18n,
    );
    if !datasheet.is_empty() {
        c = c.push(link_row(
            t!(i18n, ids::PROPERTIES_DATASHEET),
            &datasheet,
            PanelMsg::PlacedPartAction(crate::library::PlacedPartAction::OpenDatasheet),
            muted,
            &ctx.tokens,
        ));
    }
    for (label, value) in documents {
        // raw: the document's label is written in the library row.
        c = c.push(form_input_row(raw(label), &value, muted, input_bg));
    }
    c = c.push(super::library_rows::library_and_id_row(
        &get("Library"),
        &get("Library ID"),
        muted,
        input_bg,
        &ctx.tokens,
        i18n,
    ));
    c = super::library_rows::push_symbol_choice_row(c, id, &ctx.symbol_choices, muted, i18n);
    c = super::body_row::push_body_row(c, id, &get("Body Style"), i18n);
    super::library_rows::push_part_actions(c, i18n)
}

/// Mirror X, Mirror Y, Locked and DNP as a 2x2 grid of checkboxes, each
/// linked to its mark in the drawing.
fn graphical_grid<'a>(
    ctx: &'a PanelContext,
    id: uuid::Uuid,
    ink: Color,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = &ctx.i18n;
    let mirrored = |axis: &str| {
        ctx.selection_info
            .iter()
            .any(|(k, v)| k == "Mirror" && v == axis)
    };
    let yes = |key: &str| info_row(&ctx.selection_info, key) == "Yes";
    let cell = |label, checked: bool, msg: PanelMsg, field: FieldId| -> Element<'a, PanelMsg> {
        let check = signex_widgets::checkbox(checked)
            .size(TS.heading)
            .labelled(t!(i18n, label))
            .text_size(TS.body)
            .spacing(SP.s)
            .style(super::super::form_look::checkbox_style)
            .on_toggle(move |_| msg.clone());
        let check = container(check)
            .padding([SP.xs, 0.0])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                text_color: Some(ink),
                ..container::Style::default()
            });
        linked_row(check.boxed(), field, ctx)
    };
    let pair = |a: Element<'a, PanelMsg>, b: Element<'a, PanelMsg>| -> Element<'a, PanelMsg> {
        iced::widget::row![
            container(a).width(Length::FillPortion(1)),
            container(b).width(Length::FillPortion(1)),
        ]
        .spacing(SP.m)
        .width(Length::Fill)
        .boxed()
    };
    let mirror_x = cell(
        ids::PROPERTIES_MIRROR_X,
        mirrored("X"),
        PanelMsg::ToggleSymbolMirrorX(id),
        MIRROR_X,
    );
    let mirror_y = cell(
        ids::PROPERTIES_MIRROR_Y,
        mirrored("Y"),
        PanelMsg::ToggleSymbolMirrorY(id),
        MIRROR_Y,
    );
    let locked = cell(
        ids::PROPERTIES_LOCKED,
        yes("Locked"),
        PanelMsg::ToggleSymbolLocked(id),
        LOCKED,
    );
    // DNP stays a per-part flag until variants carry fitted state.
    let dnp = cell(
        ids::PROPERTIES_DNP,
        yes("DNP"),
        PanelMsg::ToggleSymbolDnp(id),
        DNP,
    );
    Column::new()
        .spacing(0)
        .width(Length::Fill)
        .padding(iced::Padding {
            top: 0.0,
            right: SP.m,
            bottom: SP.s,
            left: SP.m,
        })
        .push(pair(mirror_x, mirror_y))
        .push(pair(locked, dnp))
}

/// The caption under an `=` value (#1986): the field shows its source, the
/// caption what it evaluates to or why it does not. `None` for a literal.
fn value_note<'a>(
    ctx: &'a PanelContext,
    shows: &str,
    error: &str,
    muted: Color,
) -> Option<Element<'a, PanelMsg>> {
    let i18n = &ctx.i18n;
    let (note, color) = if !error.is_empty() {
        let error = error.to_string();
        let color = crate::styles::ti(ctx.tokens.error);
        (
            t!(i18n, ids::PROPERTIES_VALUE_ERROR, "error" => error),
            color,
        )
    } else if !shows.is_empty() {
        let value = shows.to_string();
        (
            t!(i18n, ids::PROPERTIES_VALUE_SHOWS, "value" => value),
            muted,
        )
    } else {
        return None;
    };
    Some(
        container(text(note).size(TS.caption).color(color))
            .padding([0, PROPERTY_ROW_PAD_X])
            .boxed(),
    )
}
