//! Properties surface of a sheet symbol (mockup row 46): the line-art illustration,
//! then Location (Position, Size) → Properties (Name, File with Browse…,
//! Open Sheet) → Style (Border, Fill, Line Width, Reset to Default) →
//! Entries (read-only, name and direction). A symbol whose file is not in
//! the project folder gets an error banner offering Create Sheet… and
//! Browse…, its File field an error outline, and Open Sheet is disabled with
//! the reason under it.

use super::super::*;
use super::illustrations::sheet_symbol::{
    EntryDirection, SHEET_BORDER, SHEET_ENTRIES, SHEET_FILE, SHEET_FILL, SHEET_LINE_WIDTH,
    SHEET_NAME, entry_direction, sheet_symbol_illustration,
};
use super::placed_rows::{RowLook, push_location_section};
use crate::panels::SelectionFormMsg;
use crate::panels::form_look;
use crate::panels::multi_selection::{
    control_row, counted_section, hover_linked, illustration_view, inset_caption,
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
    let missing = (ctx.selected_child_sheet_file.as_deref())
        .is_some_and(|file| sheet_file_missing(&ctx.projects, file));
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);
    // The multi-channel view of the symbol (#1279): its caption, its stacked
    // illustration and its Repeat, Channel naming and Entries sections.
    let repeat_view = ctx
        .compiled_design
        .sheet_symbol
        .as_ref()
        .filter(|view| view.sheet_symbol == sheet.uuid);
    if missing {
        col = col.push(missing_banner(ctx, sheet));
    }
    if let Some(view) = repeat_view {
        col = col.push(super::child_sheet_repeat::caption(ctx, view));
    }
    // Only a repeated sheet draws as a stack (mockup row 151); one sheet
    // shows its entries' direction shapes on their edges (mockup row 46).
    let repeated_view = repeat_view.filter(|view| shows_channels(view));
    col = col.push(match repeated_view {
        Some(view) => super::child_sheet_repeat::illustration(ctx, view),
        None => illustration_view(ctx, sheet_symbol_illustration(sheet, &ctx.i18n)),
    });
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
    col = col.push(properties_section(
        ctx,
        sheet,
        missing,
        repeated_view.is_some(),
        muted,
    ));
    col = col.push(style_section(ctx, sheet, palette));
    col = col.push(match repeat_view {
        // Repeat and Channel naming (#1279), then the per-channel Entries
        // (mockup row 151).
        Some(view) if shows_channels(view) => {
            let entries = super::child_sheet_repeat::entries(ctx, view);
            super::child_sheet_repeat::sections(ctx, view, entries)
        }
        // One sheet keeps the row 46 order, Style then Entries, so the
        // entries are not pushed under the switched-off Repeat (#2827).
        Some(view) => Column::new()
            .width(Length::Fill)
            .push(entries_section(ctx, sheet, muted))
            .push(super::child_sheet_repeat::sections(
                ctx,
                view,
                Space::new().boxed(),
            ))
            .boxed(),
        None => entries_section(ctx, sheet, muted),
    });
    col.boxed()
}

/// Whether the symbol repeats, so its drawing and Entries are per channel.
fn shows_channels(view: &crate::panels::compiled_design::SheetSymbolRepeatView) -> bool {
    view.repeat.is_some()
}

/// Whether the project tree marks the resolved path `file` as a missing child
/// sheet, compared under each project's case rule. The tree's facts come from
/// the off-thread file probe, so nothing here reads the disk; until a probe
/// answers, a file counts as present.
pub(crate) fn sheet_file_missing(
    projects: &[crate::panels::ProjectPanelInfo],
    file: &std::path::Path,
) -> bool {
    let file = crate::app::project_sheets::lexically_normalize(file);
    projects.iter().any(|p| {
        let key = crate::app::project_sheets::fold_key(&file, p.case_fold);
        rows_mark_missing(&p.sheets, &key)
    })
}

