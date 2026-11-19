//! Properties surface of a sheet symbol (#46): the line-art illustration,
//! then Location (Position, Size) → Properties (Name, File with Browse…,
//! Open Sheet) → Style (Border, Fill, Line Width, Reset to Default) →
//! Entries (read-only, name and direction). A symbol whose file is not in
//! the project folder gets an error banner offering Create Sheet… and
//! Browse…, and its File field an error outline.

use super::super::*;
use super::illustrations::sheet_symbol::{
    EntryDirection, SHEET_BORDER, SHEET_ENTRIES, SHEET_FILE, SHEET_FILL, SHEET_LINE_WIDTH,
    SHEET_NAME, entry_direction, sheet_symbol_illustration,
};
use super::placed_rows::{RowLook, push_location_section};
use crate::panels::form_look;
use crate::panels::SelectionFormMsg;
use crate::panels::multi_selection::{
    control_row, counted_section, hover_linked, illustration_view,
};
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{MessageId, UserText, ids, t};
use signex_types::schematic::ChildSheet;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The catalog command "Open Sheet" runs: what double-clicking does.
const OPEN_SHEET_COMMAND: &str = "open_child_sheet";
/// The catalog command behind Create Sheet…: it opens the Create Sheet
/// dialog for a symbol whose file is missing.
const CREATE_SHEET_COMMAND: &str = "create_sheet_from_sheet_symbol";

/// Properties section for a single hierarchical child sheet.
pub(in crate::panels) fn view_child_sheet_properties<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    _primary: Color,
    _border_c: Color,
) -> Element<'a, PanelMsg> {
    let Some(sheet) = ctx.selected_child_sheet.as_ref() else {
        return Column::<Element<'a, PanelMsg>>::new()
            .width(Length::Fill)
            .boxed();
    };
    let palette = PanelPalette::from_tokens(&ctx.tokens);
    let missing = sheet_file_missing(&ctx.projects, &sheet.filename);
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);
    if missing {
        col = col.push(missing_banner(ctx, sheet));
    }
    col = col.push(illustration_view(
        ctx,
        sheet_symbol_illustration(sheet, &ctx.i18n),
    ));
    col = col.push(
        container(
            text(t!(ctx.i18n, ids::PROPERTIES_SHEET_ENTRIES_NOTE))
                .size(TS.caption)
                .color(muted),
        )
        .padding([0.0, SP.l])
        .width(Length::Fill)
        .boxed(),
    );
    let look = RowLook {
        muted,
        input_bg: palette.input_bg,
    };
    col = push_location_section(col, ctx, look);
    col = col.push(properties_section(ctx, sheet, missing, muted));
    col = col.push(style_section(ctx, sheet, palette));
    col = col.push(entries_section(ctx, sheet, muted));
    col.boxed()
}

/// The basename of a file reference, folded for comparison.
fn file_key(reference: &str) -> String {
    reference
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(reference)
        .to_lowercase()
}

/// Whether the project tree marks the file `reference` names as a missing
/// child sheet. The tree's facts come from the off-thread file probe, so
/// the view reads no disk; until a probe answers, a file counts as present.
pub(in crate::panels) fn sheet_file_missing(
    projects: &[crate::panels::ProjectPanelInfo],
    reference: &str,
) -> bool {
    projects
        .iter()
        .any(|p| rows_mark_missing(&p.sheets, reference))
}

/// Whether `rows` (or their children) hold a missing child reference to
/// the file `reference` names, matched by file name.
fn rows_mark_missing(rows: &[crate::panels::SheetInfo], reference: &str) -> bool {
    fn walk(rows: &[crate::panels::SheetInfo], key: &str) -> bool {
        rows.iter().any(|row| {
            (row.is_child_reference && row.missing && file_key(&row.filename) == key)
                || walk(&row.children, key)
        })
    }
    let key = file_key(reference);
    !key.is_empty() && walk(rows, &key)
}

