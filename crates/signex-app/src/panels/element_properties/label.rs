//! Properties of a selected net label or port (mockup row 44): the illustration, then
//! Location, then Properties (the name, its Net Class, Font, Font Size with
//! B / I / U / T, and a net label's justification grid naming its anchor).
//! State ▸ Locked follows from the shared router.

use super::super::*;
use super::font_row::font_family_row;
use super::rows::{
    PanelStyle, h_align_from_row, info_row, justify_h_row, labelled_row_top, rotation_degrees,
};
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{ids, t};
use signex_widgets::dropdown_style::GraphiteDropdown as _;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The text sizes the Font Size list offers, in points.
const FONT_SIZES_PT: [u32; 13] = [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72];

/// Whether the selected label is a port (global or hierarchical), named
/// from the engine's type row.
pub(super) fn is_port(elem_type: &str) -> bool {
    matches!(elem_type, "Global Label" | "Hierarchical Label")
}

pub(super) fn push_label_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    uuid: Option<uuid::Uuid>,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let PanelStyle { muted, palette, .. } = st;
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let get = |key: &str| info_row(&ctx.selection_info, key);
    let label_text = get("Text");
    let port = is_port(&get("Type"));
    let rotation_deg = rotation_degrees(&get("Rotation"));
    let text_size_pt = get("Text Size").parse::<u32>().unwrap_or(10);
    let justify_h = h_align_from_row(&justify_h_row(&ctx.selection_info));
    let label_font_family = Some(get("Font Family")).filter(|family| !family.is_empty());

    let Some(id) = uuid else {
        return col;
    };
    let illustration = if port {
        super::illustrations::port(&label_text, i18n)
    } else {
        super::illustrations::net_label(&label_text, justify_h, i18n)
    };
    col = col.push(super::illustrations::show(illustration, ctx));

    // ── Location ── (#2418): Position and Rotation through the shared rows.
    let look = super::placed_rows::RowLook {
        muted,
        input_bg: palette.input_bg,
    };
    col = super::placed_rows::push_location_section(col, ctx, look);

    // The picker works on the net the project netlist puts this label on (a
    // child sheet's local `LOCAL` is `/LOCAL`), in the table of the project
    // the label's sheet belongs to.
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
    col.push(
        ctx.collapsible(
            "sel_props",
            ids::PANELS_PLACED_PROPERTIES,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                let name_label = if port {
                    ids::PROPERTIES_PORT_NAME
                } else {
                    ids::PROPERTIES_NET_NAME
                };
                c = c.push(form_edit_row(
                    t!(i18n, name_label),
                    &label_text,
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
                c = c.push(font_family_row(
                    t!(i18n, ids::PROPERTIES_FONT),
                    label_font_family.as_deref(),
                    muted,
                    move |family| {
                        PanelMsg::EditTextFontFamily(signex_engine::TextTarget::Label(id), family)
                    },
                    i18n,
                ));
                c = c.push(font_size_row(
                    i18n,
                    text_size_pt,
                    move |pt| PanelMsg::EditLabelFontSizePt(id, pt),
                    st,
                ));
                if port {
                    return c;
                }
                // Justification is a row (mockup row 44): its name in the
                // label column at the grid's top, the 3x3 dot grid with the
                // word that names its anchor beside it.
                let anchor = t!(i18n, crate::panels::label_anchor(rotation_deg, justify_h));
                let grid = row![
                    justification_grid(id, rotation_deg, justify_h),
                    text(anchor).size(TS.caption).color(muted),
                ]
                .spacing(SP.m)
                .align_y(iced::Alignment::Center)
                .boxed();
                c.push(labelled_row_top(
                    t!(i18n, ids::PANELS_PROPERTIES_JUSTIFICATION),
                    grid,
                ))
            },
        )
        .boxed(),
    )
}

/// The Font Size row: the size list with the B / I / U / T buttons beside it,
/// as one row. `on_pick` builds the message for a size in points.
pub(super) fn font_size_row<'a>(
    i18n: &signex_i18n::I18n,
    current_pt: u32,
    on_pick: impl Fn(u32) -> PanelMsg + 'a,
    st: PanelStyle,
) -> Element<'a, PanelMsg> {
    let PanelPalette {
        input_bg,
        input_bdr,
        ..
    } = st.palette;
    let sizes: Vec<String> = FONT_SIZES_PT.iter().map(u32::to_string).collect();
    let size_list =
        iced::widget::pick_list(Some(current_pt.to_string()), sizes, |s: &String| s.clone())
            .graphite()
            .on_select(move |s: String| on_pick(s.parse().unwrap_or(current_pt)))
            .padding([SP.xs, SP.s])
            .width(Length::FillPortion(2));
    container(
        row![
            property_label(t!(i18n, ids::PROPERTIES_FONT_SIZE), st.muted),
            size_list,
            container(font_style_row(st.muted, st.primary, input_bg, input_bdr))
                .width(Length::FillPortion(3)),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}
