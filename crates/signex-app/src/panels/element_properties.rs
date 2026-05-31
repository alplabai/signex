//! Properties panel for a single selected schematic element (HI-22 / MD-20).
//!
//! Extracted from `panels/mod.rs`. Pure view code — zero behaviour change.
//! Routes between Symbol / Label / TextNote / Drawing / ChildSheet
//! contexts; the per-shape Drawing surface and per-child-sheet style
//! editor are nested view fns kept in the same module.

use iced::mouse;
use iced::widget::{
    Column, Space, button, canvas, column, container, pick_list, row, scrollable, svg, text,
    text_input,
};
use iced::{Background, Border, Color, Element, Length, Point, Rectangle, Renderer, Theme};

use super::{
    DrawingFieldId, PROPERTY_ROW_PAD_X, PanelContext, PanelMsg, collapsible_section,
    empty_section_row, font_style_row, form_check_row, form_edit_row, form_input_row, form_label,
    form_pick_row, justification_grid, net_numeric_row, net_params_add_bar, net_params_header,
    net_params_tabs, prop_kv_row, shape_icon_handle, thin_sep,
};

pub(super) fn view_selected_element_properties<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let input_bg = crate::styles::ti(ctx.tokens.selection);
    let input_bdr = crate::styles::ti(ctx.tokens.accent);
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    let elem_type = ctx
        .selection_info
        .iter()
        .find(|(k, _)| k == "Type")
        .map(|(_, v)| v.as_str())
        .unwrap_or("Object");

    let uuid = ctx.selected_uuid;
    let selected_kind = ctx.selected_kind;
    let get = |key: &str| -> String {
        ctx.selection_info
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };

    // ── Header ──
    col = col.push(
        container(text(elem_type.to_owned()).size(11).color(primary))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(thin_sep(border_c));

    // ── Editable properties based on element type ──
    // Power ports get their own Altium-style panel; regular symbols keep the
    // existing designator/value/footprint layout.
    let is_power_port = elem_type == "Power Port";

    if is_power_port && let Some(id) = uuid {
        let value = get("Value");
        let position = get("Position");
        let rotation_str = get("Rotation");
        let lib_id = get("Library ID");
        let rotation_deg = rotation_str
            .trim_end_matches('°')
            .trim()
            .parse::<f64>()
            .unwrap_or(0.0);
        // Pick a Style token by looking inside the lib_id (same logic the
        // built-in power renderer uses).
        let lid = lib_id.to_lowercase();
        let current_style =
            if lid.contains("gnd") && !lid.contains("earth") && !lid.contains("gndref") {
                "Power Ground"
            } else if lid.contains("gndref") {
                "Signal Ground"
            } else if lid.contains("earth") {
                "Earth"
            } else if lid.contains("arrow") {
                "Arrow"
            } else if lid.contains("wave") {
                "Wave"
            } else if lid.contains("circle") {
                "Circle"
            } else {
                "Bar"
            };
        let style_options: Vec<String> = [
            "Bar",
            "Arrow",
            "Wave",
            "Circle",
            "Power Ground",
            "Signal Ground",
            "Earth",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();

        // ── Location ──
        let pos_loc = position.clone();
        let rot_current = rotation_deg;
        col = col.push(collapsible_section(
            "sel_location",
            "Location",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_input_row(
                    "(X/Y)", &pos_loc, muted, input_bg, input_bdr,
                ));
                let rotation_opts: Vec<String> = vec![
                    "0 Degrees".into(),
                    "90 Degrees".into(),
                    "180 Degrees".into(),
                    "270 Degrees".into(),
                ];
                let rot_label = format!("{:.0} Degrees", rot_current);
                c = c.push(form_pick_row(
                    "Rotation",
                    rotation_opts,
                    rot_label,
                    move |s| {
                        let deg = s
                            .split_whitespace()
                            .next()
                            .and_then(|n| n.parse::<f64>().ok())
                            .unwrap_or(0.0);
                        PanelMsg::EditSymbolRotation(id, deg)
                    },
                    muted,
                ));
                c
            },
        ));

        // ── Properties (Name, Style) ──
        let name_val = value.clone();
        let base_lib = lib_id.clone();
        col = col.push(collapsible_section(
            "sel_props",
            "Properties",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_edit_row("Name", &name_val, muted, move |s| {
                    PanelMsg::EditSymbolValue(id, s)
                }));
                let base_lib = base_lib.clone();
                let current_rot = rot_current;
                c = c.push(form_pick_row(
                    "Style",
                    style_options,
                    current_style.to_string(),
                    move |style_label| {
                        // Map Altium Style label → lib_id keyword the built-in
                        // power renderer recognizes.
                        let tag = match style_label.as_str() {
                            "Bar" => "bar",
                            "Arrow" => "arrow",
                            "Wave" => "wave",
                            "Circle" => "circle",
                            "Power Ground" => "GND",
                            "Signal Ground" => "GNDREF",
                            "Earth" => "Earth",
                            _ => "bar",
                        };
                        let new_lib = format!("power:{tag}");
                        // The built-in renderer flips body direction when
                        // lib_id contains "gnd". Compensate rotation so the
                        // port keeps its current visual orientation: if we
                        // are switching between gnd-like and non-gnd-like,
                        // rotate by +180° from current, else keep current.
                        let old_gnd = base_lib.to_lowercase().contains("gnd")
                            && !base_lib.to_lowercase().contains("earth");
                        let new_gnd = new_lib.to_lowercase().contains("gnd")
                            && !new_lib.to_lowercase().contains("earth");
                        let target_rot = if old_gnd != new_gnd {
                            (current_rot + 180.0).rem_euclid(360.0)
                        } else {
                            current_rot
                        };
                        PanelMsg::EditPowerPortStyle {
                            symbol_id: id,
                            new_lib_id: new_lib,
                            rotation_degrees: target_rot,
                        }
                    },
                    muted,
                ));
                c
            },
        ));

        // Add Font + B/I/U/T row to Properties section via a second collapsible
        col = col.push(collapsible_section(
            "sel_props_font",
            "Font",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                let font_opts: Vec<String> = vec![
                    "Iosevka Fixed SS03".into(),
                    "Roboto".into(),
                    "Fira Code".into(),
                    "Arial".into(),
                    "Times New Roman".into(),
                ];
                c = c.push(form_pick_row(
                    "Font",
                    font_opts,
                    "Iosevka Fixed SS03".to_string(),
                    |_| PanelMsg::Noop,
                    muted,
                ));
                let size_opts: Vec<String> = [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72]
                    .iter()
                    .map(|n| n.to_string())
                    .collect();
                c = c.push(form_pick_row(
                    "Size",
                    size_opts,
                    "10".to_string(),
                    move |s| {
                        let pt: u32 = s.parse().unwrap_or(10);
                        PanelMsg::EditSymbolValueFontSizePt(id, pt)
                    },
                    muted,
                ));
                c = c.push(font_style_row(muted, primary, input_bg, input_bdr));
                c
            },
        ));

        // ── General (Net) — informational ──
        let phys_name = value.clone();
        col = col.push(collapsible_section(
            "sel_net",
            "General (Net)",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_input_row(
                    "Physical Name",
                    &phys_name,
                    muted,
                    input_bg,
                    input_bdr,
                ));
                c = c.push(form_input_row(
                    "Net Name", &phys_name, muted, input_bg, input_bdr,
                ));
                c = c.push(net_numeric_row(
                    "Power Net",
                    "0.000",
                    "V",
                    muted,
                    input_bg,
                    input_bdr,
                ));
                c = c.push(net_numeric_row(
                    "High Speed",
                    "0.000",
                    "Hz",
                    muted,
                    input_bg,
                    input_bdr,
                ));
                let dp_opts: Vec<String> = vec!["None".into()];
                c = c.push(form_pick_row(
                    "Differential Pair",
                    dp_opts,
                    "None".to_string(),
                    |_| PanelMsg::Noop,
                    muted,
                ));
                c
            },
        ));

        // ── Parameters (Net) — placeholder (no parameters/rules/classes yet) ──
        col = col.push(collapsible_section(
            "sel_net_params",
            "Parameters (Net)",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(net_params_tabs(primary, muted, input_bg, input_bdr));
                c = c.push(net_params_header(muted, border_c));
                c = c.push(empty_section_row("No Parameters", muted, border_c));
                c = c.push(empty_section_row("No Rules", muted, border_c));
                c = c.push(empty_section_row("No Classes", muted, border_c));
                c = c.push(net_params_add_bar(muted, input_bg, input_bdr));
                c
            },
        ));

        return scrollable(col).width(Length::Fill).into();
    }

    match selected_kind {
        Some(signex_types::schematic::SelectedKind::Symbol) => {
            let reference = get("Reference");
            let value = get("Value");
            let description = get("Description");
            let datasheet = get("Datasheet");
            let footprint = get("Footprint");
            let lib_id = get("Library ID");
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
            // Custom parameters: every ("Param: NAME", value) tuple.
            let params: Vec<(String, String)> = ctx
                .selection_info
                .iter()
                .filter_map(|(k, v)| {
                    k.strip_prefix("Param: ")
                        .map(|name| (name.to_string(), v.clone()))
                })
                .collect();

            if let Some(id) = uuid {
                // General section — editable
                col = col.push(collapsible_section(
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
                        c = c.push(form_edit_row("Footprint", &footprint, muted, move |s| {
                            PanelMsg::EditSymbolFootprint(id, s)
                        }));
                        if !datasheet.is_empty() {
                            c = c.push(form_input_row(
                                "Datasheet",
                                &datasheet,
                                muted,
                                input_bg,
                                input_bdr,
                            ));
                        }
                        c = c.push(form_input_row(
                            "Library ID",
                            &lib_id,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c
                    },
                ));

                // Location section — read-only for now
                col = col.push(collapsible_section(
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
                ));

                // Graphical section — checkboxes
                col = col.push(collapsible_section(
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
                        ));
                        c = c.push(form_check_row(
                            "Mirror Y",
                            has_mirror_y,
                            PanelMsg::ToggleSymbolMirrorY(id),
                            muted,
                        ));
                        c = c.push(form_check_row(
                            "Locked",
                            locked,
                            PanelMsg::ToggleSymbolLocked(id),
                            muted,
                        ));
                        c = c.push(form_check_row(
                            "DNP",
                            dnp,
                            PanelMsg::ToggleSymbolDnp(id),
                            muted,
                        ));
                        c
                    },
                ));

                // Parameters section — custom fields carried on the symbol
                // instance. Read-only for v0.6; editing per-field lands in
                // v0.7 with the parameter-manager dialog.
                let header_label = if params.is_empty() {
                    "Parameters (none)".to_string()
                } else {
                    format!("Parameters ({})", params.len())
                };
                let section_params = params.clone();
                col = col.push(collapsible_section(
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
                                    text("No custom parameters".to_string())
                                        .size(11)
                                        .color(muted),
                                )
                                .padding([6, 8]),
                            );
                        } else {
                            for (name, value) in &section_params {
                                c = c.push(form_input_row(name, value, muted, input_bg, input_bdr));
                            }
                        }
                        c
                    },
                ));
            }
        }
        Some(signex_types::schematic::SelectedKind::SymbolRefField)
        | Some(signex_types::schematic::SelectedKind::SymbolValField) => {
            let text_value = get("Text");
            let position = get("Position");
            let rotation = get("Rotation");
            let text_size = get("Text Size");
            let justify_h = get("Justify H");
            let justify_v = get("Justify V");
            let visible = get("Visible");
            let fields_autoplaced = get("Fields Autoplaced");
            let is_reference = matches!(
                selected_kind,
                Some(signex_types::schematic::SelectedKind::SymbolRefField)
            );

            if let Some(id) = uuid {
                col = col.push(collapsible_section(
                    "sel_basic",
                    "Basic Properties",
                    &ctx.collapsed_sections,
                    muted,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        c = c.push(form_input_row(
                            "Field",
                            if is_reference { "Reference" } else { "Value" },
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c = c.push(form_edit_row("Text", &text_value, muted, move |s| {
                            if is_reference {
                                PanelMsg::EditSymbolDesignator(id, s)
                            } else {
                                PanelMsg::EditSymbolValue(id, s)
                            }
                        }));
                        c = c.push(form_input_row(
                            "Visible", &visible, muted, input_bg, input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Fields Autoplaced",
                            &fields_autoplaced,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c
                    },
                ));

                col = col.push(collapsible_section(
                    "sel_text",
                    "Text Properties",
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
                        c = c.push(form_input_row(
                            "Justify H",
                            &justify_h,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Justify V",
                            &justify_v,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Text Size",
                            &text_size,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c
                    },
                ));
            }
        }
        Some(signex_types::schematic::SelectedKind::Label) => {
            // Net Name stored in Standard escapes `/` as `{slash}`. Show the
            // visible form in the panel; the edit handler re-escapes on save.
            let label_text = crate::schematic_runtime::text::expand_char_escapes(&get("Text"));
            let position = get("Position");
            let rotation_str = get("Rotation");
            let text_size_str = get("Text Size");
            let justify_h_str = get("Justify H");

            // Parse numeric values for edit controls (with fallbacks).
            let rotation_deg = rotation_str
                .trim_end_matches('°')
                .trim()
                .parse::<f64>()
                .unwrap_or(0.0);
            let text_size_pt = text_size_str.parse::<u32>().unwrap_or(10);
            let justify_h = match justify_h_str.as_str() {
                "Left" => signex_types::schematic::HAlign::Left,
                "Right" => signex_types::schematic::HAlign::Right,
                _ => signex_types::schematic::HAlign::Center,
            };

            if let Some(id) = uuid {
                // ── Location ──
                let pos_clone = position.clone();
                let rot_current = rotation_deg;
                col = col.push(collapsible_section(
                    "sel_location",
                    "Location",
                    &ctx.collapsed_sections,
                    primary,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        c = c.push(form_input_row(
                            "(X/Y)", &pos_clone, muted, input_bg, input_bdr,
                        ));
                        let rotation_opts: Vec<String> = vec![
                            "0 Degrees".into(),
                            "90 Degrees".into(),
                            "180 Degrees".into(),
                            "270 Degrees".into(),
                        ];
                        let rot_label = format!("{:.0} Degrees", rot_current);
                        c = c.push(form_pick_row(
                            "Rotation",
                            rotation_opts,
                            rot_label,
                            move |s| {
                                let deg = s
                                    .split_whitespace()
                                    .next()
                                    .and_then(|n| n.parse::<f64>().ok())
                                    .unwrap_or(0.0);
                                PanelMsg::EditLabelRotation(id, deg)
                            },
                            muted,
                        ));
                        c
                    },
                ));

                // ── Properties (Net Name, Font, Justification) ──
                let net_name = label_text.clone();
                col = col.push(collapsible_section(
                    "sel_props",
                    "Properties",
                    &ctx.collapsed_sections,
                    primary,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        c = c.push(form_edit_row("Net Name", &net_name, muted, move |s| {
                            PanelMsg::EditLabelText(id, s)
                        }));
                        // Font family + size + color (color + family are cosmetic for now)
                        let font_opts: Vec<String> = crate::fonts::system_font_families().clone();
                        let default_font = font_opts
                            .iter()
                            .find(|f| f.to_lowercase().contains("iosevka"))
                            .cloned()
                            .unwrap_or_else(|| font_opts.first().cloned().unwrap_or_default());
                        c = c.push(form_pick_row(
                            "Font",
                            font_opts,
                            default_font,
                            |_| PanelMsg::Noop,
                            muted,
                        ));
                        let size_opts: Vec<String> =
                            [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72]
                                .iter()
                                .map(|n| n.to_string())
                                .collect();
                        c = c.push(form_pick_row(
                            "Font Size",
                            size_opts,
                            text_size_pt.to_string(),
                            move |s| {
                                let pt: u32 = s.parse().unwrap_or(10);
                                PanelMsg::EditLabelFontSizePt(id, pt)
                            },
                            muted,
                        ));
                        // B/I/U/T row — cosmetic for now.
                        c = c.push(font_style_row(muted, primary, input_bg, input_bdr));
                        // 3x3 Justification grid — Altium's 9-point anchor picker.
                        c = c.push(form_label("Justification", muted));
                        c = c.push(
                            container(justification_grid(
                                id,
                                rotation_deg,
                                justify_h,
                                input_bg,
                                input_bdr,
                                primary,
                                muted,
                                ctx.theme_id,
                            ))
                            .padding([4, 8]),
                        );
                        c
                    },
                ));
            }
        }
        Some(signex_types::schematic::SelectedKind::TextNote) => {
            let note_text = get("Text");
            let position = get("Position");
            let rotation = get("Rotation");
            let text_size = get("Text Size");
            let justify_h = get("Justify H");
            let justify_v = get("Justify V");

            if let Some(id) = uuid {
                col = col.push(collapsible_section(
                    "sel_basic",
                    "Basic Properties",
                    &ctx.collapsed_sections,
                    muted,
                    border_c,
                    move || {
                        let mut c = Column::new().spacing(0).width(Length::Fill);
                        c = c.push(form_edit_row("Text", &note_text, muted, move |s| {
                            PanelMsg::EditTextNoteText(id, s)
                        }));
                        c
                    },
                ));

                col = col.push(collapsible_section(
                    "sel_text",
                    "Text Properties",
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
                        c = c.push(form_input_row(
                            "Justify H",
                            &justify_h,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Justify V",
                            &justify_v,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c = c.push(form_input_row(
                            "Text Size",
                            &text_size,
                            muted,
                            input_bg,
                            input_bdr,
                        ));
                        c
                    },
                ));
            }
        }
        Some(signex_types::schematic::SelectedKind::Drawing) => {
            col = col.push(view_drawing_properties(ctx, muted, primary, border_c));
        }
        Some(signex_types::schematic::SelectedKind::ChildSheet) => {
            col = col.push(view_child_sheet_properties(ctx, muted, primary, border_c));
        }
        _ => {
            // Generic read-only properties for other types
            let info: Vec<(String, String)> = ctx
                .selection_info
                .iter()
                .filter(|(k, _)| k != "Type")
                .cloned()
                .collect();
            col = col.push(collapsible_section(
                "sel_general",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    for (key, value) in &info {
                        c = c.push(prop_kv_row(key, value, muted, primary));
                    }
                    c
                },
            ));
        }
    }

    // ── Status bar ──
    col = col.push(Space::new().height(8.0));
    col = col.push(thin_sep(border_c));
    col = col.push(container(text("1 object selected").size(10).color(muted)).padding([4, 8]));

    scrollable(col).width(Length::Fill).into()
}
pub(super) fn view_drawing_properties<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    _primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    use signex_types::schematic::FillType;
    let get = |key: &str| -> String {
        ctx.selection_info
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let parse_pair = |s: &str| -> (f64, f64) {
        let parts: Vec<&str> = s.split(',').collect();
        let x = parts
            .first()
            .and_then(|p| p.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        let y = parts
            .get(1)
            .and_then(|p| p.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        (x, y)
    };
    let parse_f64 = |s: &str| -> f64 { s.trim().parse::<f64>().ok().unwrap_or(0.0) };
    let parse_fill = |s: &str| -> FillType {
        match s {
            "Outline" => FillType::Outline,
            "Background" => FillType::Background,
            _ => FillType::None,
        }
    };

    let elem_type = get("Type");
    // Line + Arc store their stroke in the `Width` key; Rect / Circle
    // / Polygon put stroke under `Border` and use `Width` for the
    // X-dimension (Rect) or nothing (Circle / Polygon). Picking the
    // wrong one here pre-fills the input with the X-dim or 0.
    let stroke_w = match elem_type.as_str() {
        "Line" | "Arc" => parse_f64(&get("Width")),
        _ => parse_f64(&get("Border")),
    };
    let fill = parse_fill(&get("Fill"));
    let show_fill = matches!(elem_type.as_str(), "Rectangle" | "Circle" | "Polygon");

    let mut col = Column::new().spacing(0).width(Length::Fill);
    // Header: shape icon + type label. Draft SVGs live at
    // assets/icons/shape_*.svg and can be swapped out for final art
    // without touching the panel code.
    let header_row: Element<'a, PanelMsg> =
        if let Some(icon) = shape_icon_handle(&elem_type, ctx.theme_id) {
            row![
                iced::widget::svg(icon).width(16).height(16),
                text(elem_type.clone())
                    .size(11)
                    .color(Color::from_rgb(0.90, 0.90, 0.92)),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            text(elem_type.clone())
                .size(11)
                .color(Color::from_rgb(0.90, 0.90, 0.92))
                .into()
        };
    col = col.push(container(header_row).padding([6, 8]).width(Length::Fill));
    col = col.push(thin_sep(border_c));

    // Live preview canvas — shows the selected shape at panel scale
    // with optional radius/angle annotations so edits to the rows
    // below re-render the preview immediately.
    if let Some(drawing) = &ctx.selected_drawing {
        let preview = DrawingPreview {
            drawing: drawing.clone(),
            stroke: Color::from_rgb(0.94, 0.74, 0.28),
            fill: Color::from_rgb(0.94, 0.74, 0.28),
            muted,
            accent: Color::from_rgb(0.24, 0.62, 0.97),
        };
        let canvas_w: Element<'a, PanelMsg> = iced::widget::canvas(preview)
            .width(Length::Fill)
            .height(Length::Fixed(160.0))
            .into();
        col = col.push(
            container(canvas_w)
                .padding([8, 12])
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.07, 0.07, 0.08, 0.6))),
                    border: Border {
                        width: 1.0,
                        color: border_c,
                        radius: 0.0.into(),
                    },
                    ..container::Style::default()
                }),
        );
    }

    let buf = &ctx.drawing_edit_buf;
    match elem_type.as_str() {
        "Line" => {
            let (sx, sy) = parse_pair(&get("Start"));
            let (ex, ey) = parse_pair(&get("End"));
            col = col.push(collapsible_section(
                "draw_line",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(drawing_num_row(
                        "Start X",
                        DrawingFieldId::LineStartX,
                        sx,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Start Y",
                        DrawingFieldId::LineStartY,
                        sy,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "End X",
                        DrawingFieldId::LineEndX,
                        ex,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "End Y",
                        DrawingFieldId::LineEndY,
                        ey,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Width (mm)",
                        DrawingFieldId::LineWidth,
                        stroke_w,
                        buf,
                        muted,
                    ));
                    c
                },
            ));
        }
        "Rectangle" => {
            let (px, py) = parse_pair(&get("Position"));
            let w_mm = parse_f64(&get("Width"));
            let h_mm = parse_f64(&get("Height"));
            col = col.push(collapsible_section(
                "draw_rect",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(drawing_num_row(
                        "Position X",
                        DrawingFieldId::RectStartX,
                        px,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Position Y",
                        DrawingFieldId::RectStartY,
                        py,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Width (mm)",
                        DrawingFieldId::RectWidth,
                        w_mm,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Height (mm)",
                        DrawingFieldId::RectHeight,
                        h_mm,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Border",
                        DrawingFieldId::RectBorder,
                        stroke_w,
                        buf,
                        muted,
                    ));
                    if show_fill {
                        c = c.push(drawing_fill_row(fill, muted, border_c));
                    }
                    c
                },
            ));
        }
        "Circle" => {
            let (cx, cy) = parse_pair(&get("Center"));
            let radius = parse_f64(&get("Radius"));
            col = col.push(collapsible_section(
                "draw_circle",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(drawing_num_row(
                        "Center X",
                        DrawingFieldId::CircleCenterX,
                        cx,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Center Y",
                        DrawingFieldId::CircleCenterY,
                        cy,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Radius",
                        DrawingFieldId::CircleRadius,
                        radius,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Border",
                        DrawingFieldId::CircleBorder,
                        stroke_w,
                        buf,
                        muted,
                    ));
                    if show_fill {
                        c = c.push(drawing_fill_row(fill, muted, border_c));
                    }
                    c
                },
            ));
        }
        "Arc" => {
            let (cx, cy) = parse_pair(&get("Center"));
            let radius = parse_f64(&get("Radius"));
            let start_angle = parse_f64(&get("Start Angle"));
            let end_angle = parse_f64(&get("End Angle"));
            col = col.push(collapsible_section(
                "draw_arc",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(drawing_num_row(
                        "Center X",
                        DrawingFieldId::ArcCenterX,
                        cx,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Center Y",
                        DrawingFieldId::ArcCenterY,
                        cy,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Radius",
                        DrawingFieldId::ArcRadius,
                        radius,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Start Angle",
                        DrawingFieldId::ArcStartAngle,
                        start_angle,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "End Angle",
                        DrawingFieldId::ArcEndAngle,
                        end_angle,
                        buf,
                        muted,
                    ));
                    c = c.push(drawing_num_row(
                        "Width",
                        DrawingFieldId::ArcWidth,
                        stroke_w,
                        buf,
                        muted,
                    ));
                    c
                },
            ));
        }
        "Polygon" => {
            let vert_count = parse_f64(&get("Vertices")) as i32;
            col = col.push(collapsible_section(
                "draw_poly",
                "Properties",
                &ctx.collapsed_sections,
                muted,
                border_c,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(prop_kv_row(
                        "Vertices",
                        &vert_count.to_string(),
                        muted,
                        Color::from_rgb(0.90, 0.90, 0.92),
                    ));
                    c = c.push(drawing_num_row(
                        "Border",
                        DrawingFieldId::PolyBorder,
                        stroke_w,
                        buf,
                        muted,
                    ));
                    if show_fill {
                        c = c.push(drawing_fill_row(fill, muted, border_c));
                    }
                    c
                },
            ));
        }
        _ => {
            for (key, value) in &ctx.selection_info {
                if key != "Type" {
                    col = col.push(prop_kv_row(
                        key,
                        value,
                        muted,
                        Color::from_rgb(0.9, 0.9, 0.92),
                    ));
                }
            }
        }
    }
    // Stroke colour swatch row — applies to every drawing kind that
    // matched a known variant above. Reads the current stored colour
    // from the live SchDrawing so the active tile highlights correctly.
    if matches!(
        elem_type.as_str(),
        "Line" | "Rectangle" | "Circle" | "Arc" | "Polygon"
    ) {
        let current_color = ctx.selected_drawing.as_ref().and_then(|d| match d {
            signex_types::schematic::SchDrawing::Line { stroke_color, .. }
            | signex_types::schematic::SchDrawing::Rect { stroke_color, .. }
            | signex_types::schematic::SchDrawing::Circle { stroke_color, .. }
            | signex_types::schematic::SchDrawing::Arc { stroke_color, .. }
            | signex_types::schematic::SchDrawing::Polyline { stroke_color, .. } => *stroke_color,
        });
        col = col.push(drawing_stroke_color_row(current_color, muted));
    }
    col.into()
}

