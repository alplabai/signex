//! Properties of a selected text note.

use super::super::*;
use super::font_row::font_family_row;
use super::rows::{PanelStyle, info_row, justify_h_row, justify_v_row};
use iced::Widget;

pub(super) fn push_text_note_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    uuid: Option<uuid::Uuid>,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let PanelStyle { muted, palette, .. } = st;
    let PanelPalette { input_bg, .. } = palette;
    let get = |key: &str| info_row(&ctx.selection_info, key);
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
    col
}
