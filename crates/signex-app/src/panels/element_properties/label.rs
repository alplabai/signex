//! Properties of a selected net label.

use super::super::*;
use super::font_row::font_family_row;
use super::rows::{PanelStyle, h_align_from_row, info_row, justify_h_row, rotation_degrees};
use crate::styles::Spacing;
use iced::Widget;
use signex_i18n::{ids, t};

const SP: Spacing = Spacing::DEFAULT;

pub(super) fn push_label_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    uuid: Option<uuid::Uuid>,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let PanelStyle {
        muted,
        primary,
        palette,
        ..
    } = st;
    let PanelPalette {
        input_bg,
        input_bdr,
        ..
    } = palette;
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let get = |key: &str| info_row(&ctx.selection_info, key);
    let label_text = get("Text");
    let rotation_str = get("Rotation");
    let text_size_str = get("Text Size");
    let justify_h_str = justify_h_row(&ctx.selection_info);
    let label_font_family = Some(get("Font Family")).filter(|family| !family.is_empty());

    // Parse numeric values for edit controls (with fallbacks).
    let rotation_deg = rotation_degrees(&rotation_str);
    let text_size_pt = text_size_str.parse::<u32>().unwrap_or(10);
    let justify_h = h_align_from_row(&justify_h_str);

    if let Some(id) = uuid {
        // ── Location ── (#2418): Position and Rotation through the shared rows.
        let look = super::placed_rows::RowLook { muted, input_bg };
        col = super::placed_rows::push_location_section(col, ctx, look);

        // ── Properties (Net Name, Font, Justification) ──
        let net_name = label_text.clone();
        // The picker works on the net the project netlist puts this
        // label on (a child sheet's local `LOCAL` is `/LOCAL`), in the
        // table of the project the label's sheet belongs to.
        let named = |choice: crate::panels::NetClassChoice| crate::choice::Choice {
            label: choice.label(i18n),
            value: choice,
        };
        let net_class_picker = ctx.net_classes.label.as_ref().map(|pick| {
            (
                pick.project,
                pick.net.clone(),
                crate::panels::net_class_choices(&pick.classes)
                    .into_iter()
                    .map(named)
                    .collect::<Vec<_>>(),
                named(crate::panels::net_class_choice(&pick.classes, &pick.net)),
            )
        });
        col = col.push(
            ctx.collapsible(
                "sel_props",
                ids::PROPERTIES_SECTION_PROPERTIES,
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(form_edit_row(
                        t!(i18n, ids::PROPERTIES_NET_NAME),
                        &net_name,
                        muted,
                        move |s| PanelMsg::EditLabelText(id, s),
                    ));
                    if let Some((project, net, choices, current)) = net_class_picker {
                        c = c.push(form_choice_row(
                            t!(i18n, ids::PROPERTIES_NET_CLASS),
                            choices,
                            current,
                            move |choice: crate::panels::NetClassChoice| {
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
                        t!(i18n, ids::PROPERTIES_FONT),
                        label_font_family.as_deref(),
                        muted,
                        move |family| {
                            PanelMsg::EditTextFontFamily(
                                signex_engine::TextTarget::Label(id),
                                family,
                            )
                        },
                        i18n,
                    ));
                    let size_opts: Vec<String> = [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72]
                        .iter()
                        .map(|n| n.to_string())
                        .collect();
                    c = c.push(form_pick_row(
                        t!(i18n, ids::PROPERTIES_FONT_SIZE),
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
    col
}
