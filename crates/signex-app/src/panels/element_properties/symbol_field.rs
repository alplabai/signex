//! Properties of a selected symbol Reference or Value field.

use super::super::*;
use super::font_row::font_family_row;
use super::rows::{PanelStyle, info_row, justify_h_row, justify_v_row};
use iced::Widget;

pub(super) fn push_symbol_field_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    uuid: Option<uuid::Uuid>,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let PanelStyle { muted, palette, .. } = st;
    let PanelPalette { input_bg, .. } = palette;
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
    col
}
