//! Properties of a selected placed part.

use super::super::*;
use super::rows::{PanelStyle, info_row};
use crate::styles::{Spacing, TypeScale};
use iced::Widget;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

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
    let position = get("Position");
    let rotation = get("Rotation");
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
    let lbl = LabelInk::graphite(muted);
    if let Some(id) = uuid {
        // General section — editable
        col = col.push(
            ctx.collapsible(
                "sel_general",
                "General",
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(form_edit_row("Designator", &reference, lbl, move |s| {
                        PanelMsg::EditSymbolDesignator(id, s)
                    }));
                    c = c.push(form_edit_row("Value", &value, lbl, move |s| {
                        PanelMsg::EditSymbolValue(id, s)
                    }));
                    if !description.is_empty() {
                        c = c.push(form_input_row("Description", &description, lbl, input_bg));
                    }
                    c = super::library_rows::push_footprint_row(
                        c,
                        id,
                        &footprint,
                        &footprint_choices,
                        lbl,
                    );
                    if !datasheet.is_empty() {
                        c = c.push(form_input_row("Datasheet", &datasheet, lbl, input_bg));
                    }
                    for (label, value) in &documents {
                        c = c.push(form_input_row(label, value, lbl, input_bg));
                    }
                    c = c.push(super::library_rows::library_id_row(
                        &lib_id,
                        lbl,
                        input_bg,
                        &ctx.tokens,
                    ));
                    c = super::library_rows::push_symbol_choice_row(c, id, &symbol_choices, lbl);
                    c = super::library_rows::push_library_rows(
                        c,
                        id,
                        &library_name,
                        library_status,
                        lbl,
                        input_bg,
                        &ctx.tokens,
                    );
                    c = super::library_rows::push_part_actions(c);
                    super::body_row::push_body_row(c, id, &body, lbl, input_bg)
                },
            )
            .boxed(),
        );

        // Location section — read-only for now
        col = col.push(
            ctx.collapsible(
                "sel_location",
                "Location",
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(form_input_row("Position", &position, lbl, input_bg));
                    c = c.push(form_input_row("Rotation", &rotation, lbl, input_bg));
                    c
                },
            )
            .boxed(),
        );

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
                        lbl,
                        panel_bg,
                    ));
                    c = c.push(form_check_row(
                        "Mirror Y",
                        has_mirror_y,
                        PanelMsg::ToggleSymbolMirrorY(id),
                        lbl,
                        panel_bg,
                    ));
                    c = c.push(form_check_row(
                        "Locked",
                        locked,
                        PanelMsg::ToggleSymbolLocked(id),
                        lbl,
                        panel_bg,
                    ));
                    c = c.push(form_check_row(
                        "DNP",
                        dnp,
                        PanelMsg::ToggleSymbolDnp(id),
                        lbl,
                        panel_bg,
                    ));
                    c
                },
            )
            .boxed(),
        );

        // Parameters section — custom fields carried on the symbol
        // instance. Read-only here; per-field editing belongs to the
        // Parameter Manager dialog.
        let header_label = if params.is_empty() {
            "Parameters (none)".to_string()
        } else {
            format!("Parameters ({})", params.len())
        };
        let section_params = params.clone();
        col = col.push(
            ctx.collapsible(
                "sel_parameters",
                &header_label,
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    if section_params.is_empty() {
                        c = c.push(
                            container(text(raw("No custom parameters")).size(TS.body).color(muted))
                                .padding([6.0, SP.m])
                                .boxed(),
                        );
                    } else {
                        for (name, value) in &section_params {
                            c = c.push(form_input_row(name, value, lbl, input_bg));
                        }
                    }
                    c
                },
            )
            .boxed(),
        );

        col = super::footprint_section::push_footprint_section(col, ctx);
        col = super::pins::push_pins_section(col, ctx, muted, primary, border_c);
    }
    col
}