/// Properties section for a single hierarchical child sheet.
/// Shows read-only info (Name / File / Position / Size) plus
/// editable Border Colour, Fill Colour and Line Width with a
/// Reset-to-default button. Colour edits open an iced_aw
/// ColorPicker overlay anchored to a swatch button.
pub(super) fn view_child_sheet_properties<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let Some(child_sheet) = ctx.selected_child_sheet.as_ref() else {
        return Column::new().width(Length::Fill).into();
    };

    let id = child_sheet.uuid;
    let name = child_sheet.name.clone();
    let filename = child_sheet.filename.clone();
    let position = format!(
        "{:.2}, {:.2}",
        child_sheet.position.x, child_sheet.position.y
    );
    let size = format!("{:.1} x {:.1} mm", child_sheet.size.0, child_sheet.size.1);

    let stroke_width = child_sheet.stroke_width;
    let stroke_color = child_sheet.stroke_color;
    let fill_color = child_sheet.fill_color;
    let border_picker_open = ctx.child_sheet_border_picker_open;
    let fill_picker_open = ctx.child_sheet_fill_picker_open;
    let border_advanced_open = ctx.child_sheet_border_advanced_open;
    let fill_advanced_open = ctx.child_sheet_fill_advanced_open;
    let stroke_width_buf = ctx.child_sheet_stroke_width_buf.clone();

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    // ── Properties (read-only identity / geometry) ──
    col = col.push(collapsible_section(
        "sel_child_sheet_props",
        "Properties",
        &ctx.collapsed_sections,
        muted,
        border_c,
        move || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            c = c.push(prop_kv_row("Name", &name, muted, primary));
            c = c.push(prop_kv_row("File", &filename, muted, primary));
            c = c.push(prop_kv_row("Position", &position, muted, primary));
            c = c.push(prop_kv_row("Size", &size, muted, primary));
            c
        },
    ));

    // ── Style (editable) ──
    col = col.push(collapsible_section(
        "sel_child_sheet_style",
        "Style",
        &ctx.collapsed_sections,
        muted,
        border_c,
        move || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            c = c.push(child_sheet_color_row(
                "Border Colour",
                id,
                stroke_color,
                border_picker_open,
                border_advanced_open,
                muted,
                border_c,
                /* is_border */ true,
            ));
            c = c.push(child_sheet_color_row(
                "Fill Colour",
                id,
                fill_color,
                fill_picker_open,
                fill_advanced_open,
                muted,
                border_c,
                /* is_border */ false,
            ));
            c = c.push(child_sheet_stroke_width_row(
                id,
                stroke_width,
                stroke_width_buf,
                muted,
                border_c,
            ));
            c = c.push(
                container(
                    iced::widget::button(
                        text("Reset to Default")
                            .size(10)
                            .color(Color::from_rgb(0.92, 0.92, 0.94)),
                    )
                    .padding([4, 10])
                    .on_press(PanelMsg::ResetChildSheetStyle(id))
                    .style(iced::widget::button::secondary),
                )
                .padding([6, 8])
                .width(Length::Fill),
            );
            c
        },
    ));

    col.into()
}

