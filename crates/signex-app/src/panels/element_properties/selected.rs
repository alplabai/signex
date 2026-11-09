//! Properties surface for a single selected schematic element.
//!
//! Routes between Symbol / Reference-or-Value field / Label / TextNote
//! / Drawing / ChildSheet contexts. Moved verbatim from the former
//! single-file `element_properties` module — pure view code, zero
//! behaviour change.

use super::super::*;
use super::font_row::font_family_row;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

pub(in crate::panels) fn view_selected_element_properties<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let palette = PanelPalette::from_tokens(&ctx.tokens);
    let PanelPalette {
        input_bg,
        input_bdr,
        panel_bg,
        ..
    } = palette;
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);

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

    // ── Editable properties based on element type ──
    // Power ports get their own Altium-style panel; regular symbols keep the
    // existing designator/value/footprint layout.
    let is_power_port = elem_type == "Power Port";
    let is_placed_part =
        !is_power_port && selected_kind == Some(signex_types::schematic::SelectedKind::Symbol);

    // ── Header ──
    col = col.push(super::header::selection_header(
        ctx,
        elem_type,
        is_placed_part,
        &get("Description"),
        primary,
    ));
    col = col.push(thin_sep(border_c));

    if is_power_port && let Some(id) = uuid {
        let value = get("Value");
        let position = get("Position");
        let rotation_str = get("Rotation");
        let lib_id = get("Library ID");
        let library_name = get("Library");
        let library_drifted = get("Library Drift") == "Yes";
        let library_status = super::library_rows::LibraryStatus {
            drifted: library_drifted,
            older_instances: get("Library Older Version").parse().unwrap_or(0),
            this_one_current: get("Library This One Current") == "Yes",
            i18n: &ctx.i18n,
            missing: get("Library Status") == "Symbol missing",
        };
        let rotation_deg = rotation_degrees(&rotation_str);
        // The Style row reads the same classifier the built-in power-port
        // body is drawn from, so the style shown is the shape on the canvas.
        let current_style = signex_types::schematic::PowerPortStyle::of(&lib_id).label();
        let style_options: Vec<String> = signex_types::schematic::PowerPortStyle::ALL
            .iter()
            .map(|s| s.label().to_string())
            .collect();

        // ── Location ──
        let pos_loc = position.clone();
        let rot_current = rotation_deg;
        col = col.push(
            ctx.collapsible(
                "sel_location",
                "Location",
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(form_input_row("(X/Y)", &pos_loc, muted, input_bg));
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
                            let deg = pick_list_degrees(&s);
                            PanelMsg::EditSymbolRotation(id, deg)
                        },
                        muted,
                    ));
                    c
                },
            )
            .boxed(),
        );

        // ── Properties (Name, Style) ──
        let name_val = value.clone();
        let base_lib = lib_id.clone();
        col = col.push(
            ctx.collapsible(
                "sel_props",
                "Properties",
                &ctx.collapsed_sections,
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
                            use signex_types::schematic::PowerPortStyle;
                            let new_style = PowerPortStyle::ALL
                                .into_iter()
                                .find(|s| s.label() == style_label)
                                .unwrap_or(PowerPortStyle::Bar);
                            let new_lib = format!("power:{}", new_style.lib_tag());
                            // A ground body hangs below its connection point and
                            // every other style rises above it. Turn the port by
                            // 180° when the direction flips, so it keeps its
                            // current visual orientation.
                            let old_down = PowerPortStyle::of(&base_lib).hangs_down();
                            let target_rot = if old_down != new_style.hangs_down() {
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
                    super::library_rows::push_library_rows(
                        c,
                        id,
                        &library_name,
                        library_status,
                        muted,
                        input_bg,
                        &ctx.tokens,
                    )
                },
            )
            .boxed(),
        );

        // Add Font + B/I/U/T row to Properties section via a second collapsible
        col = col.push(
            ctx.collapsible(
                "sel_props_font",
                "Font",
                &ctx.collapsed_sections,
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
            )
            .boxed(),
        );

        // ── General (Net) — informational ──
        let phys_name = value.clone();
        col = col.push(
            ctx.collapsible(
                "sel_net",
                "General (Net)",
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(form_input_row("Physical Name", &phys_name, muted, input_bg));
                    c = c.push(form_input_row("Net Name", &phys_name, muted, input_bg));
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
            )
            .boxed(),
        );

        // ── Parameters (Net) — placeholder (no parameters/rules/classes yet) ──
        col = col.push(
            ctx.collapsible(
                "sel_net_params",
                "Parameters (Net)",
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(net_params_tabs(primary, input_bg, input_bdr));
                    c = c.push(net_params_header(muted, border_c));
                    c = c.push(empty_section_row("No Parameters", muted, border_c));
                    c = c.push(empty_section_row("No Rules", muted, border_c));
                    c = c.push(empty_section_row("No Classes", muted, border_c));
                    c = c.push(net_params_add_bar(muted, input_bg, input_bdr));
                    c
                },
            )
            .boxed(),
        );

        return scrollable(col).width(Length::Fill).boxed();
    }

    match selected_kind {
        Some(signex_types::schematic::SelectedKind::Symbol) => {
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
                                c = c.push(form_input_row(
                                    "Description",
                                    &description,
                                    lbl,
                                    input_bg,
                                ));
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
                            c = super::library_rows::push_symbol_choice_row(
                                c,
                                id,
                                &symbol_choices,
                                lbl,
                            );
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
                                    container(
                                        text(raw("No custom parameters"))
                                            .size(TS.body)
                                            .color(muted),
                                    )
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
        }
        Some(signex_types::schematic::SelectedKind::SymbolRefField)
        | Some(signex_types::schematic::SelectedKind::SymbolValField) => {
            let text_value = get("Text");
            let position = get("Position");
            let rotation = get("Rotation");
            let text_size = get("Text Size");
            let justify_h = justify_h_row(&ctx.selection_info);
            let justify_v = justify_v_row(&ctx.selection_info);
            let visible = get("Visible");
            let fields_autoplaced = get("Fields Autoplaced");
            let font_family = Some(get("Font Family")).filter(|family| !family.is_empty());
            let is_reference = matches!(
                selected_kind,
                Some(signex_types::schematic::SelectedKind::SymbolRefField)
            );

            if let Some(id) = uuid {
                col = col.push(
                    ctx.collapsible(
                        "sel_basic",
                        "Basic Properties",
                        &ctx.collapsed_sections,
                        move || {
                            let mut c = Column::new().spacing(0).width(Length::Fill);
                            c = c.push(form_input_row(
                                "Field",
                                if is_reference { "Reference" } else { "Value" },
                                muted,
                                input_bg,
                            ));
                            c = c.push(form_edit_row("Text", &text_value, muted, move |s| {
                                if is_reference {
                                    PanelMsg::EditSymbolDesignator(id, s)
                                } else {
                                    PanelMsg::EditSymbolValue(id, s)
                                }
                            }));
                            c = c.push(form_input_row("Visible", &visible, muted, input_bg));
                            c = c.push(form_input_row(
                                "Fields Autoplaced",
                                &fields_autoplaced,
                                muted,
                                input_bg,
                            ));
                            c
                        },
                    )
                    .boxed(),
                );

                col = col.push(
                    ctx.collapsible(
                        "sel_text",
                        "Text Properties",
                        &ctx.collapsed_sections,
                        move || {
                            let mut c = Column::new().spacing(0).width(Length::Fill);
                            c = c.push(form_input_row("Position", &position, muted, input_bg));
                            c = c.push(form_input_row("Rotation", &rotation, muted, input_bg));
                            c = c.push(form_input_row("Justify H", &justify_h, muted, input_bg));
                            c = c.push(form_input_row("Justify V", &justify_v, muted, input_bg));
                            c = c.push(form_input_row("Text Size", &text_size, muted, input_bg));
                            c = c.push(font_family_row(
                                "Font",
                                font_family.as_deref(),
                                muted,
                                move |family| {
                                    let target = if is_reference {
                                        signex_engine::TextTarget::SymbolReference(id)
                                    } else {
                                        signex_engine::TextTarget::SymbolValue(id)
                                    };
                                    PanelMsg::EditTextFontFamily(target, family)
                                },
                            ));
                            c
                        },
                    )
                    .boxed(),
                );
            }
        }
        Some(signex_types::schematic::SelectedKind::Label) => {
            let label_text = get("Text");
            let position = get("Position");
            let rotation_str = get("Rotation");
            let text_size_str = get("Text Size");
            let justify_h_str = justify_h_row(&ctx.selection_info);
            let label_font_family = Some(get("Font Family")).filter(|family| !family.is_empty());

            // Parse numeric values for edit controls (with fallbacks).
            let rotation_deg = rotation_degrees(&rotation_str);
            let text_size_pt = text_size_str.parse::<u32>().unwrap_or(10);
            let justify_h = h_align_from_row(&justify_h_str);

            if let Some(id) = uuid {
                // ── Location ──
                let pos_clone = position.clone();
                let rot_current = rotation_deg;
                col = col.push(
                    ctx.collapsible(
                        "sel_location",
                        "Location",
                        &ctx.collapsed_sections,
                        move || {
                            let mut c = Column::new().spacing(0).width(Length::Fill);
                            c = c.push(form_input_row("(X/Y)", &pos_clone, muted, input_bg));
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
                                    let deg = pick_list_degrees(&s);
                                    PanelMsg::EditLabelRotation(id, deg)
                                },
                                muted,
                            ));
                            c
                        },
                    )
                    .boxed(),
                );

                // ── Properties (Net Name, Font, Justification) ──
                let net_name = label_text.clone();
                // The picker works on the net the project netlist puts this
                // label on (a child sheet's local `LOCAL` is `/LOCAL`), in the
                // table of the project the label's sheet belongs to.
                let net_class_picker = ctx.net_classes.label.as_ref().map(|pick| {
                    (
                        pick.project,
                        pick.net.clone(),
                        crate::panels::net_class_choices(&pick.classes),
                        crate::panels::net_class_choice(&pick.classes, &pick.net),
                    )
                });
                col = col.push(
                    ctx.collapsible(
                        "sel_props",
                        "Properties",
                        &ctx.collapsed_sections,
                        move || {
                            let mut c = Column::new().spacing(0).width(Length::Fill);
                            c = c.push(form_edit_row("Net Name", &net_name, muted, move |s| {
                                PanelMsg::EditLabelText(id, s)
                            }));
                            if let Some((project, net, choices, current)) = net_class_picker {
                                c = c.push(form_pick_row(
                                    "Net Class",
                                    choices,
                                    current,
                                    move |choice| {
                                        PanelMsg::NetClass(NetClassMsg::AssignNet {
                                            project,
                                            net: net.clone(),
                                            class: choice.into_class(),
                                        })
                                    },
                                    muted,
                                ));
                            }
                            // Font family (stored on the label; size is below, colour is cosmetic for now)
                            c = c.push(font_family_row(
                                "Font",
                                label_font_family.as_deref(),
                                muted,
                                move |family| {
                                    PanelMsg::EditTextFontFamily(
                                        signex_engine::TextTarget::Label(id),
                                        family,
                                    )
                                },
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
                            c = c.push(form_label(signex_i18n::t!(
                                i18n,
                                signex_i18n::ids::PANELS_PROPERTIES_JUSTIFICATION
                            )));
                            c = c.push(
                                container(justification_grid(
                                    id,
                                    rotation_deg,
                                    justify_h,
                                    palette,
                                    ctx.theme_id,
                                ))
                                .padding([SP.s, SP.m])
                                .boxed(),
                            );
                            c
                        },
                    )
                    .boxed(),
                );
            }
        }
        Some(signex_types::schematic::SelectedKind::TextNote) => {
            let note_text = get("Text");
            let position = get("Position");
            let rotation = get("Rotation");
            let text_size = get("Text Size");
            let justify_h = justify_h_row(&ctx.selection_info);
            let justify_v = justify_v_row(&ctx.selection_info);
            let note_font_family = Some(get("Font Family")).filter(|family| !family.is_empty());

            if let Some(id) = uuid {
                col = col.push(
                    ctx.collapsible(
                        "sel_basic",
                        "Basic Properties",
                        &ctx.collapsed_sections,
                        move || {
                            let mut c = Column::new().spacing(0).width(Length::Fill);
                            c = c.push(form_edit_row("Text", &note_text, muted, move |s| {
                                PanelMsg::EditTextNoteText(id, s)
                            }));
                            c
                        },
                    )
                    .boxed(),
                );

                col = col.push(
                    ctx.collapsible(
                        "sel_text",
                        "Text Properties",
                        &ctx.collapsed_sections,
                        move || {
                            let mut c = Column::new().spacing(0).width(Length::Fill);
                            c = c.push(form_input_row("Position", &position, muted, input_bg));
                            c = c.push(form_input_row("Rotation", &rotation, muted, input_bg));
                            c = c.push(form_input_row("Justify H", &justify_h, muted, input_bg));
                            c = c.push(form_input_row("Justify V", &justify_v, muted, input_bg));
                            c = c.push(form_input_row("Text Size", &text_size, muted, input_bg));
                            c = c.push(font_family_row(
                                "Font",
                                note_font_family.as_deref(),
                                muted,
                                move |family| {
                                    PanelMsg::EditTextFontFamily(
                                        signex_engine::TextTarget::TextNote(id),
                                        family,
                                    )
                                },
                            ));
                            c
                        },
                    )
                    .boxed(),
                );
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
            // `Locked` is the checkbox row below, not a read-only line.
            let info: Vec<(String, String)> = ctx
                .selection_info
                .iter()
                .filter(|(k, _)| k != "Type" && k != "Locked")
                .cloned()
                .collect();
            col = col.push(
                ctx.collapsible("sel_general", "Properties", &ctx.collapsed_sections, || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    for (key, value) in &info {
                        c = c.push(prop_kv_row(key, value, muted, primary));
                    }
                    c
                })
                .boxed(),
            );
        }
    }

    // ── Wire style ──: width preset and own colour, undoable.
    if let (Some(id), Some((width, color))) = (uuid, ctx.selected_wire_style) {
        let picker = ctx.wire_color_picker;
        col = col.push(
            ctx.collapsible(
                "sel_wire_style",
                "Style",
                &ctx.collapsed_sections,
                move || {
                    Column::new()
                        .spacing(0)
                        .width(Length::Fill)
                        .push(crate::panels::wire_width_row(
                            width,
                            move |preset| PanelMsg::EditWireWidth(id, preset),
                            muted,
                        ))
                        .push(crate::panels::wire_color_row(
                            color,
                            picker,
                            std::rc::Rc::new(move |c| PanelMsg::EditWireColor(id, c)),
                            muted,
                            border_c,
                        ))
                },
            )
            .boxed(),
        );
    }

    // ── Lock ── (#1573): wires, labels, text notes and drawings.
    if let (Some(id), Some(kind)) = (uuid, selected_kind)
        && matches!(
            kind,
            signex_types::schematic::SelectedKind::Wire
                | signex_types::schematic::SelectedKind::Label
                | signex_types::schematic::SelectedKind::TextNote
                | signex_types::schematic::SelectedKind::Drawing
        )
    {
        let locked = get("Locked") == "Yes";
        let item = signex_types::schematic::SelectedItem::new(id, kind);
        col = col.push(
            ctx.collapsible("sel_state", "State", &ctx.collapsed_sections, move || {
                Column::new()
                    .spacing(0)
                    .width(Length::Fill)
                    .push(form_check_row(
                        "Locked",
                        locked,
                        PanelMsg::ToggleItemLocked(item),
                        muted,
                        panel_bg,
                    ))
            })
            .boxed(),
        );
    }

    // ── Status bar ──
    col = col.push(Space::new().height(8.0).boxed());
    col = col.push(thin_sep(border_c));
    col = col.push(
        container(text(raw("1 object selected")).size(TS.caption).color(muted))
            .padding([SP.s, SP.m])
            .boxed(),
    );

    scrollable(col).width(Length::Fill).boxed()
}