/// The error banner of a sheet symbol with no file: what is missing, and
/// Create Sheet… (the Create Sheet dialog) or Browse… for an existing file.
fn missing_banner<'a>(ctx: &'a PanelContext, sheet: &ChildSheet) -> Element<'a, PanelMsg> {
    let i18n = &ctx.i18n;
    let resolved = crate::styles::tokens::resolve(&ctx.tokens);
    let error = crate::styles::ti(ctx.tokens.error);
    let ink = crate::styles::ti(ctx.tokens.text);
    let (err_soft, id) = (resolved.err_soft, sheet.uuid);
    let actions = row![
        button(text(t!(i18n, ids::PROPERTIES_SHEET_CREATE)).size(TS.caption))
            .padding([SP.xs, SP.m])
            .on_press(PanelMsg::MessageLinkPressed(CREATE_SHEET_COMMAND))
            .style(form_look::option_tile(true)),
        button(text(t!(i18n, ids::PROPERTIES_SHEET_BROWSE)).size(TS.caption))
            .padding([SP.xs, SP.m])
            .on_press(PanelMsg::SelectionForm(
                SelectionFormMsg::SheetFileBrowseClicked(id)
            ))
            .style(form_look::ghost_button),
    ]
    .spacing(SP.s);
    let body = Column::new()
        .spacing(SP.xs)
        .width(Length::Fill)
        .push(
            text(t!(i18n, ids::PROPERTIES_SHEET_MISSING_TITLE))
                .size(TS.body)
                .color(ink)
                .boxed(),
        )
        .push(
            text(t!(
                i18n,
                ids::PROPERTIES_SHEET_MISSING_DETAIL,
                "file" => sheet.filename.clone()
            ))
            .size(TS.caption)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::Word)
            .boxed(),
        )
        .push(actions.boxed());
    container(
        container(body)
            .padding(SP.m)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(err_soft)),
                border: Border {
                    width: 1.0,
                    color: error,
                    radius: crate::panels::graphite::PROPERTY_FIELD_RADIUS.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// Name, File (with Browse…) and the Open Sheet action.
fn properties_section<'a>(
    ctx: &'a PanelContext,
    sheet: &ChildSheet,
    missing: bool,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let id = sheet.uuid;
    let name = buffered_or(&ctx.child_sheet_name_buf, id, &sheet.name);
    let filename = buffered_or(&ctx.child_sheet_file_buf, id, &sheet.filename);
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let error = crate::styles::ti(ctx.tokens.error);
    ctx.collapsible(
        "sel_child_sheet_props",
        ids::PANELS_PLACED_PROPERTIES,
        &ctx.collapsed_sections,
        move || {
            let name_input = text_box(name, None, move |s| PanelMsg::ChildSheetNameTyping(id, s))
                .on_submit(PanelMsg::CommitChildSheetName(id));
            let file_input = text_box(filename, missing.then_some(error), move |s| {
                PanelMsg::ChildSheetFileTyping(id, s)
            })
            .on_submit(PanelMsg::CommitChildSheetFile(id));
            let browse = button(text(t!(i18n, ids::PROPERTIES_SHEET_BROWSE)).size(TS.caption))
                .padding([SP.s, SP.m])
                .on_press(PanelMsg::SelectionForm(
                    SelectionFormMsg::SheetFileBrowseClicked(id),
                ))
                .style(form_look::ghost_button);
            let file = row![file_input, browse]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center)
                .boxed();
            let open = button(text(t!(i18n, ids::PROPERTIES_SHEET_OPEN)).size(TS.caption))
                .padding([SP.s, SP.m])
                .on_press(PanelMsg::MessageLinkPressed(OPEN_SHEET_COMMAND))
                .style(form_look::ghost_button);
            let name_label = t!(i18n, ids::PANELS_CHILD_SHEET_NAME);
            let file_label = t!(i18n, ids::PANELS_CHILD_SHEET_FILE);
            Column::new()
                .spacing(0)
                .width(Length::Fill)
                .push(hover_linked(
                    ctx,
                    SHEET_NAME,
                    control_row(name_label, name_input.boxed(), muted),
                ))
                .push(hover_linked(
                    ctx,
                    SHEET_FILE,
                    control_row(file_label, file, muted),
                ))
                .push(control_row(
                    signex_i18n::UserText::new(String::new()),
                    open.boxed(),
                    muted,
                ))
        },
    )
    .boxed()
}