/// One swatch row in the child-sheet Style section.
///
/// Click flow:
///   1. Click swatch → expands an inline preset palette panel below
///      the row (full panel width, like the canvas-font popup) with
///      a 12-colour grid plus "Custom…" and (when an override is
///      active) "Reset to Default".
///   2. Click the swatch again to collapse, or click "Custom…" to
///      switch to the iced_aw HSV / RGB ColorPicker overlay.
///
/// Both the palette pick and the advanced-picker submit reuse the
/// same `EditChildSheet*Color` message so engine command + undo/redo
/// round-trip is identical for both paths.
fn child_sheet_color_row<'a>(
    label: &'a str,
    sheet_id: uuid::Uuid,
    current: Option<signex_types::schematic::StrokeColor>,
    show_picker: bool,
    show_advanced: bool,
    muted: Color,
    border_c: Color,
    is_border: bool,
) -> Element<'a, PanelMsg> {
    let preview_color = current
        .map(|c| {
            iced::Color::from_rgba(
                c.r as f32 / 255.0,
                c.g as f32 / 255.0,
                c.b as f32 / 255.0,
                c.a as f32 / 255.0,
            )
        })
        .unwrap_or(iced::Color::from_rgba(0.5, 0.5, 0.5, 0.4));
    let label_text = if let Some(c) = current {
        format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
    } else {
        "Default".to_string()
    };

    let toggle_msg = if is_border {
        PanelMsg::ToggleChildSheetBorderPicker(sheet_id)
    } else {
        PanelMsg::ToggleChildSheetFillPicker(sheet_id)
    };

    // Swatch button: 18x18 colour fill + small hex / "Default" caption.
    let swatch_color = preview_color;
    let swatch: Element<'a, PanelMsg> = container(Space::new())
        .width(18)
        .height(18)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(swatch_color)),
            border: Border {
                width: 1.0,
                color: border_c,
                radius: 2.0.into(),
            },
            ..container::Style::default()
        })
        .into();

    let swatch_button = iced::widget::button(
        row![
            swatch,
            text(label_text)
                .size(10)
                .color(Color::from_rgb(0.90, 0.90, 0.92)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 6])
    .on_press(toggle_msg)
    .style(iced::widget::button::secondary);

    // Build the per-channel "submit colour" message.
    let make_submit = move |c: iced::Color| -> PanelMsg {
        if is_border {
            PanelMsg::EditChildSheetBorderColor(sheet_id, c)
        } else {
            PanelMsg::EditChildSheetFillColor(sheet_id, c)
        }
    };

    // ── Advanced (HSV / RGB) overlay ──
    if show_advanced {
        let picker = iced_aw::ColorPicker::new(
            true,
            preview_color,
            swatch_button,
            PanelMsg::CancelChildSheetColorPicker,
            move |c| make_submit(c),
        );
        return container(
            row![
                text(label.to_string()).size(10).color(muted).width(96),
                picker,
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8])
        .width(Length::Fill)
        .into();
    }

    // The header row (label + swatch button) is always shown.
    let header = container(
        row![
            text(label.to_string()).size(10).color(muted).width(96),
            swatch_button,
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([4, 8])
    .width(Length::Fill);

    if !show_picker {
        return header.into();
    }

    // ── Inline preset palette (rendered below the row, full width) ──
    let presets: [(&str, [u8; 3]); 12] = [
        ("Black", [0x00, 0x00, 0x00]),
        ("Dark Gray", [0x40, 0x40, 0x40]),
        ("Gray", [0x80, 0x80, 0x80]),
        ("White", [0xFF, 0xFF, 0xFF]),
        ("Red", [0xC0, 0x39, 0x2B]),
        ("Orange", [0xE6, 0x7E, 0x22]),
        ("Yellow", [0xF1, 0xC4, 0x0F]),
        ("Olive", [0xB4, 0xA5, 0x58]),
        ("Green", [0x27, 0xAE, 0x60]),
        ("Teal", [0x16, 0xA0, 0x85]),
        ("Blue", [0x29, 0x80, 0xB9]),
        ("Purple", [0x8E, 0x44, 0xAD]),
    ];

    // 6 columns × 2 rows of preset swatches; each cell stretches
    // proportionally so the grid always fills the available panel
    // width (no clipping in narrow docks).
    let mut palette_grid: Column<'a, PanelMsg> = Column::new().spacing(4);
    for chunk in presets.chunks(6) {
        let mut r: iced::widget::Row<'a, PanelMsg> = iced::widget::Row::new().spacing(4);
        for (_name, rgb) in chunk {
            let c = iced::Color::from_rgb(
                rgb[0] as f32 / 255.0,
                rgb[1] as f32 / 255.0,
                rgb[2] as f32 / 255.0,
            );
            let swatch_btn = iced::widget::button(Space::new())
                .width(Length::Fill)
                .height(22)
                .padding(0)
                .on_press(make_submit(c))
                .style(move |_t: &Theme, _s| iced::widget::button::Style {
                    background: Some(Background::Color(c)),
                    border: Border {
                        width: 1.0,
                        color: border_c,
                        radius: 2.0.into(),
                    },
                    ..iced::widget::button::Style::default()
                });
            r = r.push(swatch_btn);
        }
        palette_grid = palette_grid.push(r);
    }

    let mut palette_col: Column<'a, PanelMsg> =
        Column::new().spacing(6).padding([6, 8]).width(Length::Fill);
    palette_col = palette_col.push(text("Preset Colours").size(10).color(muted));
    palette_col = palette_col.push(palette_grid);

    let mut action_row: iced::widget::Row<'a, PanelMsg> =
        iced::widget::Row::new().spacing(4).width(Length::Fill);
    action_row = action_row.push(
        iced::widget::button(
            text("Custom…")
                .size(10)
                .color(Color::from_rgb(0.92, 0.92, 0.94)),
        )
        .padding([4, 10])
        .width(Length::Fill)
        .on_press(PanelMsg::OpenChildSheetAdvancedPicker(sheet_id, is_border))
        .style(iced::widget::button::secondary),
    );
    if current.is_some() {
        action_row = action_row.push(
            iced::widget::button(
                text("Reset to Default")
                    .size(10)
                    .color(Color::from_rgb(0.92, 0.92, 0.94)),
            )
            .padding([4, 10])
            .width(Length::Fill)
            .on_press(PanelMsg::ResetChildSheetStyle(sheet_id))
            .style(iced::widget::button::secondary),
        );
    }
    palette_col = palette_col.push(action_row);

    let palette_panel = container(palette_col)
        .width(Length::Fill)
        .style(move |_t: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.16, 0.16, 0.18))),
            border: Border {
                width: 1.0,
                color: border_c,
                radius: 4.0.into(),
            },
            ..container::Style::default()
        });

    column![header, container(palette_panel).padding([0, 8])]
        .spacing(4)
        .width(Length::Fill)
        .into()
}

