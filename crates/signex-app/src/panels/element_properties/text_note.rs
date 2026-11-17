//! Properties of a selected text note.

use super::super::*;
use super::font_row::font_family_row;
use super::rows::{PanelStyle, info_row, justify_h_row, justify_v_row};
use super::symbol_field::push_text_rows;
use iced::Widget;
use signex_i18n::{ids, t};

pub(super) fn push_text_note_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    uuid: Option<uuid::Uuid>,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let PanelStyle { muted, palette, .. } = st;
    let PanelPalette { input_bg, .. } = palette;
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
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
                ids::PROPERTIES_SECTION_BASIC,
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(form_edit_row(
                        t!(i18n, ids::PROPERTIES_TEXT),
                        &note_text,
                        muted,
                        move |s| PanelMsg::EditTextNoteText(id, s),
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
                        note_font_family.as_deref(),
                        muted,
                        move |family| {
                            PanelMsg::EditTextFontFamily(
                                signex_engine::TextTarget::TextNote(id),
                                family,
                            )
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