/// A form text box; `error` draws its outline in the error colour.
fn text_box<'a>(
    value: String,
    error: Option<Color>,
    on_input: impl Fn(String) -> PanelMsg + 'a,
) -> iced::widget::TextInput<'a, PanelMsg> {
    signex_widgets::text_input(UserText::new(String::new()), value)
        .size(TS.body)
        .padding(SP.s)
        .width(Length::Fill)
        .on_input(on_input)
        .style(move |theme: &Theme, status| {
            let mut style = form_look::text_input_style(theme, status);
            if let Some(error) = error {
                style.border.color = error;
            }
            style
        })
}

/// Border, Fill, Line Width and Reset to Default.
fn style_section<'a>(
    ctx: &'a PanelContext,
    sheet: &ChildSheet,
    palette: PanelPalette,
) -> Element<'a, PanelMsg> {
    let id = sheet.uuid;
    let (stroke_color, fill_color) = (sheet.stroke_color, sheet.fill_color);
    let stroke_width = sheet.stroke_width;
    let pickers = (
        ctx.child_sheet_border_picker_open,
        ctx.child_sheet_border_advanced_open,
        ctx.child_sheet_fill_picker_open,
        ctx.child_sheet_fill_advanced_open,
    );
    let stroke_width_buf = ctx.child_sheet_stroke_width_buf.clone();
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    ctx.collapsible(
        "sel_child_sheet_style",
        ids::PROPERTIES_SECTION_STYLE,
        &ctx.collapsed_sections,
        move || {
            let (border_open, border_advanced, fill_open, fill_advanced) = pickers;
            let border = child_sheet_color_row(
                id,
                stroke_color,
                border_open,
                border_advanced,
                palette,
                /* is_border */ true,
                i18n,
            );
            let fill = child_sheet_color_row(
                id,
                fill_color,
                fill_open,
                fill_advanced,
                palette,
                /* is_border */ false,
                i18n,
            );
            let reset = container(
                button(text(t!(i18n, ids::PANELS_PROPERTIES_RESET_STYLE)).size(TS.caption))
                    .padding([SP.s, SP.m])
                    .on_press(PanelMsg::ResetChildSheetStyle(id))
                    .style(form_look::option_tile(false)),
            )
            .padding([6.0, SP.m])
            .width(Length::Fill)
            .boxed();
            Column::new()
                .spacing(0)
                .width(Length::Fill)
                .push(hover_linked(ctx, SHEET_BORDER, border))
                .push(hover_linked(ctx, SHEET_FILL, fill))
                .push(hover_linked(
                    ctx,
                    SHEET_LINE_WIDTH,
                    child_sheet_stroke_width_row(
                        id,
                        stroke_width,
                        stroke_width_buf,
                        palette.muted,
                        i18n,
                    ),
                ))
                .push(reset)
        },
    )
    .boxed()
}

/// The catalog name of an entry direction; a word the form has no name for
/// is shown as stored.
fn direction_label(word: &str, i18n: &signex_i18n::I18n) -> signex_widgets::Label<'static> {
    let id: MessageId = match entry_direction(word) {
        EntryDirection::Input => ids::PROPERTIES_ENTRY_INPUT,
        EntryDirection::Output => ids::PROPERTIES_ENTRY_OUTPUT,
        EntryDirection::Bidirectional => ids::PROPERTIES_ENTRY_BIDIRECTIONAL,
        EntryDirection::TriState => ids::PROPERTIES_ENTRY_TRI_STATE,
        EntryDirection::Passive => ids::PROPERTIES_ENTRY_PASSIVE,
        EntryDirection::Unspecified => ids::PROPERTIES_ENTRY_UNSPECIFIED,
        EntryDirection::Other => return UserText::new(word.to_owned()).into(),
    };
    t!(i18n, id).into()
}