/// Numeric stroke-width row for the child-sheet Style section.
fn child_sheet_stroke_width_row<'a>(
    sheet_id: uuid::Uuid,
    stored_value: f64,
    buffered: Option<String>,
    muted: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let display = match buffered {
        Some(s) => s,
        None => format!("{:.4}", stored_value)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string(),
    };
    let display_for_input = display.clone();
    let input = iced::widget::text_input("0.1524", &display_for_input)
        .size(11)
        .padding(4)
        .width(120)
        .on_input(move |s| PanelMsg::ChildSheetStrokeWidthTyping(sheet_id, s))
        .on_submit(PanelMsg::CommitChildSheetStrokeWidth(sheet_id))
        .style(
            move |_theme: &Theme, _status| iced::widget::text_input::Style {
                background: Background::Color(Color::from_rgba(0.07, 0.07, 0.08, 1.0)),
                border: Border {
                    width: 1.0,
                    color: border_c,
                    radius: 2.0.into(),
                },
                icon: Color::from_rgba(0.7, 0.7, 0.7, 1.0),
                placeholder: Color::from_rgba(0.5, 0.5, 0.5, 1.0),
                value: Color::from_rgb(0.95, 0.95, 0.96),
                selection: Color::from_rgba(0.24, 0.62, 0.97, 0.4),
            },
        );

    container(
        row![
            text("Line Width (mm)").size(10).color(muted).width(96),
            input,
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([4, 8])
    .width(Length::Fill)
    .into()
}

/// Buffer-backed numeric row — survives empty / partial input so the
/// user can erase and retype the whole value. Emits DrawingFieldTyping
/// on every keystroke; the handler commits to the engine when the
/// string parses as f64.
fn drawing_num_row<'a>(
    label: &'a str,
    field: DrawingFieldId,
    stored_value: f64,
    buf: &std::collections::HashMap<DrawingFieldId, String>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    use iced::widget::{row, text, text_input};
    let display = buf
        .get(&field)
        .cloned()
        .unwrap_or_else(|| format!("{stored_value:.3}"));
    row![
        text(label)
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        text_input("", &display)
            .size(11)
            .on_input(move |s| PanelMsg::DrawingFieldTyping(field, s))
            .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

/// Altium-style stroke colour swatch row. A small preset palette
/// (Theme/Red/Green/Blue/Yellow/Orange/White/Black) lets the user
/// recolour a placed shape without committing to a full colour
/// picker. Each tile dispatches UpdateDrawingEdit::StrokeColor.
fn drawing_stroke_color_row<'a>(
    current: Option<signex_types::schematic::StrokeColor>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    use crate::app::contracts::DrawingFieldEdit as E;
    use iced::widget::{button, row, text};
    use signex_types::schematic::StrokeColor;
    let rgb = |r: u8, g: u8, b: u8| -> StrokeColor { StrokeColor { r, g, b, a: 255 } };
    let tile = |label: &'static str,
                stored: Option<StrokeColor>,
                active: bool,
                fill_color: Color|
     -> Element<'a, PanelMsg> {
        button(text(label).size(9))
            .padding([3, 6])
            .on_press(PanelMsg::UpdateDrawingEdit(E::StrokeColor(stored)))
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(Background::Color(fill_color)),
                border: Border {
                    width: if active { 2.0 } else { 1.0 },
                    radius: 3.0.into(),
                    color: if active {
                        Color::from_rgb(1.0, 1.0, 1.0)
                    } else {
                        Color::from_rgb(0.28, 0.28, 0.32)
                    },
                },
                text_color: Color::WHITE,
                ..iced::widget::button::Style::default()
            })
            .into()
    };
    let is_active = |c: Option<StrokeColor>| -> bool {
        match (current, c) {
            (None, None) => true,
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    };
    let theme_active = current.is_none();
    row![
        text("Color")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        row![
            tile(
                "Auto",
                None,
                theme_active,
                Color::from_rgba(0.28, 0.28, 0.32, 0.6),
            ),
            tile(
                "",
                Some(rgb(0xE5, 0x3E, 0x3E)),
                is_active(Some(rgb(0xE5, 0x3E, 0x3E))),
                Color::from_rgb(0.90, 0.24, 0.24),
            ),
            tile(
                "",
                Some(rgb(0x3E, 0xA5, 0x44)),
                is_active(Some(rgb(0x3E, 0xA5, 0x44))),
                Color::from_rgb(0.24, 0.65, 0.27),
            ),
            tile(
                "",
                Some(rgb(0x3C, 0x85, 0xD6)),
                is_active(Some(rgb(0x3C, 0x85, 0xD6))),
                Color::from_rgb(0.24, 0.52, 0.84),
            ),
            tile(
                "",
                Some(rgb(0xE6, 0xB7, 0x1E)),
                is_active(Some(rgb(0xE6, 0xB7, 0x1E))),
                Color::from_rgb(0.90, 0.72, 0.12),
            ),
            tile(
                "",
                Some(rgb(0xE0, 0xE0, 0xE0)),
                is_active(Some(rgb(0xE0, 0xE0, 0xE0))),
                Color::from_rgb(0.88, 0.88, 0.88),
            ),
        ]
        .spacing(4)
        .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

fn drawing_fill_row<'a>(
    current: signex_types::schematic::FillType,
    muted: Color,
    _border_c: Color,
) -> Element<'a, PanelMsg> {
    use crate::app::contracts::DrawingFieldEdit as E;
    use signex_types::schematic::FillType;
    let tile = |label: &'static str, ft: FillType, active: bool| -> Element<'a, PanelMsg> {
        iced::widget::button(text(label).size(10))
            .padding([3, 8])
            .on_press(PanelMsg::UpdateDrawingEdit(E::Fill(ft)))
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(Background::Color(if active {
                    Color::from_rgb(0.20, 0.36, 0.58)
                } else {
                    Color::from_rgba(0.25, 0.25, 0.28, 0.4)
                })),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: Color::from_rgb(0.28, 0.28, 0.32),
                },
                text_color: if active {
                    Color::from_rgb(1.0, 1.0, 1.0)
                } else {
                    muted
                },
                ..iced::widget::button::Style::default()
            })
            .into()
    };
    row![
        text("Fill")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(2)),
        row![
            tile("None", FillType::None, current == FillType::None),
            tile("Outline", FillType::Outline, current == FillType::Outline),
            tile(
                "Background",
                FillType::Background,
                current == FillType::Background,
            ),
        ]
        .spacing(4)
        .width(Length::FillPortion(3)),
    ]
    .padding([4, PROPERTY_ROW_PAD_X])
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

