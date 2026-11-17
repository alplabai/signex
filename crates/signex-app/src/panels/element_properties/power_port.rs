//! Properties of a selected power port.

use super::super::*;
use super::rows::{PanelStyle, info_row, rotation_degrees};
use crate::choice::{Choice, chosen};
use iced::Widget;
use signex_i18n::{MessageId, ids, t};
use signex_types::schematic::PowerPortStyle;

/// The catalog name of a power-port style.
fn style_label(style: PowerPortStyle) -> MessageId {
    match style {
        PowerPortStyle::Bar => ids::PROPERTIES_POWER_STYLE_BAR,
        PowerPortStyle::Arrow => ids::PROPERTIES_POWER_STYLE_ARROW,
        PowerPortStyle::Wave => ids::PROPERTIES_POWER_STYLE_WAVE,
        PowerPortStyle::Circle => ids::PROPERTIES_POWER_STYLE_CIRCLE,
        PowerPortStyle::PowerGround => ids::PROPERTIES_POWER_STYLE_POWER_GROUND,
        PowerPortStyle::SignalGround => ids::PROPERTIES_POWER_STYLE_SIGNAL_GROUND,
        PowerPortStyle::Earth => ids::PROPERTIES_POWER_STYLE_EARTH,
    }
}

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
    let library_status = super::library_rows::LibraryStatus {
        drifted: library_drifted,
        older_instances: get("Library Older Version").parse().unwrap_or(0),
        this_one_current: get("Library This One Current") == "Yes",
        i18n: &ctx.i18n,
        missing: get("Library Status") == "Symbol missing",
    };
    let rotation_deg = rotation_degrees(&rotation_str);
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    // The Style row reads the same classifier the built-in power-port
    // body is drawn from, so the style shown is the shape on the canvas.
    let style_choices = Choice::named(&PowerPortStyle::ALL, style_label, i18n);
    let current_style = chosen(&style_choices, &PowerPortStyle::of(&lib_id))
        .unwrap_or_else(|| style_choices[0].clone());

    // ── Location ──
    let pos_loc = position.clone();
    let rot_current = rotation_deg;
    col = col.push(
        ctx.collapsible(
            "sel_location",
            ids::PROPERTIES_SECTION_LOCATION,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_input_row(
                    t!(i18n, ids::PROPERTIES_XY),
                    &pos_loc,
                    muted,
                    input_bg,
                ));
                c = c.push(form_rotation_row(
                    i18n,
                    rot_current,
                    move |deg| PanelMsg::EditSymbolRotation(id, deg),
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
            ids::PROPERTIES_SECTION_PROPERTIES,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_edit_row(
                    t!(i18n, ids::PROPERTIES_NAME),
                    &name_val,
                    muted,
                    move |s| PanelMsg::EditSymbolValue(id, s),
                ));
                let base_lib = base_lib.clone();
                let current_rot = rot_current;
                c = c.push(form_choice_row(
                    t!(i18n, ids::PROPERTIES_STYLE),
                    style_choices,
                    current_style,
                    move |new_style: PowerPortStyle| {
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
            ids::PROPERTIES_SECTION_FONT,
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
                    t!(i18n, ids::PROPERTIES_FONT),
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
                    t!(i18n, ids::PROPERTIES_SIZE),
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
            ids::PROPERTIES_SECTION_NET_GENERAL,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_input_row(
                    t!(i18n, ids::PROPERTIES_PHYSICAL_NAME),
                    &phys_name,
                    muted,
                    input_bg,
                ));
                c = c.push(form_input_row(
                    t!(i18n, ids::PROPERTIES_NET_NAME),
                    &phys_name,
                    muted,
                    input_bg,
                ));
                c = c.push(net_numeric_row(
                    t!(i18n, ids::PROPERTIES_POWER_NET),
                    "0.000",
                    "V",
                    muted,
                    input_bg,
                    input_bdr,
                ));
                c = c.push(net_numeric_row(
                    t!(i18n, ids::PROPERTIES_HIGH_SPEED),
                    "0.000",
                    "Hz",
                    muted,
                    input_bg,
                    input_bdr,
                ));
                let none = i18n.message_plain(ids::PROPERTIES_DIFF_PAIR_NONE, &[]);
                c = c.push(form_pick_row(
                    t!(i18n, ids::PROPERTIES_DIFF_PAIR),
                    vec![none.clone()],
                    none,
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
            ids::PROPERTIES_SECTION_NET_PARAMETERS,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(net_params_tabs(i18n, primary, input_bg, input_bdr));
                c = c.push(net_params_header(i18n, muted, border_c));
                for empty in [
                    ids::PROPERTIES_NET_NO_PARAMETERS,
                    ids::PROPERTIES_NET_NO_RULES,
                    ids::PROPERTIES_NET_NO_CLASSES,
                ] {
                    c = c.push(empty_section_row(t!(i18n, empty), muted, border_c));
                }
                c = c.push(net_params_add_bar(i18n, muted, input_bg, input_bdr));
                c
            },
        )
        .boxed(),
    );

    col
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each style reads as `PowerPortStyle::label`, the English the list
    /// showed before, and a choice is found by its value.
    #[test]
    fn each_style_keeps_its_english_name() {
        let en = signex_i18n::I18n::en_us();
        let choices = Choice::named(&PowerPortStyle::ALL, style_label, &en);
        for style in PowerPortStyle::ALL {
            assert_eq!(en.message_plain(style_label(style), &[]), style.label());
            assert_eq!(
                chosen(&choices, &style).map(|c| c.label).as_deref(),
                Some(style.label())
            );
        }
    }
}