/// Rotation, in degrees, read from the panel's display text (`90°`). A
/// missing, malformed or non-finite value reads as `0`. Pure: this runs
/// while the panel is built, so no refusal is logged here.
fn rotation_degrees(text: &str) -> f64 {
    signex_types::numeric::parse_finite_f64(text.trim_end_matches('°')).unwrap_or(0.0)
}

/// Rotation, in degrees, from a rotation pick-list label (`90 Degrees`).
fn pick_list_degrees(label: &str) -> f64 {
    rotation_degrees(label.split_whitespace().next().unwrap_or(""))
}

/// The row `key` of a selection's info, empty when the selection has none.
fn info_row(info: &[(String, String)], key: &str) -> String {
    info.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

/// The horizontal justification row of a selection (#2024): the one lookup
/// every label, text note and symbol field panel shows it from.
fn justify_h_row(info: &[(String, String)]) -> String {
    info_row(info, signex_engine::info_keys::JUSTIFY_H)
}

/// The vertical justification row of a selection (#2024).
fn justify_v_row(info: &[(String, String)]) -> String {
    info_row(info, signex_engine::info_keys::JUSTIFY_V)
}

/// The justification a Properties row reports (`info_keys::JUSTIFY_H`).
fn h_align_from_row(row: &str) -> signex_types::schematic::HAlign {
    use signex_types::schematic::HAlign;
    match row {
        "Left" => HAlign::Left,
        "Right" => HAlign::Right,
        _ => HAlign::Center,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        h_align_from_row, justify_h_row, justify_v_row, pick_list_degrees, rotation_degrees,
    };
    use signex_types::schematic::HAlign;

    /// #828 -- a non-finite rotation text never reaches the element.
    #[test]
    fn non_finite_rotation_reads_as_zero() {
        for bad in ["nan", "NaN", "inf", "-inf", "1e999", "inf°"] {
            assert_eq!(rotation_degrees(bad), 0.0, "{bad:?}");
            assert_eq!(pick_list_degrees(&format!("{bad} Degrees")), 0.0, "{bad:?}");
        }
    }

    #[test]
    fn ordinary_rotation_still_reads() {
        assert_eq!(rotation_degrees("90°"), 90.0);
        assert_eq!(rotation_degrees(" 270 "), 270.0);
        assert_eq!(pick_list_degrees("180 Degrees"), 180.0);
        assert_eq!(pick_list_degrees(""), 0.0);
    }

    /// #2024 - a left-justified label shows Left: its row, as the engine
    /// writes it, reads back as `HAlign::Left` through the shared key.
    #[test]
    fn a_left_justified_label_row_reads_back_as_left() {
        use signex_types::schematic::{
            Label, LabelType, Point, SchematicSheet, SelectedItem, SelectedKind,
        };
        let label = Label::new("NET", Point::new(0.0, 0.0), LabelType::Net);
        let id = label.uuid;
        let engine = signex_engine::Engine::new(SchematicSheet {
            labels: vec![label],
            ..Default::default()
        })
        .expect("engine");
        let details = engine
            .describe_single_selection(&[SelectedItem::new(id, SelectedKind::Label)])
            .expect("details");
        let row = justify_h_row(&details.info);
        assert_eq!(row, "Left", "the row the panel shows");
        assert_eq!(h_align_from_row(&row), HAlign::Left);
        assert_eq!(h_align_from_row("Right"), HAlign::Right);
        assert_eq!(h_align_from_row("Center"), HAlign::Center);
    }

    /// #2024 - a text note's Properties rows carry its justification, read
    /// from the same keys the engine writes them under.
    #[test]
    fn a_text_note_row_carries_its_justification() {
        use signex_types::schematic::{
            Point, SchematicSheet, SelectedItem, SelectedKind, TextNote, VAlign,
        };
        let note = TextNote {
            uuid: uuid::Uuid::now_v7(),
            text: "note".into(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Top,
            locked: false,
        };
        let id = note.uuid;
        let engine = signex_engine::Engine::new(SchematicSheet {
            text_notes: vec![note],
            ..Default::default()
        })
        .expect("engine");
        let details = engine
            .describe_single_selection(&[SelectedItem::new(id, SelectedKind::TextNote)])
            .expect("details");
        assert_eq!(justify_h_row(&details.info), "Left");
        assert_eq!(justify_v_row(&details.info), "Top");
    }

    /// The text a laid-out Properties view shows for a selection of `kind`
    /// whose info rows are `info`.
    fn shown_texts(
        kind: signex_types::schematic::SelectedKind,
        info: Vec<(String, String)>,
    ) -> Vec<String> {
        use super::view_selected_element_properties;
        let (app, _task) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.selected_kind = Some(kind);
        ctx.selected_uuid = Some(uuid::Uuid::now_v7());
        ctx.selection_info = info;
        let view = view_selected_element_properties(
            &ctx,
            iced::Color::WHITE,
            iced::Color::WHITE,
            iced::Color::BLACK,
        );
        crate::layout_probe::lay_out_texts(view, iced::Size::new(400.0, 4000.0))
            .into_iter()
            .map(|t| t.text)
            .collect()
    }

    /// What the Label view looks like drawn, with its justification row set
    /// to `row`: the anchor picker highlights the cell the row selects.
    fn label_pixels(row: &str) -> Vec<u8> {
        use super::view_selected_element_properties;
        let (app, _task) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.selected_kind = Some(signex_types::schematic::SelectedKind::Label);
        ctx.selected_uuid = Some(uuid::Uuid::nil());
        ctx.selection_info = vec![
            ("Rotation".to_string(), "0".to_string()),
            (
                signex_engine::info_keys::JUSTIFY_H.to_string(),
                row.to_string(),
            ),
        ];
        let view = view_selected_element_properties(
            &ctx,
            iced::Color::WHITE,
            iced::Color::WHITE,
            iced::Color::BLACK,
        );
        crate::layout_probe::draw_pixels(view, iced::Size::new(400.0, 1200.0))
    }

    /// #2024 - the label view selects the anchor cell its justification row
    /// names: a Left row and a Right row draw differently. A view that read
    /// the row under another key would see neither and draw both the same.
    #[test]
    fn the_label_view_selects_the_cell_its_justification_row_names() {
        use signex_types::schematic::HAlign;
        let window = (400_usize, 1200_usize);
        let (left, right) = (label_pixels("Left"), label_pixels("Right"));
        let (left_grid, right_grid) = (anchor_grid(HAlign::Left), anchor_grid(HAlign::Right));
        assert_ne!(left_grid.pixels, right_grid.pixels, "the references differ");
        // Each row's view holds the picker drawn for its own alignment far
        // better than the other one's: a swapped row reading fails both.
        let score = |block: &Block, frame: &[u8]| block.best_match(frame, window.0);
        let (ll, lr) = (score(&left_grid, &left), score(&right_grid, &left));
        let (rr, rl) = (score(&right_grid, &right), score(&left_grid, &right));
        // The picker is laid out in a column with other controls, so the match
        // is partial; the two pickers differ by two cells, a gap of about 0.17.
        assert!(ll > lr + 0.1, "Left row: Left {ll}, Right {lr}");
        assert!(rr > rl + 0.1, "Right row: Right {rr}, Left {rl}");
    }

    /// The anchor picker drawn alone for one alignment, cropped to its
    /// non-background pixels.
    struct Block {
        pixels: Vec<u8>,
        width: usize,
        height: usize,
    }

    fn anchor_grid(h: signex_types::schematic::HAlign) -> Block {
        let (app, _task) = crate::app::Signex::new();
        let ctx = &app.document_state.panel_ctx;
        let palette = crate::panels::palette::PanelPalette::from_tokens(&ctx.tokens);
        let grid =
            crate::panels::justification_grid(uuid::Uuid::nil(), 0.0, h, palette, ctx.theme_id);
        let (w, hgt) = (120_usize, 120_usize);
        let data = crate::layout_probe::draw_pixels(grid, iced::Size::new(w as f32, hgt as f32));
        let lit = |x: usize, y: usize| data[(y * w + x) * 4..(y * w + x) * 4 + 3] != [0, 0, 0];
        let xs: Vec<usize> = (0..w).filter(|&x| (0..hgt).any(|y| lit(x, y))).collect();
        let ys: Vec<usize> = (0..hgt).filter(|&y| (0..w).any(|x| lit(x, y))).collect();
        let (x0, x1) = (xs[0], xs[xs.len() - 1]);
        let (y0, y1) = (ys[0], ys[ys.len() - 1]);
        let mut pixels = Vec::new();
        for y in y0..=y1 {
            pixels.extend_from_slice(&data[(y * w + x0) * 4..(y * w + x1 + 1) * 4]);
        }
        Block {
            pixels,
            width: x1 - x0 + 1,
            height: y1 - y0 + 1,
        }
    }

    impl Block {
        /// The best fraction of this block's pixels that equal (within a few
        /// levels) those of `frame`, `frame_width` px wide, over every
        /// position. Edges are anti-aliased, so an exact match is too strict.
        fn best_match(&self, frame: &[u8], frame_width: usize) -> f32 {
            let rows = frame.len() / 4 / frame_width;
            // The fraction of every `step`-th pixel of the block equal at (x, y).
            let at = |x: usize, y: usize, step: usize| {
                let (mut same, mut seen) = (0usize, 0usize);
                for r in (0..self.height).step_by(step) {
                    for c in (0..self.width).step_by(step) {
                        let a = ((y + r) * frame_width + x + c) * 4;
                        let b = (r * self.width + c) * 4;
                        seen += 1;
                        same += usize::from(
                            (0..3).all(|k| frame[a + k].abs_diff(self.pixels[b + k]) <= 6),
                        );
                    }
                }
                same as f32 / seen as f32
            };
            // Coarse over every 4th position on a sampled block, then the
            // full block around the best of them.
            let (mut best_at, mut best) = ((0, 0), -1.0_f32);
            for y in (0..=rows - self.height).step_by(4) {
                for x in (0..=frame_width - self.width).step_by(4) {
                    let score = at(x, y, 4);
                    if score > best {
                        (best_at, best) = ((x, y), score);
                    }
                }
            }
            let mut refined = 0.0_f32;
            for y in best_at.1.saturating_sub(4)..=(best_at.1 + 4).min(rows - self.height) {
                for x in best_at.0.saturating_sub(4)..=(best_at.0 + 4).min(frame_width - self.width)
                {
                    refined = refined.max(at(x, y, 1));
                }
            }
            refined
        }
    }

    fn justify_info() -> Vec<(String, String)> {
        vec![
            (
                signex_engine::info_keys::JUSTIFY_H.to_string(),
                "Lefty".to_string(),
            ),
            (
                signex_engine::info_keys::JUSTIFY_V.to_string(),
                "Topsy".to_string(),
            ),
        ]
    }

    /// #2024 - the symbol-field and text-note views show the justification
    /// rows under the keys the engine writes.
    #[test]
    fn the_field_and_note_views_show_the_justification_rows() {
        use signex_types::schematic::SelectedKind;
        for kind in [SelectedKind::SymbolRefField, SelectedKind::TextNote] {
            let texts = shown_texts(kind, justify_info());
            for want in ["Lefty", "Topsy"] {
                assert!(texts.iter().any(|t| t == want), "{kind:?} lacks {want}");
            }
        }
    }
}
