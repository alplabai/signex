//! A rich text note's own rows in Properties (#1114, mockup row 203): Edit…
//! beside its Text, and the Frame (None / Border / Filled, Border width with
//! Auto, Padding) and Size (Fit text / Fixed width / Fixed box, Width,
//! Height) sections after the row 47 sections. A pick is one edit; a typed
//! length commits on Enter. A locked note's rows are read-only.

use super::super::*;
use super::rows::{labelled_row, sunken_value, unit_input, unit_suffix};
use super::text_note::segmented_row;
use crate::panels::{NoteLayout, NoteLength, PlacedEditMsg, PlacedField};
use signex_i18n::{ids, t};
use signex_types::measure::length_field_text;
use signex_types::schematic::{NoteFrame, NoteSize, RichNoteLayout, SelectedItem};
use signex_widgets::Label;

/// Edit… beside the Text box: the note's in-place editor.
pub(super) fn edit_row<'a>(ctx: &'a PanelContext, item: SelectedItem) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let edit = signex_widgets::button(signex_widgets::text(t!(i18n, ids::PROPERTIES_NOTE_EDIT)))
        .padding([2.0, 8.0])
        .style(crate::styles::ghost_button(&ctx.tokens))
        .on_press_maybe(ctx.placed.locked.is_none().then_some(PanelMsg::PlacedEdit(
            PlacedEditMsg::RichNoteEditRequested { item },
        )));
    labelled_row(
        Label::from(t!(i18n, ids::PROPERTIES_NOTE_SOURCE)),
        edit.boxed(),
    )
}

/// The Frame and Size sections of the rich note `item`.
pub(super) fn push_sections<'a>(
    col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    item: SelectedItem,
    st: super::rows::PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let Some(layout) = ctx.placed.note_rich else {
        return col;
    };
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let read_only = ctx.placed.locked.is_some();
    let picked = move |layout: RichNoteLayout| {
        PanelMsg::PlacedEdit(PlacedEditMsg::NoteLayoutPicked {
            item,
            layout: NoteLayout::Rich(layout),
        })
    };
    let frame = ctx.collapsible(
        "sel_note_frame",
        ids::PROPERTIES_NOTE_FRAME_SECTION,
        &ctx.collapsed_sections,
        move || {
            let frames = [
                (NoteFrame::None, t!(i18n, ids::PROPERTIES_NOTE_FRAME_NONE)),
                (
                    NoteFrame::Border,
                    t!(i18n, ids::PROPERTIES_NOTE_FRAME_BORDER),
                ),
                (
                    NoteFrame::Filled,
                    t!(i18n, ids::PROPERTIES_NOTE_FRAME_FILLED),
                ),
            ];
            Column::new()
                .spacing(0)
                .width(Length::Fill)
                .push(segmented_row(
                    t!(i18n, ids::PROPERTIES_NOTE_FRAME),
                    frames,
                    layout.frame,
                    move |frame| picked(RichNoteLayout { frame, ..layout }),
                    st,
                    read_only,
                ))
                .push(length_row(ctx, item, NoteLength::Border, layout))
                .push(length_row(ctx, item, NoteLength::Padding, layout))
        },
    );
    let size = ctx.collapsible(
        "sel_note_size",
        ids::PROPERTIES_NOTE_SIZE_SECTION,
        &ctx.collapsed_sections,
        move || {
            let sizes = [
                (NoteSize::FitText, t!(i18n, ids::PROPERTIES_NOTE_SIZE_FIT)),
                (
                    NoteSize::FixedWidth,
                    t!(i18n, ids::PROPERTIES_NOTE_SIZE_WIDTH),
                ),
                (NoteSize::FixedBox, t!(i18n, ids::PROPERTIES_NOTE_SIZE_BOX)),
            ];
            let mut c = Column::new()
                .spacing(0)
                .width(Length::Fill)
                .push(segmented_row(
                    t!(i18n, ids::PROPERTIES_NOTE_SIZE),
                    sizes,
                    layout.size,
                    move |size| picked(RichNoteLayout { size, ..layout }),
                    st,
                    read_only,
                ));
            if layout.size.has_width() {
                c = c.push(length_row(ctx, item, NoteLength::Width, layout));
            }
            if layout.size == NoteSize::FixedBox {
                c = c.push(length_row(ctx, item, NoteLength::Height, layout));
            }
            c
        },
    );
    col.push(frame.boxed()).push(size.boxed())
}