/// The read-only Entries table: name and direction of each entry, so the
/// sheet's interface shows without opening it.
fn entries_section<'a>(
    ctx: &'a PanelContext,
    sheet: &ChildSheet,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let ink = crate::styles::ti(ctx.tokens.text);
    let rows: Vec<(String, signex_widgets::Label<'static>)> = sheet
        .pins
        .iter()
        .map(|pin| (pin.name.clone(), direction_label(&pin.direction, i18n)))
        .collect();
    counted_section(
        ctx,
        "sel_child_sheet_entries",
        ids::PROPERTIES_SECTION_ENTRIES,
        Some(i18n.count(rows.len() as i64)),
        move || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            if rows.is_empty() {
                c = c.push(
                    container(
                        text(t!(i18n, ids::PROPERTIES_SHEET_NO_ENTRIES))
                            .size(TS.caption)
                            .color(muted),
                    )
                    .padding([SP.s, SP.l])
                    .boxed(),
                );
            }
            for (name, direction) in rows {
                let line = row![
                    text(UserText::new(name))
                        .size(TS.body)
                        .font(iced::Font::MONOSPACE)
                        .color(ink)
                        .width(Length::Fill),
                    text(direction).size(TS.caption).color(muted),
                ]
                .spacing(SP.m)
                .align_y(iced::Alignment::Center);
                c = c.push(hover_linked(
                    ctx,
                    SHEET_ENTRIES,
                    container(line)
                        .padding([SP.xs, SP.l])
                        .width(Length::Fill)
                        .boxed(),
                ));
            }
            c
        },
    )
}

/// One swatch row in the child-sheet Style section.
///
/// Click flow:
///   1. Click swatch → expands an inline preset palette panel below
///      the row (full panel width, like the canvas-font popup) with
///      a 12-colour grid plus "Custom…" and (when an override is
///      active) "Reset to Default".
///   2. Click the swatch again to collapse, or click "Custom…" to
///      switch to the iced_aw HSV / RGB ColorPicker overlay.
///
/// Both the palette pick and the advanced-picker submit reuse the
/// same `EditChildSheet*Color` message so engine command + undo/redo
/// round-trip is identical for both paths.
fn child_sheet_color_row<'a>(
    sheet_id: uuid::Uuid,
    current: Option<signex_types::schematic::StrokeColor>,
    show_picker: bool,
    show_advanced: bool,
    palette: PanelPalette,
    is_border: bool,
    i18n: &signex_i18n::I18n,
) -> Element<'a, PanelMsg> {
    use std::rc::Rc;

    let PanelPalette {
        muted,
        border: border_c,
        ..
    } = palette;

    // StrokeColor → [u8; 4] RGBA so the generic widget stays agnostic
    // of the schematic colour type.
    let current_rgba = current.map(|c| [c.r, c.g, c.b, c.a]);

    let on_toggle = if is_border {
        PanelMsg::ToggleChildSheetBorderPicker(sheet_id)
    } else {
        PanelMsg::ToggleChildSheetFillPicker(sheet_id)
    };

    // `from_rgba8` reproduces the exact `iced::Color` the old preset path
    // emitted; the handler re-quantises to `StrokeColor` regardless.
    let on_pick: Rc<dyn Fn([u8; 4]) -> PanelMsg + 'static> = Rc::new(move |rgba: [u8; 4]| {
        let color = iced::Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3] as f32 / 255.0);
        if is_border {
            PanelMsg::EditChildSheetBorderColor(sheet_id, color)
        } else {
            PanelMsg::EditChildSheetFillColor(sheet_id, color)
        }
    });

    // Reset-to-default only shows when an override is active.
    let on_clear = current
        .is_some()
        .then_some(PanelMsg::ResetChildSheetStyle(sheet_id));

    let label = if is_border {
        ids::PROPERTIES_SHEET_BORDER
    } else {
        ids::PROPERTIES_SHEET_FILL
    };
    color_field(
        ColorFieldProps {
            label: t!(i18n, label).into(),
            current: current_rgba,
            none_label: t!(i18n, ids::PROPERTIES_COLOR_DEFAULT).into(),
            show_palette: show_picker,
            show_advanced,
            muted,
            border_c,
            on_toggle,
            on_advanced: PanelMsg::OpenChildSheetAdvancedPicker(sheet_id, is_border),
            on_cancel: PanelMsg::CancelChildSheetColorPicker,
            on_pick,
            on_clear,
        },
        i18n,
    )
}