// ─── Drawing preview widget ─────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DrawingPreview {
    pub drawing: signex_types::schematic::SchDrawing,
    pub stroke: Color,
    pub fill: Color,
    pub muted: Color,
    pub accent: Color,
}

impl<Message> canvas::Program<Message> for DrawingPreview {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use signex_types::schematic::SchDrawing;
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let pad = 14.0_f32;
        let view_w = (bounds.width - 2.0 * pad).max(20.0);
        let view_h = (bounds.height - 2.0 * pad).max(20.0);
        let cx_px = bounds.width / 2.0;
        let cy_px = bounds.height / 2.0;

        let (min_x, min_y, max_x, max_y) = shape_preview_bbox(&self.drawing);
        let span_w = (max_x - min_x).abs().max(0.1);
        let span_h = (max_y - min_y).abs().max(0.1);
        let scale = (view_w as f64 / span_w).min(view_h as f64 / span_h) as f32;
        let wcx = (min_x + max_x) * 0.5;
        let wcy = (min_y + max_y) * 0.5;
        let w2s = |wx: f64, wy: f64| -> Point {
            Point::new(
                cx_px + ((wx - wcx) as f32) * scale,
                cy_px + ((wy - wcy) as f32) * scale,
            )
        };

        let stroke = canvas::Stroke::default()
            .with_color(self.stroke)
            .with_width(1.8);
        let dashed = canvas::Stroke::default()
            .with_color(Color {
                a: 0.4,
                ..self.muted
            })
            .with_width(1.0);
        let annotation = canvas::Stroke::default()
            .with_color(self.accent)
            .with_width(1.4);