/// The value a length box shows: Auto for an unset border, a Fixed box's
/// default height when none was set.
fn shown(length: NoteLength, layout: RichNoteLayout, ctx: &PanelContext) -> String {
    let unit = ctx.unit;
    match length {
        NoteLength::Border => layout.border_mm.map_or_else(
            || {
                ctx.i18n
                    .message_plain(ids::PROPERTIES_NOTE_BORDER_AUTO, &[])
            },
            |mm| length_field_text(mm, unit),
        ),
        NoteLength::Padding => length_field_text(layout.padding_mm, unit),
        NoteLength::Width => length_field_text(layout.box_width_mm(), unit),
        NoteLength::Height => {
            length_field_text(layout.box_height_mm().unwrap_or(layout.height_mm), unit)
        }
    }
}

/// One length row; a locked note shows the read-only value.
fn length_row<'a>(
    ctx: &'a PanelContext,
    item: SelectedItem,
    length: NoteLength,
    layout: RichNoteLayout,
) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let label = match length {
        NoteLength::Border => ids::PROPERTIES_NOTE_BORDER,
        NoteLength::Padding => ids::PROPERTIES_NOTE_PADDING,
        NoteLength::Width => ids::PROPERTIES_NOTE_WIDTH,
        NoteLength::Height => ids::PROPERTIES_NOTE_HEIGHT,
    };
    let unit = Label::from(unit_suffix(i18n, ctx.unit));
    let value = shown(length, layout, ctx);
    let control = if ctx.placed.locked.is_some() {
        sunken_value(None, value, Some(unit))
    } else {
        let field = PlacedField::NoteLength(length);
        let value = ctx.placed.text(item, &field, value);
        let submitted = PanelMsg::PlacedEdit(PlacedEditMsg::Submitted {
            item,
            field: field.clone(),
        });
        let typed = move |text| {
            PanelMsg::PlacedEdit(PlacedEditMsg::Typed {
                item,
                field: field.clone(),
                text,
            })
        };
        unit_input(None, value, Some(unit), typed, submitted)
    };
    labelled_row(Label::from(t!(i18n, label)), control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Widget as _;

    fn ctx_with(layout: RichNoteLayout) -> PanelContext {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_view.panel_ctx.clone();
        ctx.placed.note_rich = Some(layout);
        ctx
    }

    fn style(ctx: &PanelContext) -> super::super::rows::PanelStyle {
        super::super::rows::PanelStyle {
            muted: Color::BLACK,
            primary: Color::BLACK,
            border_c: Color::BLACK,
            palette: crate::panels::palette::PanelPalette::from_tokens(&ctx.tokens),
        }
    }

    /// Auto shows for an unset border; a Fixed box shows its default height.
    #[test]
    fn the_length_boxes_show_auto_and_the_defaults() {
        let layout = RichNoteLayout {
            size: NoteSize::FixedBox,
            ..RichNoteLayout::default()
        };
        let ctx = ctx_with(layout);
        assert_eq!(shown(NoteLength::Border, layout, &ctx), "Auto");
        let width = shown(NoteLength::Width, layout, &ctx);
        assert!(width.starts_with("50.8"), "{width}");
        let height = shown(NoteLength::Height, layout, &ctx);
        assert!(height.starts_with("25.4"), "{height}");
    }

    /// Mockup row 203: the Size section shows Width for Fixed width and
    /// Width and Height for Fixed box; the view lays out at the panel width.
    #[test]
    fn the_size_section_shows_the_boxes_its_size_has() {
        let item = SelectedItem::new(
            uuid::Uuid::from_u128(1),
            signex_types::schematic::SelectedKind::TextNote,
        );
        let height = |size| {
            let ctx = ctx_with(RichNoteLayout {
                size,
                ..RichNoteLayout::default()
            });
            let col: Element<'_, PanelMsg> =
                push_sections(Column::new(), &ctx, item, style(&ctx)).boxed();
            crate::layout_probe::root_size(col, iced::Size::new(300.0, 2000.0)).height
        };
        let fit = height(NoteSize::FitText);
        let width = height(NoteSize::FixedWidth);
        let boxed = height(NoteSize::FixedBox);
        assert!(width > fit, "Fixed width adds Width: {width} > {fit}");
        assert!(boxed > width, "Fixed box adds Height: {boxed} > {width}");
    }
}
