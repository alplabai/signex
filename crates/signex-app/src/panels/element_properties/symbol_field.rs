//! Properties of a selected symbol Reference or Value field.

use super::super::*;
use super::font_row::font_family_row;
use super::rows::{PanelStyle, info_row, justify_h_row, justify_v_row};
use iced::Widget;
use signex_i18n::{ids, t};

pub(super) fn push_symbol_field_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    uuid: Option<uuid::Uuid>,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let PanelStyle { muted, palette, .. } = st;
    let PanelPalette { input_bg, .. } = palette;
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let selected_kind = ctx.selected_kind;
    let get = |key: &str| info_row(&ctx.selection_info, key);
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
                ids::PROPERTIES_SECTION_BASIC,
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    let field = if is_reference {
                        ids::PROPERTIES_FIELD_REFERENCE
                    } else {
                        ids::PROPERTIES_FIELD_VALUE
                    };
                    c = c.push(form_input_row(
                        t!(i18n, ids::PROPERTIES_FIELD),
                        &i18n.message_plain(field, &[]),
                        muted,
                        input_bg,
                    ));
                    c = c.push(form_edit_row(
                        t!(i18n, ids::PROPERTIES_TEXT),
                        &text_value,
                        muted,
                        move |s| {
                            if is_reference {
                                PanelMsg::EditSymbolDesignator(id, s)
                            } else {
                                PanelMsg::EditSymbolValue(id, s)
                            }
                        },
                    ));
                    c = c.push(form_input_row(
                        t!(i18n, ids::PROPERTIES_VISIBLE),
                        &visible,
                        muted,
                        input_bg,
                    ));
                    c = c.push(form_input_row(
                        t!(i18n, ids::PROPERTIES_FIELDS_AUTOPLACED),
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
                ids::PROPERTIES_SECTION_TEXT,
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = push_text_rows(
                        c,
                        i18n,
                        [&position, &rotation, &justify_h, &justify_v, &text_size],
                        muted,
                        input_bg,
                    );
                    c = c.push(font_family_row(
                        t!(i18n, ids::PROPERTIES_FONT),
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
                        i18n,
                    ));
                    c
                },
            )
            .boxed(),
        );
    }
    col
}

/// The read-only Position, Rotation, Justify H, Justify V and Text Size rows
/// of a text-bearing selection, `values` in that order.
pub(super) fn push_text_rows<'a>(
    mut c: Column<Element<'a, PanelMsg>>,
    i18n: &signex_i18n::I18n,
    values: [&str; 5],
    muted: Color,
    input_bg: Color,
) -> Column<Element<'a, PanelMsg>> {
    let labels = [
        ids::PROPERTIES_POSITION,
        ids::PROPERTIES_ROTATION,
        ids::PROPERTIES_JUSTIFY_H,
        ids::PROPERTIES_JUSTIFY_V,
        ids::PROPERTIES_TEXT_SIZE,
    ];
    for (label, value) in labels.into_iter().zip(values) {
        c = c.push(form_input_row(t!(i18n, label), value, muted, input_bg));
    }
    c
}
