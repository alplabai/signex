//! Properties of a selected power port (Altium-style panel).

use super::super::*;
use super::library_rows::LibraryStatus;
use super::rows::{
    PanelStyle, font_size_options, info_row, pick_list_degrees, rotation_degrees, rotation_options,
};
use iced::Widget;

pub(super) fn push_power_port_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    id: uuid::Uuid,
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
        ..
    } = palette;
    let get = |key: &str| info_row(&ctx.selection_info, key);
    let value = get("Value");
    let position = get("Position");
    let rotation_str = get("Rotation");
    let lib_id = get("Library ID");
    let library_name = get("Library");
    let library_drifted = get("Library Drift") == "Yes";
    let library_status = LibraryStatus {
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
        collapsible_section(
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
                let rotation_opts: Vec<String> = rotation_options();
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
        collapsible_section(
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
                    input_bdr,
                )
            },
        )
        .boxed(),
    );

    // Add Font + B/I/U/T row to Properties section via a second collapsible
    col = col.push(
        collapsible_section(
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
                let size_opts: Vec<String> = font_size_options();
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
        collapsible_section(
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
        )
        .boxed(),
    );

    // ── Parameters (Net) — placeholder (no parameters/rules/classes yet) ──
    col = col.push(
        collapsible_section(
            "sel_net_params",
            "Parameters (Net)",
            &ctx.collapsed_sections,
            primary,
            border_c,
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

    col
}
