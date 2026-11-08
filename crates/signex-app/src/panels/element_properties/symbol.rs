//! Properties of a selected placed symbol.

use super::super::*;
use super::rows::{PanelStyle, info_row};
use crate::styles::{Spacing, TypeScale};
use iced::Widget;

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
        input_bg,
        input_bdr,
        panel_bg,
        ..
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

    col = super::pins::push_focused_pin_section(col, ctx, muted, border_c);
    if let Some(id) = uuid {
        // General section — editable
        col = col.push(
            collapsible_section(
                "sel_general",
                "General",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(form_edit_row("Designator", &reference, muted, move |s| {
                        PanelMsg::EditSymbolDesignator(id, s)
                    }));
                    c = c.push(form_edit_row("Value", &value, muted, move |s| {
                        PanelMsg::EditSymbolValue(id, s)
                    }));
                    if !description.is_empty() {
                        c = c.push(form_input_row(
                            "Description",
                            &description,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                    }
                    c = super::library_rows::push_footprint_row(
                        c,
                        id,
                        &footprint,
                        &footprint_choices,
                        muted,
                    );
                    if !datasheet.is_empty() {
                        c = c.push(form_input_row(
                            "Datasheet",
                            &datasheet,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                    }
                    for (label, value) in &documents {
                        c = c.push(form_input_row(label, value, muted, input_bg, input_bdr));
                    }
                    c = c.push(form_input_row(
                        "Library ID",
                        &lib_id,
                        muted,
                        input_bg,
                        input_bdr,
                    ));
                    c = super::library_rows::push_symbol_choice_row(c, id, &symbol_choices, muted);
                    c = super::library_rows::push_library_rows(
                        c,
                        id,
                        &library_name,
                        library_status,
                        muted,
                        input_bg,
                        input_bdr,
                    );
                    c = super::library_rows::push_part_actions(c);
                    super::body_row::push_body_row(c, id, &body, muted, input_bg, input_bdr)
                },
            )
            .boxed(),
        );

        // Location section — read-only for now
        col = col.push(
            collapsible_section(
                "sel_location",
                "Location",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(form_input_row(
                        "Position", &position, muted, input_bg, input_bdr,
                    ));
                    c = c.push(form_input_row(
                        "Rotation", &rotation, muted, input_bg, input_bdr,
                    ));
                    c
                },
            )
            .boxed(),
        );

        // Graphical section — checkboxes
        col = col.push(
            collapsible_section(
                "sel_graphical",
                "Graphical",
                &ctx.collapsed_sections,
                muted,
                border_c,
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
            collapsible_section(
                "sel_parameters",
                &header_label,
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    if section_params.is_empty() {
                        c = c.push(
                            container(
                                text(raw("No custom parameters".to_string()))
                                    .size(TS.body)
                                    .color(muted),
                            )
                            .padding([6.0, SP.m])
                            .boxed(),
                        );
                    } else {
                        for (name, value) in &section_params {
                            c = c.push(form_input_row(name, value, muted, input_bg, input_bdr));
                        }
                    }
                    c
                },
            )
            .boxed(),
        );

        col = super::pins::push_pins_section(col, ctx, muted, primary, border_c);
    }
    col
}