/// Whether `rows` (or their children) hold a missing child reference whose
/// resolved file has the comparison key `key`.
fn rows_mark_missing(rows: &[crate::panels::SheetInfo], key: &str) -> bool {
    !key.is_empty()
        && rows.iter().any(|row| {
            (row.is_child_reference && row.missing && row.file_key == key)
                || rows_mark_missing(&row.children, key)
        })
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
            .on_press(PanelMsg::CommandButtonPressed(CREATE_SHEET_COMMAND))
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

/// What Open Sheet sends; a missing file leaves the button disabled.
fn open_sheet_press(missing: bool) -> Option<PanelMsg> {
    (!missing).then_some(PanelMsg::CommandButtonPressed(OPEN_SHEET_COMMAND))
}

/// Name, File (with Browse…) and the Open Sheet action.
fn properties_section<'a>(
    ctx: &'a PanelContext,
    sheet: &ChildSheet,
    missing: bool,
    repeat: bool,
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
                .on_press_maybe(open_sheet_press(missing))
                .style(form_look::ghost_button);
            let file_label = t!(i18n, ids::PANELS_CHILD_SHEET_FILE);
            // With the multi-channel view the name is the designator, where
            // `Repeat(CH, 1, 4)` repeats the sheet (#1279).
            let name_row = if repeat {
                let label = t!(i18n, ids::PROPERTIES_SHEET_SYMBOL_DESIGNATOR);
                super::child_sheet_repeat::linked(
                    super::child_sheet_repeat::FIELD_DESIGNATOR,
                    control_row(label, name_input.boxed(), muted),
                )
            } else {
                let label = t!(i18n, ids::PANELS_CHILD_SHEET_NAME);
                hover_linked(
                    ctx,
                    SHEET_NAME,
                    control_row(label, name_input.boxed(), muted),
                )
            };
            let rows = Column::new()
                .spacing(0)
                .width(Length::Fill)
                .push(name_row)
                .push(hover_linked(
                    ctx,
                    SHEET_FILE,
                    control_row(file_label, file, muted),
                ))
                .push(control_row(
                    signex_i18n::UserText::new(String::new()),
                    open.boxed(),
                    muted,
                ));
            if missing {
                rows.push(inset_caption(
                    t!(i18n, ids::PROPERTIES_SHEET_OPEN_MISSING),
                    muted,
                ))
            } else {
                rows
            }
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
    let refused_ink = crate::styles::ti(ctx.tokens.error);
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
                /* is_border */ true,
                i18n,
            );
            let fill = child_sheet_color_row(
                id,
                fill_color,
                fill_open,
                fill_advanced,
                /* is_border */ false,
                i18n,
            );
            // Mockup row 46: a small ghost action under the fields.
            let reset = container(
                button(text(t!(i18n, ids::PANELS_PROPERTIES_RESET_STYLE)).size(TS.caption))
                    .padding([SP.xs, SP.m])
                    .on_press(PanelMsg::ResetChildSheetStyle(id))
                    .style(form_look::ghost_button),
            )
            .padding([SP.s, SP.m])
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
                        refused_ink,
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