        match &self.drawing {
            SchDrawing::Line { start, end, .. } => {
                frame.stroke(
                    &canvas::Path::line(w2s(start.x, start.y), w2s(end.x, end.y)),
                    stroke,
                );
                let dot = |f: &mut canvas::Frame, p: Point| {
                    f.fill(&canvas::Path::circle(p, 3.0), self.accent);
                };
                dot(&mut frame, w2s(start.x, start.y));
                dot(&mut frame, w2s(end.x, end.y));
            }
            SchDrawing::Rect {
                start, end, fill, ..
            } => {
                let x0 = start.x.min(end.x);
                let x1 = start.x.max(end.x);
                let y0 = start.y.min(end.y);
                let y1 = start.y.max(end.y);
                let a = w2s(x0, y0);
                let b = w2s(x1, y1);
                let rect_pos = Point::new(a.x.min(b.x), a.y.min(b.y));
                let rect_size =
                    iced::Size::new((b.x - a.x).abs().max(1.0), (b.y - a.y).abs().max(1.0));
                let path = canvas::Path::rectangle(rect_pos, rect_size);
                if !matches!(fill, signex_types::schematic::FillType::None) {
                    frame.fill(
                        &path,
                        Color {
                            a: 0.25,
                            ..self.fill
                        },
                    );
                }
                frame.stroke(&path, stroke);
            }
            SchDrawing::Circle {
                center,
                radius,
                fill,
                ..
            } => {
                let cp = w2s(center.x, center.y);
                let rs = (*radius as f32) * scale;
                let path = canvas::Path::circle(cp, rs.max(1.0));
                if !matches!(fill, signex_types::schematic::FillType::None) {
                    frame.fill(
                        &path,
                        Color {
                            a: 0.22,
                            ..self.fill
                        },
                    );
                }
                frame.stroke(&path, stroke);
                let spoke = canvas::Path::line(cp, Point::new(cp.x + rs, cp.y));
                frame.stroke(&spoke, annotation);
            }
            SchDrawing::Arc {
                start, mid, end, ..
            } => {
                if let Some((cxw, cyw, rw)) =
                    circumcircle_points_local((start.x, start.y), (mid.x, mid.y), (end.x, end.y))
                {
                    let cp = w2s(cxw, cyw);
                    let rs = (rw as f32) * scale;
                    frame.stroke(&canvas::Path::circle(cp, rs.max(1.0)), dashed);
                    let sa = (start.y - cyw).atan2(start.x - cxw);
                    let ea = (end.y - cyw).atan2(end.x - cxw);
                    let ma = (mid.y - cyw).atan2(mid.x - cxw);
                    let (from, to) = arc_sweep_local(sa, ma, ea);
                    let steps = 64_usize;
                    let mut prev = w2s(start.x, start.y);
                    for i in 1..=steps {
                        let t = i as f64 / steps as f64;
                        let a = from + (to - from) * t;
                        let wx = cxw + rw * a.cos();
                        let wy = cyw + rw * a.sin();
                        let next = w2s(wx, wy);
                        frame.stroke(&canvas::Path::line(prev, next), stroke);
                        prev = next;
                    }
                    frame.stroke(&canvas::Path::line(cp, w2s(start.x, start.y)), annotation);
                    frame.stroke(&canvas::Path::line(cp, w2s(end.x, end.y)), annotation);
                } else {
                    frame.stroke(
                        &canvas::Path::line(w2s(start.x, start.y), w2s(mid.x, mid.y)),
                        stroke,
                    );
                    frame.stroke(
                        &canvas::Path::line(w2s(mid.x, mid.y), w2s(end.x, end.y)),
                        stroke,
                    );
                }
            }
            SchDrawing::Polyline { points, fill, .. } => {
                if points.len() >= 2 {
                    let close = !matches!(fill, signex_types::schematic::FillType::None)
                        && points.len() >= 3;
                    let path = canvas::Path::new(|b| {
                        let first = w2s(points[0].x, points[0].y);
                        b.move_to(first);
                        for p in &points[1..] {
                            b.line_to(w2s(p.x, p.y));
                        }
                        if close {
                            b.close();
                        }
                    });
                    if close {
                        frame.fill(
                            &path,
                            Color {
                                a: 0.22,
                                ..self.fill
                            },
                        );
                    }
                    frame.stroke(&path, stroke);
                    for p in points {
                        let sp = w2s(p.x, p.y);
                        frame.fill(&canvas::Path::circle(sp, 2.5), self.accent);
                    }
                }
            }
        }