/// The text the Line Width box shows: the draft while typed; empty for the
/// default width, so the 0.1524 placeholder shows.
fn line_width_text(stored: f64, buffered: Option<String>) -> String {
    match buffered {
        Some(s) => s,
        None if !signex_types::schematic::is_set_width(stored) => String::new(),
        None => format!("{stored:.4}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string(),
    }
}

/// Line Width (mm) with the 0.1524 placeholder; Enter on an empty box sets
/// the default width back.
fn child_sheet_stroke_width_row<'a>(
    sheet_id: uuid::Uuid,
    stored_value: f64,
    buffered: Option<String>,
    muted: Color,
    i18n: &signex_i18n::I18n,
) -> Element<'a, PanelMsg> {
    let placeholder = t!(i18n, ids::PLACEHOLDER_CHILD_SHEET_LINE_WIDTH);
    let input = signex_widgets::text_input(placeholder, line_width_text(stored_value, buffered))
        .size(TS.body)
        .padding(SP.s)
        .width(Length::Fill)
        .on_input(move |s| PanelMsg::ChildSheetStrokeWidthTyping(sheet_id, s))
        .on_submit(PanelMsg::CommitChildSheetStrokeWidth(sheet_id))
        .style(form_look::text_input_style);
    let control = row![
        input,
        text(t!(i18n, ids::PROPERTIES_UNIT_MM))
            .size(TS.caption)
            .color(muted),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center)
    .boxed();
    control_row(t!(i18n, ids::PROPERTIES_SHEET_LINE_WIDTH), control, muted)
}

/// The in-progress text for sheet `id`, else the live value.
fn buffered_or(buf: &Option<(uuid::Uuid, String)>, id: uuid::Uuid, live: &str) -> String {
    match buf {
        Some((owner, text)) if *owner == id => text.clone(),
        _ => live.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panels::SheetInfo;

    fn info(filename: &str, missing: bool, child: bool) -> SheetInfo {
        SheetInfo {
            name: filename.into(),
            filename: filename.into(),
            sym_count: 0,
            wire_count: 0,
            label_count: 0,
            is_open: false,
            is_dirty: false,
            is_active: false,
            missing,
            is_child_reference: child,
            in_project_list: false,
            unreadable: false,
            children: Vec::new(),
        }
    }

    fn tree(child: SheetInfo) -> Vec<SheetInfo> {
        let mut root = info("main.snxsch", false, false);
        root.children = vec![child];
        vec![root]
    }

    /// #46: the banner shows only for a child reference the probe found
    /// absent, matched by its file name.
    #[test]
    fn the_missing_banner_shows_only_when_the_file_is_absent() {
        let absent = tree(info("debug.snxsch", true, true));
        assert!(rows_mark_missing(&absent, "debug.snxsch"));
        assert!(rows_mark_missing(&absent, "sub/Debug.snxsch"));
        assert!(!rows_mark_missing(&absent, "power.snxsch"));
        assert!(!rows_mark_missing(&absent, ""));
        let present = tree(info("debug.snxsch", false, true));
        assert!(!rows_mark_missing(&present, "debug.snxsch"));
        let project_page = tree(info("debug.snxsch", true, false));
        assert!(
            !rows_mark_missing(&project_page, "debug.snxsch"),
            "a missing project page is not a sheet symbol's file"
        );
    }

    #[test]
    fn the_default_line_width_shows_the_placeholder() {
        assert_eq!(line_width_text(0.0, None), "");
        assert_eq!(line_width_text(0.25, None), "0.25");
        assert_eq!(line_width_text(0.25, Some(String::new())), "");
    }
}