/// One colour field of the child-sheet Style section (mockup row 46): the
/// swatch, its code or Default and a chevron, opening the preset popover.
/// Custom… switches to the HSV / RGB picker. Both picks send the same
/// `EditChildSheet*Color` message, so undo is identical for both.
fn child_sheet_color_row<'a>(
    sheet_id: uuid::Uuid,
    current: Option<signex_types::schematic::StrokeColor>,
    show_picker: bool,
    show_advanced: bool,
    is_border: bool,
    i18n: &signex_i18n::I18n,
) -> Element<'a, PanelMsg> {
    use crate::panels::wire_style_rows::{PopoverColorRow, WireColorPicker, popover_color_row};
    use std::rc::Rc;

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
    let picker = if show_advanced {
        WireColorPicker::Advanced
    } else if show_picker {
        WireColorPicker::Palette
    } else {
        WireColorPicker::Closed
    };
    popover_color_row(
        PopoverColorRow {
            label: t!(i18n, label),
            current: current_rgba,
            picker,
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
    error: Color,
    i18n: &signex_i18n::I18n,
) -> Element<'a, PanelMsg> {
    let placeholder = t!(i18n, ids::PLACEHOLDER_CHILD_SHEET_LINE_WIDTH);
    let refused = (buffered.as_deref())
        .is_some_and(|typed| crate::numeric_field::typed_line_width(typed).is_none());
    let input = signex_widgets::text_input(placeholder, line_width_text(stored_value, buffered))
        .size(TS.body)
        .padding(SP.s)
        .width(Length::Fill)
        .on_input(move |s| PanelMsg::ChildSheetStrokeWidthTyping(sheet_id, s))
        .on_submit(PanelMsg::CommitChildSheetStrokeWidth(sheet_id))
        .style(form_look::refusable_input_style(refused.then_some(error)));
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
pub(super) mod tests {
    use super::*;
    use crate::panels::SheetInfo;
    use signex_types::schematic::SelectedKind as Kind;

    /// The comparison key of `path`, on a volume that ignores case.
    fn key(path: &str) -> String {
        let path = crate::app::project_sheets::lexically_normalize(std::path::Path::new(path));
        crate::app::project_sheets::fold_key(&path, crate::app::CaseFold::case_only())
    }

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
            file_key: key(filename),
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

    /// Mockup row 46: the banner shows only for a child reference the probe found
    /// absent, matched by its resolved path under the project's case rule.
    #[test]
    fn the_missing_banner_shows_only_when_the_file_is_absent() {
        let absent = tree(info("debug.snxsch", true, true));
        assert!(rows_mark_missing(&absent, &key("debug.snxsch")));
        assert!(rows_mark_missing(&absent, &key("Debug.snxsch")));
        assert!(
            !rows_mark_missing(&absent, &key("sub/Debug.snxsch")),
            "the same name in another folder is another file"
        );
        assert!(!rows_mark_missing(&absent, &key("power.snxsch")));
        assert!(!rows_mark_missing(&absent, ""));
        let present = tree(info("debug.snxsch", false, true));
        assert!(!rows_mark_missing(&present, &key("debug.snxsch")));
        let project_page = tree(info("debug.snxsch", true, false));
        assert!(
            !rows_mark_missing(&project_page, &key("debug.snxsch")),
            "a missing project page is not a sheet symbol's file"
        );
    }

    /// #2847: a healthy `a/debug.snxsch` is not marked because
    /// `b/debug.snxsch` is missing.
    #[test]
    fn a_missing_file_does_not_mark_the_same_name_in_another_folder() {
        let mut root = info("main.snxsch", false, false);
        root.children = vec![
            info("a/debug.snxsch", false, true),
            info("b/debug.snxsch", true, true),
        ];
        let rows = vec![root];
        assert!(!rows_mark_missing(&rows, &key("a/debug.snxsch")));
        assert!(rows_mark_missing(&rows, &key("b/debug.snxsch")));
    }

    /// #2847: Open Sheet sends nothing while the file is missing.
    #[test]
    fn open_sheet_is_disabled_only_for_a_missing_file() {
        assert!(open_sheet_press(true).is_none());
        assert!(matches!(
            open_sheet_press(false),
            Some(PanelMsg::CommandButtonPressed(OPEN_SHEET_COMMAND))
        ));
    }

    /// A sheet holding the sheet symbol `name` at (20, 20), 40 x 30 mm, on
    /// `child.snxsch`, with an input entry on its left edge and an output
    /// on its right.
    fn sheet_with_symbol(name: &str) -> (signex_types::schematic::SchematicSheet, uuid::Uuid) {
        use signex_types::schematic::{Point, SheetPin};
        let pin = |name: &str, direction: &str, x: f64| SheetPin {
            uuid: uuid::Uuid::now_v7(),
            name: name.into(),
            direction: direction.into(),
            position: Point::new(x, 30.0),
            rotation: 0.0,
            auto_generated: false,
            user_moved: false,
        };
        let symbol = ChildSheet {
            uuid: uuid::Uuid::now_v7(),
            name: name.into(),
            filename: "child.snxsch".into(),
            position: Point::new(20.0, 20.0),
            size: (40.0, 30.0),
            stroke_width: 0.0,
            fill: Default::default(),
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: vec![pin("VIN", "input", 20.0), pin("PG", "output", 60.0)],
            instances: Vec::new(),
        };
        let id = symbol.uuid;
        let sheet = signex_types::schematic::SchematicSheet {
            paper_size: "A4".into(),
            child_sheets: vec![symbol],
            ..Default::default()
        };
        (sheet, id)
    }

    /// An app showing `sheet` in its one tab, snapping off so a click lands
    /// where it is aimed.
    pub(in crate::panels::element_properties) fn app_showing(
        sheet: signex_types::schematic::SchematicSheet,
    ) -> crate::app::Signex {
        let (mut app, _task) = crate::app::Signex::new();
        app.ui_state.first_run_tour_open = false;
        app.ui_state.snap_enabled = false;
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "props".into(),
            path: std::path::PathBuf::from("props.snxsch"),
            cached_document: None,
            project_id: None,
            kind: crate::app::TabKind::Schematic,
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        app.apply_loaded_schematic(Some(sheet), false, false, false);
        app
    }

    /// A plain click on the canvas at `(x, y)` mm, the way a user selects.
    pub(in crate::panels::element_properties) fn click(
        app: &mut crate::app::Signex,
        (x, y): (f64, f64),
    ) {
        let _task = app.update(crate::app::Message::CanvasEvent(
            crate::canvas::CanvasEvent::Clicked {
                world_x: x,
                world_y: y,
            },
        ));
    }

    /// The texts of the laid-out Properties view, top to bottom, with the
    /// bidi isolation marks of interpolated values removed.
    pub(in crate::panels::element_properties) fn properties_texts(
        app: &crate::app::Signex,
    ) -> Vec<(String, f32)> {
        let ctx = &app.document_state.panel_ctx;
        let view = super::super::selected::view_selected_element_properties(
            ctx,
            Color::WHITE,
            Color::WHITE,
            Color::BLACK,
        );
        let mut texts: Vec<(String, f32)> =
            crate::layout_probe::lay_out_texts(view, iced::Size::new(320.0, 6000.0))
                .into_iter()
                .map(|t| (t.text.replace(['\u{2068}', '\u{2069}'], ""), t.bounds.y))
                .collect();
        texts.sort_by(|a, b| a.1.total_cmp(&b.1));
        texts
    }

    /// The top of the first text that reads `want`, ignoring case.
    fn top_of(texts: &[(String, f32)], want: &str) -> Option<f32> {
        (texts.iter())
            .find(|(t, _)| t.eq_ignore_ascii_case(want))
            .map(|(_, y)| *y)
    }

    /// #2800, mockup rows 46 and 151: clicking a repeated sheet symbol on
    /// the canvas reaches the selection refresh, so Properties is headed
    /// Sheet Symbol, the Name row reads Designator, the stacked view of four
    /// channels is shown and the Repeat and Channel naming sections are there.
    #[test]
    fn clicking_a_repeated_sheet_symbol_shows_its_multi_channel_properties() {
        let (sheet, id) = sheet_with_symbol("Repeat(CH, 1, 4)");
        let mut app = app_showing(sheet);
        click(&mut app, (40.0, 35.0));
        let ctx = &app.document_state.panel_ctx;
        assert_eq!(ctx.selected_kind, Some(Kind::ChildSheet));
        let view = (ctx.compiled_design.sheet_symbol.as_ref()).expect("the multi-channel view");
        assert_eq!(view.sheet_symbol, id);
        assert_eq!(view.repeat.as_ref().map(|r| r.count()), Some(4));
        let texts = properties_texts(&app);
        let shown: Vec<&str> = texts.iter().map(|(t, _)| t.as_str()).collect();
        for want in [
            "Sheet Symbol",
            "Designator",
            "Repeat",
            "Channel name",
            "Channel naming",
            "Designators",
            "Local nets",
            "child.snxsch · repeated ×4",
        ] {
            assert!(top_of(&texts, want).is_some(), "lacks {want}: {shown:?}");
        }
        assert!(top_of(&texts, "Name").is_none(), "{shown:?}");
    }

    /// #2827, mockup row 46: one sheet keeps Style then Entries, with the
    /// switched-off Repeat after them, and its Name row reads Name.
    #[test]
    fn one_sheet_lists_its_entries_before_the_repeat_switch() {
        let (sheet, _) = sheet_with_symbol("Power");
        let mut app = app_showing(sheet);
        click(&mut app, (40.0, 35.0));
        let texts = properties_texts(&app);
        let shown: Vec<&str> = texts.iter().map(|(t, _)| t.as_str()).collect();
        let at =
            |want: &str| top_of(&texts, want).unwrap_or_else(|| panic!("lacks {want}: {shown:?}"));
        let (style, entries, repeat) = (at("Style"), at("Entries"), at("Repeat"));
        assert!(style < entries && entries < repeat, "{shown:?}");
        for entry in ["VIN", "PG", "Input", "Output"] {
            assert!(
                at(entry) > entries && at(entry) < repeat,
                "{entry}: {shown:?}"
            );
        }
        assert!(top_of(&texts, "Name").is_some(), "{shown:?}");
    }

    #[test]
    fn the_default_line_width_shows_the_placeholder() {
        assert_eq!(line_width_text(0.0, None), "");
        assert_eq!(line_width_text(0.25, None), "0.25");
        assert_eq!(line_width_text(0.25, Some(String::new())), "");
    }
}
