//! Properties surface for a single selected schematic element.
//!
//! Routes between Symbol / Reference-or-Value field / Label / TextNote
//! / Drawing / ChildSheet contexts. Moved verbatim from the former
//! single-file `element_properties` module — pure view code, zero
//! behaviour change.

use super::super::*;

pub(in crate::panels) fn view_selected_element_properties<'a>(
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
