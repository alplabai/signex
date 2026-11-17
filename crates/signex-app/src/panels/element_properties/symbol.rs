//! Properties of a selected placed part.

use super::super::*;
use super::rows::{PanelStyle, info_row};
use iced::Widget;

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
    // Fixed label column on Graphite, so no label is clipped.
    if let Some(id) = uuid {
        // General section — editable
        col = col.push(
            ctx.collapsible(
                "sel_general",
                "General",
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(form_edit_row("Designator", &reference, muted, move |s| {
                        PanelMsg::EditSymbolDesignator(id, s)
                    }));
                    c = c.push(form_edit_row("Value", &value, muted, move |s| {
                        PanelMsg::EditSymbolValue(id, s)
                    }));
                    if !description.is_empty() {
                        c = c.push(form_input_row("Description", &description, muted, input_bg));
                    }
                    c = super::library_rows::push_footprint_row(
                        c,
                        id,
                        &footprint,
                        &footprint_choices,
                        muted,
                    );
                    if !datasheet.is_empty() {
                        c = c.push(form_input_row("Datasheet", &datasheet, muted, input_bg));
                    }
                    for (label, value) in &documents {
                        c = c.push(form_input_row(label, value, muted, input_bg));
                    }
                    c = c.push(super::library_rows::library_id_row(
                        &lib_id,
                        muted,
                        input_bg,
                        &ctx.tokens,
                    ));
                    c = super::library_rows::push_symbol_choice_row(c, id, &symbol_choices, muted);
                    c = super::library_rows::push_library_rows(
                        c,
                        id,
                        &library_name,
                        library_status,
                        muted,
                        input_bg,
                        &ctx.tokens,
                    );
                    c = super::library_rows::push_part_actions(c);
                    super::body_row::push_body_row(c, id, &body, muted, input_bg)
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
                "Graphical",
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(form_check_row(
                        "Mirror X",
                        has_mirror_x,
                        PanelMsg::ToggleSymbolMirrorX(id),
                        muted,
                        panel_bg,
                    ));
                    c = c.push(form_check_row(
                        "Mirror Y",
                        has_mirror_y,
                        PanelMsg::ToggleSymbolMirrorY(id),
                        muted,
                        panel_bg,
                    ));
                    c = c.push(form_check_row(
                        "Locked",
                        locked,
                        PanelMsg::ToggleSymbolLocked(id),
                        muted,
                        panel_bg,
                    ));
                    c = c.push(form_check_row(
                        "DNP",
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
