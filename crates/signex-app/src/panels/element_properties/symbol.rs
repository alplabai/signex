//! Properties of a selected placed part.

use super::super::*;
use super::rows::{PanelStyle, info_row};
use iced::Widget;
use signex_i18n::{ids, t};

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
    let PanelPalette {
        input_bg, panel_bg, ..
    } = palette;
    let get = |key: &str| info_row(&ctx.selection_info, key);
    let reference = get("Reference");
    let value = get("Value");
    // What an `=` value evaluates to, or why not (#1986).
    let value_shows = get("Value Shows");
    let value_error = get("Value Error");
    let description = get("Description");
    let datasheet = get("Datasheet");
    let footprint = get("Footprint");
    let lib_id = get("Library ID");
    let library_name = get("Library");
    let symbol_choices = ctx.symbol_choices.clone();
    let footprint_choices = ctx.footprint_choices.clone();
    let library_drifted = get("Library Drift") == "Yes";
    let library_status = super::library_rows::LibraryStatus {
        drifted: library_drifted,
        older_instances: get("Library Older Version").parse().unwrap_or(0),
        this_one_current: get("Library This One Current") == "Yes",
        i18n: &ctx.i18n,
        missing: get("Library Status") == "Symbol missing",
    };
    let body = get("Body Style");
    let locked = get("Locked") == "Yes";
    let dnp = get("DNP") == "Yes";
    let has_mirror_x = ctx
        .selection_info
        .iter()
        .any(|(k, v)| k == "Mirror" && v == "X");
    let has_mirror_y = ctx
        .selection_info
        .iter()
        .any(|(k, v)| k == "Mirror" && v == "Y");
    // The library row's documents (#1231): every
    // ("Document: LABEL", value) tuple, the primary first.
    let documents: Vec<(String, String)> = ctx
        .selection_info
        .iter()
        .filter_map(|(k, v)| {
            k.strip_prefix(crate::library::row_documents::DOCUMENT_KEY)
                .map(|label| (label.to_string(), v.clone()))
        })
        .collect();
    // Custom parameters: every ("Param: NAME", value) tuple.
    let params: Vec<(String, String)> = ctx
        .selection_info
        .iter()
        .filter_map(|(k, v)| {
            k.strip_prefix("Param: ")
                .map(|name| (name.to_string(), v.clone()))
        })
        .collect();

    col = super::pins::push_focused_pin_section(col, ctx, muted);
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    // Fixed label column on Graphite, so no label is clipped.
    if let Some(id) = uuid {
        // General section — editable
        col = col.push(
            ctx.collapsible(
                "sel_general",
                ids::PROPERTIES_SECTION_GENERAL,
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(form_edit_row(
                        t!(i18n, ids::PROPERTIES_DESIGNATOR),
                        &reference,
                        muted,
                        move |s| PanelMsg::EditSymbolDesignator(id, s),
                    ));
                    c = c.push(form_edit_row(
                        t!(i18n, ids::PROPERTIES_VALUE),
                        &value,
                        muted,
                        move |s| PanelMsg::EditSymbolValue(id, s),
                    ));
                    if let Some(note) = value_note(ctx, &value_shows, &value_error, muted) {
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
                        &footprint,
                        &footprint_choices,
                        muted,
                        i18n,
                    );
                    if !datasheet.is_empty() {
                        c = c.push(form_input_row(
                            t!(i18n, ids::PROPERTIES_DATASHEET),
                            &datasheet,
                            muted,
                            input_bg,
                        ));
                    }
                    for (label, value) in &documents {
                        // raw: the document's label is written in the library row.
                        c = c.push(form_input_row(raw(label.clone()), value, muted, input_bg));
                    }
                    c = c.push(super::library_rows::library_id_row(
                        &lib_id,
                        muted,
                        input_bg,
                        &ctx.tokens,
                        i18n,
                    ));
                    c = super::library_rows::push_symbol_choice_row(
                        c,
                        id,
                        &symbol_choices,
                        muted,
                        i18n,
                    );
                    c = super::library_rows::push_library_rows(
                        c,
                        id,
                        &library_name,
                        library_status,
                        muted,
                        input_bg,
                        &ctx.tokens,
                    );
                    c = super::library_rows::push_part_actions(c, i18n);
                    super::body_row::push_body_row(c, id, &body, muted, input_bg, i18n)
                },
            )
            .boxed(),
        );

        // Location section: Position and Rotation, one undo step per edit.
        let look = super::placed_rows::RowLook { muted, input_bg };
        col = super::placed_rows::push_location_section(col, ctx, look);

        // Graphical section — checkboxes
        col = col.push(
            ctx.collapsible(
                "sel_graphical",
                ids::PROPERTIES_SECTION_GRAPHICAL,
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(form_check_row(
                        t!(i18n, ids::PROPERTIES_MIRROR_X),
                        has_mirror_x,
                        PanelMsg::ToggleSymbolMirrorX(id),
                        muted,
                        panel_bg,
                    ));
                    c = c.push(form_check_row(
                        t!(i18n, ids::PROPERTIES_MIRROR_Y),
                        has_mirror_y,
                        PanelMsg::ToggleSymbolMirrorY(id),
                        muted,
                        panel_bg,
                    ));
                    c = c.push(form_check_row(
                        t!(i18n, ids::PROPERTIES_LOCKED),
                        locked,
                        PanelMsg::ToggleSymbolLocked(id),
                        muted,
                        panel_bg,
                    ));
                    c = c.push(form_check_row(
                        t!(i18n, ids::PROPERTIES_DNP),
                        dnp,
                        PanelMsg::ToggleSymbolDnp(id),
                        muted,
                        panel_bg,
                    ));
                    c
                },
            )
            .boxed(),
        );

        // Parameters section: the custom fields of the instance, each edited
        // in place and committed on Enter.
        col = super::placed_rows::push_parameters_section(col, ctx, params, look);

        col = super::footprint_section::push_footprint_section(col, ctx);
        col = super::pins::push_pins_section(col, ctx, muted, primary, border_c);
    }
    col
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
    let caption = crate::styles::TypeScale::DEFAULT.caption;
    Some(
        container(text(note).size(caption).color(color))
            .padding([0, PROPERTY_ROW_PAD_X])
            .boxed(),
    )
}