        vec![frame.into_geometry()]
    }
}

fn shape_preview_bbox(d: &signex_types::schematic::SchDrawing) -> (f64, f64, f64, f64) {
    use signex_types::schematic::SchDrawing;
    match d {
        SchDrawing::Line { start, end, .. } | SchDrawing::Rect { start, end, .. } => (
            start.x.min(end.x),
            start.y.min(end.y),
            start.x.max(end.x),
            start.y.max(end.y),
        ),
        SchDrawing::Circle { center, radius, .. } => (
            center.x - *radius,
            center.y - *radius,
            center.x + *radius,
            center.y + *radius,
        ),
        SchDrawing::Arc {
            start, mid, end, ..
        } => {
            let xs = [start.x, mid.x, end.x];
            let ys = [start.y, mid.y, end.y];
            let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            (min_x, min_y, max_x, max_y)
        }
        SchDrawing::Polyline { points, .. } => {
            if points.is_empty() {
                return (-1.0, -1.0, 1.0, 1.0);
            }
            let mut min_x = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            for p in points {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
            }
            (min_x, min_y, max_x, max_y)
        }
    }
}

fn circumcircle_points_local(
    a: (f64, f64),
    b: (f64, f64),
    c: (f64, f64),
) -> Option<(f64, f64, f64)> {
    let (ax, ay) = a;
    let (bx, by) = b;
    let (cx, cy) = c;
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-9 {
        return None;
    }
    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;
    let r = ((ax - ux) * (ax - ux) + (ay - uy) * (ay - uy)).sqrt();
    Some((ux, uy, r))
}

fn arc_sweep_local(s: f64, m: f64, e: f64) -> (f64, f64) {
    use std::f64::consts::TAU;
    let norm = |a: f64| -> f64 {
        let mut t = a % TAU;
        if t < 0.0 {
            t += TAU;
        }
        t
    };
    let ccw = |a: f64, b: f64| -> f64 {
        let d = b - a;
        if d < 0.0 { d + TAU } else { d }
    };
    let sn = norm(s);
    let mn = norm(m);
    let en = norm(e);
    let s_to_m = ccw(sn, mn);
    let s_to_e = ccw(sn, en);
    if s_to_m <= s_to_e {
        (s, s + s_to_e)
    } else {
        (s, s - (TAU - s_to_e))
    }
}
