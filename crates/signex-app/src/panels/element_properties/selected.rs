//! Properties surface for a single selected schematic element.
//!
//! Draws the header, dispatches to the per-kind builder (`symbol`,
//! `symbol_field`, `label`, `text_note`, `power_port`, `placed_kinds`, plus the
//! `drawing` and `child_sheet` surfaces), then the shared wire Style and
//! State sections and the status bar. Every placed object keeps one section
//! order: Location, Properties, (Net, Net Parameters), Style, State last.

use super::super::*;
use super::rows::PanelStyle;
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{MessageId, ids, t};
use signex_types::schematic::{SelectedItem, SelectedKind, WireWidthPreset};

#[cfg(test)]
use super::rows::{h_align_from_row, justify_h_row, justify_v_row, rotation_degrees};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

pub(in crate::panels) fn view_selected_element_properties<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let palette = PanelPalette::from_tokens(&ctx.tokens);
    let st = PanelStyle {
        muted,
        primary,
        border_c,
        palette,
    };
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);

    let elem_type = ctx
        .selection_info
        .iter()
        .find(|(k, _)| k == "Type")
        .map(|(_, v)| v.as_str())
        .unwrap_or("Object");

    let uuid = ctx.selected_uuid;
    let selected_kind = ctx.selected_kind;
    let get = |key: &str| -> String {
        ctx.selection_info
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };

    // ── Editable properties based on element type ──
    // Power ports get their own panel; regular symbols keep the existing
    // designator/value/footprint layout.
    let is_power_port = elem_type == "Power Port";
    let is_placed_part = !is_power_port && selected_kind == Some(SelectedKind::Symbol);

    // A Bezier, dimension or table has its own panel and header (#1267, #1300).
    let mut annotation = super::annotation::view_annotation_properties(ctx);

    // ── Header ──
    if annotation.is_none() {
        let header = match kind_title(elem_type, selected_kind, is_power_port) {
            Some(title) => {
                let line = match selected_kind {
                    _ if is_power_port => get("Value"),
                    Some(SelectedKind::Label) => get("Text"),
                    Some(SelectedKind::ChildSheet) => ctx
                        .selected_child_sheet
                        .as_ref()
                        .map_or_else(String::new, |sheet| sheet.name.clone()),
                    Some(SelectedKind::Wire) => run_net_line(ctx),
                    Some(SelectedKind::Bus) => ctx
                        .placed
                        .run_net
                        .as_ref()
                        .map_or_else(String::new, |net| net.name.clone()),
                    _ => String::new(),
                };
                titled_header(ctx, title, line, primary)
            }
            None => super::header::selection_header(
                ctx,
                elem_type,
                is_placed_part,
                &get("Description"),
                primary,
            ),
        };
        col = col.push(header);
        col = col.push(thin_sep(border_c));
    }

    if is_power_port && let Some(id) = uuid {
        col = super::power_port::push_power_port_sections(col, ctx, id, st);
        col = push_status_bar(col, ctx, muted, border_c);
        return scrollable(col).width(Length::Fill).boxed();
    }

    match selected_kind {
        Some(SelectedKind::Symbol) => {
            col = super::symbol::push_symbol_sections(col, ctx, uuid, st);
        }
        Some(SelectedKind::SymbolRefField) | Some(SelectedKind::SymbolValField) => {
            col = super::symbol_field::push_symbol_field_sections(col, ctx, uuid, st);
        }
        Some(SelectedKind::Label) => {
            col = super::label::push_label_sections(col, ctx, uuid, st);
        }
        Some(SelectedKind::TextNote) => {
            col = super::text_note::push_text_note_sections(col, ctx, uuid, st);
        }
        Some(SelectedKind::Drawing) => {
            col = col.push(
                annotation
                    .take()
                    .unwrap_or_else(|| view_drawing_properties(ctx, muted, primary, border_c)),
            );
        }
        Some(SelectedKind::ChildSheet) => {
            col = col.push(view_child_sheet_properties(ctx, muted, primary, border_c));
        }
        Some(
            SelectedKind::Wire
            | SelectedKind::Bus
            | SelectedKind::BusEntry
            | SelectedKind::Junction
            | SelectedKind::NoConnect
            | SelectedKind::SheetPin,
        )
        | None => {
            col = super::placed_kinds::push_placed_kind_sections(col, ctx, st);
        }
        Some(SelectedKind::Directive) => {
            col = super::directive::push_directive_sections(col, ctx, uuid, st);
        }
        Some(signex_types::schematic::SelectedKind::NoErcDirective) => {
            col = super::no_erc::push_no_erc_sections(col, ctx, uuid, st);
        }
        // #1280: an entry shows its connector.
        Some(
            signex_types::schematic::SelectedKind::HarnessConnector
            | signex_types::schematic::SelectedKind::HarnessEntry,
        ) => {
            col = super::harness_connector::push_harness_connector_sections(col, ctx, st);
        }
        Some(signex_types::schematic::SelectedKind::HarnessWire) => {
            col = super::harness_wire::push_harness_wire_sections(col, ctx, st);
        }
    }
    // #1280: a sheet entry can carry a harness.
    if let (Some(pin), Some(signex_types::schematic::SelectedKind::SheetPin)) =
        (uuid, selected_kind)
    {
        col = super::harness_wire::push_sheet_entry_harness_type(col, ctx, pin, st);
    }

    // ── Style ──: width preset and own colour, undoable.
    if let (Some(id), Some((width, color))) = (uuid, ctx.selected_wire_style) {
        let picker = ctx.wire_color_picker;
        let is_bus = selected_kind == Some(SelectedKind::Bus);
        let hex = crate::panels::HexEntry::of(
            crate::panels::ColorRowKey::Run(id),
            ctx.color_hex_draft.as_ref(),
        );
        col = col.push(
            ctx.collapsible(
                "sel_wire_style",
                ids::PROPERTIES_SECTION_STYLE,
                &ctx.collapsed_sections,
                move || {
                    Column::new()
                        .spacing(0)
                        .width(Length::Fill)
                        .push(run_width_grid(id, is_bus, width, st, &ctx.i18n))
                        .push(crate::panels::wire_color_row(
                            color,
                            picker,
                            std::rc::Rc::new(move |c| color_picked(id, is_bus, c)),
                            hex.clone(),
                            muted,
                            border_c,
                            &ctx.i18n,
                        ))
                },
            )
            .boxed(),
        );
    }

    // ── State ── (#1573): wires, labels, text notes and drawings. A bus has
    // no lock in the model.
    if let (Some(id), Some(kind)) = (uuid, selected_kind)
        && matches!(
            kind,
            SelectedKind::Wire
                | SelectedKind::Label
                | SelectedKind::TextNote
                | SelectedKind::Drawing
        )
    {
        let locked = get("Locked") == "Yes";
        let item = SelectedItem::new(id, kind);
        col = push_state_section(col, ctx, locked, PanelMsg::ToggleItemLocked(item), muted);
    }

    col = push_status_bar(col, ctx, muted, border_c);
    scrollable(col).width(Length::Fill).boxed()
}

/// The muted header line of a wire (mockup row 43): `net /SPI_CLK`, empty
/// until the project's net index has answered.
fn run_net_line(ctx: &PanelContext) -> String {
    ctx.placed.run_net.as_ref().map_or_else(String::new, |net| {
        ctx.i18n.message_plain(
            ids::PROPERTIES_WIRE_NET_LINE,
            &[("net", net.name.as_str().into())],
        )
    })
}

/// The header title of the kinds that name themselves from the catalog: a
/// wire, a bus, a label or port, a power port, a text note and a sheet
/// symbol. `None` keeps the shared header (a part, a drawing, …).
fn kind_title(
    elem_type: &str,
    kind: Option<SelectedKind>,
    is_power_port: bool,
) -> Option<MessageId> {
    if is_power_port {
        return Some(ids::PROPERTIES_KIND_POWER_PORT);
    }
    match kind? {
        SelectedKind::Wire => Some(ids::PROPERTIES_KIND_WIRE),
        SelectedKind::Bus => Some(ids::PROPERTIES_KIND_BUS),
        SelectedKind::TextNote => Some(ids::PROPERTIES_KIND_TEXT_NOTE),
        SelectedKind::ChildSheet => Some(ids::PROPERTIES_SHEET_SYMBOL_TITLE),
        // The engine names a label by its type (`Global Label`, …).
        SelectedKind::Label => Some(match elem_type {
            "Global Label" => ids::PROPERTIES_KIND_GLOBAL_PORT,
            "Hierarchical Label" => ids::PROPERTIES_KIND_HIERARCHICAL_PORT,
            _ => ids::PROPERTIES_KIND_NET_LABEL,
        }),
        _ => None,
    }
}

/// The header row: the kind's title and, muted beside it, what names this
/// one (a label's net, a power port's name).
fn titled_header<'a>(
    ctx: &PanelContext,
    title: MessageId,
    line: String,
    primary: Color,
) -> Element<'a, PanelMsg> {
    let ink3 = resolve(&ctx.tokens).ink3;
    let mut header = iced::widget::Row::<Element<'a, PanelMsg>>::new()
        .spacing(SP.m)
        .align_y(iced::Alignment::Center)
        .push(
            text(t!(ctx.i18n, title))
                .size(TS.body)
                .color(primary)
                .font(iced::Font {
                    weight: iced::font::Weight::Semibold,
                    ..signex_widgets::ui_font::regular()
                })
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
        );
    if !line.trim().is_empty() {
        header = header.push(
            text(signex_i18n::UserText::new(line))
                .size(TS.dense)
                .color(ink3)
                .wrapping(iced::widget::text::Wrapping::None)
                .ellipsis(iced::widget::text::Ellipsis::End)
                .width(Length::Fill)
                .boxed(),
        );
    }
    container(header)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// The State section, last of every placed object: the Locked check.
pub(super) fn push_state_section<'a>(
    col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    locked: bool,
    toggled: PanelMsg,
    muted: Color,
) -> Column<Element<'a, PanelMsg>> {
    let panel_bg = PanelPalette::from_tokens(&ctx.tokens).panel_bg;
    col.push(
        ctx.collapsible(
            "sel_state",
            ids::PROPERTIES_SECTION_STATE,
            &ctx.collapsed_sections,
            move || {
                Column::new()
                    .spacing(0)
                    .width(Length::Fill)
                    .push(form_check_row(
                        t!(ctx.i18n, ids::PROPERTIES_LOCKED),
                        locked,
                        toggled,
                        muted,
                        panel_bg,
                    ))
            },
        )
        .boxed(),
    )
}

/// The footer: "1 object selected".
fn push_status_bar<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    muted: Color,
    border_c: Color,
) -> Column<Element<'a, PanelMsg>> {
    col = col.push(Space::new().height(SP.m).boxed());
    col = col.push(thin_sep(border_c));
    col.push(
        container(
            text(t!(ctx.i18n, ids::PANELS_PROPERTIES_ONE_OBJECT_SELECTED))
                .size(TS.caption)
                .color(muted),
        )
        .padding([SP.s, SP.m])
        .boxed(),
    )
}

/// The Width presets in their 2 x 2 order, each with whether it is the
/// wire's: the data the segmented grid draws.
pub(super) fn wire_width_cells(current: WireWidthPreset) -> [[(WireWidthPreset, bool); 2]; 2] {
    let [a, b, c, d] = WireWidthPreset::ALL;
    let cell = |preset| (preset, preset == current);
    [[cell(a), cell(b)], [cell(c), cell(d)]]
}

/// The edit a Width pick sends for the selected wire or bus.
fn width_picked(id: uuid::Uuid, is_bus: bool, preset: WireWidthPreset) -> PanelMsg {
    if is_bus {
        PanelMsg::EditBusWidth(id, preset)
    } else {
        PanelMsg::EditWireWidth(id, preset)
    }
}

/// The edit a Color pick sends for the selected wire or bus.
fn color_picked(
    id: uuid::Uuid,
    is_bus: bool,
    color: Option<signex_types::theme::Color>,
) -> PanelMsg {
    if is_bus {
        PanelMsg::EditBusColor(id, color)
    } else {
        PanelMsg::EditWireColor(id, color)
    }
}

/// The Width row of a selected wire or bus: the four presets as a 2 x 2
/// segmented control, so no label is cut off.
fn run_width_grid<'a>(
    id: uuid::Uuid,
    is_bus: bool,
    current: WireWidthPreset,
    st: PanelStyle,
    i18n: &signex_i18n::I18n,
) -> Element<'a, PanelMsg> {
    let cells: Vec<Element<'a, PanelMsg>> = wire_width_cells(current)
        .into_iter()
        .flatten()
        .map(|(preset, active)| {
            seg_btn(
                t!(i18n, super::super::wire_style_rows::preset_label(preset)),
                active,
                width_picked(id, is_bus, preset),
                st.palette.input_bg,
                st.primary,
                st.muted,
            )
        })
        .collect();
    let grid = signex_widgets::FitRow::new(cells)
        .spacing(SP.xs)
        .fill(true)
        .max_columns(2);
    container(
        row![
            property_label(t!(i18n, ids::PROPERTIES_WIRE_WIDTH), st.muted),
            container(grid)
                .padding(SP.xs)
                .style(crate::panels::form_look::segmented_frame)
                .width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::{h_align_from_row, justify_h_row, justify_v_row, rotation_degrees};
    use signex_types::schematic::HAlign;

    /// #828 -- a non-finite rotation text never reaches the element.
    #[test]
    fn non_finite_rotation_reads_as_zero() {
        for bad in ["nan", "NaN", "inf", "-inf", "1e999", "inf°"] {
            assert_eq!(rotation_degrees(bad), 0.0, "{bad:?}");
        }
    }

    #[test]
    fn ordinary_rotation_still_reads() {
        assert_eq!(rotation_degrees("90°"), 90.0);
        assert_eq!(rotation_degrees(" 270 "), 270.0);
    }

    /// #2024 - a left-justified label shows Left: its row, as the engine
    /// writes it, reads back as `HAlign::Left` through the shared key.
    #[test]
    fn a_left_justified_label_row_reads_back_as_left() {
        use signex_types::schematic::{
            Label, LabelType, Point, SchematicSheet, SelectedItem, SelectedKind,
        };
        let label = Label::new("NET", Point::new(0.0, 0.0), LabelType::Net);
        let id = label.uuid;
        let engine = signex_engine::Engine::new(SchematicSheet {
            labels: vec![label],
            ..Default::default()
        })
        .expect("engine");
        let details = engine
            .describe_single_selection(&[SelectedItem::new(id, SelectedKind::Label)])
            .expect("details");
        let row = justify_h_row(&details.info);
        assert_eq!(row, "Left", "the row the panel shows");
        assert_eq!(h_align_from_row(&row), HAlign::Left);
        assert_eq!(h_align_from_row("Right"), HAlign::Right);
        assert_eq!(h_align_from_row("Center"), HAlign::Center);
    }

    /// #2024 - a text note's Properties rows carry its justification, read
    /// from the same keys the engine writes them under.
    #[test]
    fn a_text_note_row_carries_its_justification() {
        use signex_types::schematic::{
            Point, SchematicSheet, SelectedItem, SelectedKind, TextNote, VAlign,
        };
        let note = TextNote {
            uuid: uuid::Uuid::now_v7(),
            text: "note".into(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Top,
            locked: false,
        };
        let id = note.uuid;
        let engine = signex_engine::Engine::new(SchematicSheet {
            text_notes: vec![note],
            ..Default::default()
        })
        .expect("engine");
        let details = engine
            .describe_single_selection(&[SelectedItem::new(id, SelectedKind::TextNote)])
            .expect("details");
        assert_eq!(justify_h_row(&details.info), "Left");
        assert_eq!(justify_v_row(&details.info), "Top");
    }

    /// The text a laid-out Properties view shows for a selection of `kind`
    /// whose info rows are `info`.
    fn shown_texts(
        kind: signex_types::schematic::SelectedKind,
        info: Vec<(String, String)>,
    ) -> Vec<String> {
        use super::view_selected_element_properties;
        let (app, _task) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.selected_kind = Some(kind);
        ctx.selected_uuid = Some(uuid::Uuid::now_v7());
        ctx.selection_info = info;
        let view = view_selected_element_properties(
            &ctx,
            iced::Color::WHITE,
            iced::Color::WHITE,
            iced::Color::BLACK,
        );
        crate::layout_probe::lay_out_texts(view, iced::Size::new(400.0, 4000.0))
            .into_iter()
            .map(|t| t.text)
            .collect()
    }

    /// The Label view's window: 400 x 1200 px.
    const LABEL_WINDOW: (usize, usize) = (400, 1200);

    /// What the Label view looks like drawn, with its justification row set
    /// to `row`, and the band of pixel rows (top, bottom) its Justification
    /// row takes: the 3x3 grid's height from the name, a body of slack on
    /// either side.
    fn label_pixels(row: &str) -> (Vec<u8>, (usize, usize)) {
        use super::view_selected_element_properties;
        let (app, _task) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.selected_kind = Some(signex_types::schematic::SelectedKind::Label);
        ctx.selected_uuid = Some(uuid::Uuid::nil());
        ctx.selection_info = vec![
            ("Rotation".to_string(), "0".to_string()),
            (
                signex_engine::info_keys::JUSTIFY_H.to_string(),
                row.to_string(),
            ),
        ];
        let view = || {
            view_selected_element_properties(
                &ctx,
                iced::Color::WHITE,
                iced::Color::WHITE,
                iced::Color::BLACK,
            )
        };
        let size = iced::Size::new(LABEL_WINDOW.0 as f32, LABEL_WINDOW.1 as f32);
        let name = crate::layout_probe::lay_out_texts(view(), size)
            .into_iter()
            .rev()
            .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == "Justification")
            .expect("a Justification row")
            .bounds;
        let body = crate::styles::TypeScale::DEFAULT.body;
        let gap = crate::styles::Spacing::DEFAULT.xs / 2.0;
        let grid_h = body * crate::panels::form_look::ANCHOR_CELL_EM * 3.0 + gap * 4.0;
        let band = (
            (name.y - body).max(0.0) as usize,
            (name.y + grid_h + body) as usize,
        );
        (crate::layout_probe::draw_pixels(view(), size), band)
    }

    /// The mean x of the accent pixels `frame` draws inside the pixel rows
    /// `band` where `other` draws something else. For two views of one
    /// label with different anchors that is the grid's accent dot.
    fn accent_dot_x(frame: &[u8], other: &[u8], band: (usize, usize)) -> f32 {
        let accent = crate::panels::form_look::FormLook::of(&iced::Theme::Dark).accent;
        let [r, g, b, _] = accent.into_rgba8();
        let width = LABEL_WINDOW.0;
        let xs: Vec<usize> = (band.0 * width..(band.1 * width).min(frame.len() / 4))
            .filter(|&i| {
                let (got, theirs) = (&frame[i * 4..i * 4 + 3], &other[i * 4..i * 4 + 3]);
                got != theirs && got.iter().zip([r, g, b]).all(|(&c, w)| c.abs_diff(w) <= 1)
            })
            .map(|i| i % width)
            .collect();
        assert!(!xs.is_empty(), "no accent dot where the two views differ");
        xs.iter().sum::<usize>() as f32 / xs.len() as f32
    }

    /// #2024 - the label view selects the anchor cell its justification row
    /// names. A Left row puts the grid's accent dot in the left column, a
    /// Right row in the right one (mockup row 44). A view that read the row
    /// under another key draws both the same; a swapped reading puts the
    /// dots the other way round.
    #[test]
    fn the_label_view_selects_the_cell_its_justification_row_names() {
        let ((left, left_band), (right, right_band)) =
            (label_pixels("Left"), label_pixels("Right"));
        assert_eq!(left_band, right_band, "the row sits in the same place");
        let left_x = accent_dot_x(&left, &right, left_band);
        let right_x = accent_dot_x(&right, &left, right_band);
        // Two columns apart: more than one cell.
        let body = crate::styles::TypeScale::DEFAULT.body;
        let cell = body * crate::panels::form_look::ANCHOR_CELL_EM;
        assert!(
            right_x - left_x > cell,
            "Left row dot at x {left_x}, Right row dot at x {right_x}, cell {cell}"
        );
    }

    fn justify_info() -> Vec<(String, String)> {
        vec![
            (
                signex_engine::info_keys::JUSTIFY_H.to_string(),
                "Lefty".to_string(),
            ),
            (
                signex_engine::info_keys::JUSTIFY_V.to_string(),
                "Topsy".to_string(),
            ),
        ]
    }

    /// #2024 - the symbol-field view shows the justification rows under the
    /// keys the engine writes. A text note picks its justification on
    /// segmented rows instead (mockup row 47); `text_note::tests` reads them.
    #[test]
    fn the_field_view_shows_the_justification_rows() {
        use signex_types::schematic::SelectedKind;
        let texts = shown_texts(SelectedKind::SymbolRefField, justify_info());
        for want in ["Lefty", "Topsy"] {
            assert!(texts.iter().any(|t| t == want), "lacks {want}");
        }
    }

    /// Mockup row 46: a sheet symbol's header is the catalog's Sheet Symbol,
    /// not the engine's type word.
    #[test]
    fn a_sheet_symbol_header_reads_sheet_symbol() {
        use signex_types::schematic::SelectedKind;
        let texts = shown_texts(
            SelectedKind::ChildSheet,
            vec![("Type".to_string(), "Hierarchical Sheet".to_string())],
        );
        assert!(texts.iter().any(|t| t == "Sheet Symbol"), "{texts:?}");
        assert!(
            !texts.iter().any(|t| t == "Hierarchical Sheet"),
            "{texts:?}"
        );
    }

    /// The Width grid marks exactly the wire's preset, in the 2 x 2 order.
    #[test]
    fn the_width_grid_marks_the_wires_preset() {
        use signex_types::schematic::WireWidthPreset;
        let cells = super::wire_width_cells(WireWidthPreset::Medium);
        let order: Vec<WireWidthPreset> = cells.iter().flatten().map(|(p, _)| *p).collect();
        assert_eq!(order, WireWidthPreset::ALL);
        let active: Vec<WireWidthPreset> = (cells.iter().flatten())
            .filter(|(_, on)| *on)
            .map(|(p, _)| *p)
            .collect();
        assert_eq!(active, [WireWidthPreset::Medium]);
    }

    /// Mockup row 43: a bus's Style picks send the bus edits, a wire's the
    /// wire edits.
    #[test]
    fn style_picks_send_the_edit_of_the_selected_kind() {
        use crate::panels::PanelMsg;
        use signex_types::schematic::WireWidthPreset::Large;
        let id = uuid::Uuid::from_u128(1);
        assert!(matches!(
            super::width_picked(id, true, Large),
            PanelMsg::EditBusWidth(got, Large) if got == id
        ));
        assert!(matches!(
            super::width_picked(id, false, Large),
            PanelMsg::EditWireWidth(got, Large) if got == id
        ));
        assert!(matches!(
            super::color_picked(id, true, None),
            PanelMsg::EditBusColor(got, None) if got == id
        ));
        assert!(matches!(
            super::color_picked(id, false, None),
            PanelMsg::EditWireColor(got, None) if got == id
        ));
    }

    /// Mockup row 43: a selected bus shows the Style section.
    #[test]
    fn a_selected_bus_shows_the_style_section() {
        use signex_types::schematic::{SelectedKind, WireWidthPreset};
        let (app, _task) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.selected_kind = Some(SelectedKind::Bus);
        ctx.selected_uuid = Some(uuid::Uuid::now_v7());
        ctx.selected_wire_style = Some((WireWidthPreset::Large, None));
        let view = super::view_selected_element_properties(
            &ctx,
            iced::Color::WHITE,
            iced::Color::WHITE,
            iced::Color::BLACK,
        );
        let texts: Vec<String> =
            crate::layout_probe::lay_out_texts(view, iced::Size::new(400.0, 4000.0))
                .into_iter()
                .map(|t| t.text)
                .collect();
        for want in ["STYLE", "Smallest", "Small", "Medium", "Large"] {
            assert!(texts.iter().any(|t| t == want), "lacks {want}: {texts:?}");
        }
    }

    /// The single-object views end in the "1 object selected" footer, a
    /// power port too, and a power port shows the State ▸ Locked row.
    #[test]
    fn a_power_port_ends_in_state_and_the_footer() {
        use signex_types::schematic::SelectedKind;
        let texts = shown_texts(
            SelectedKind::Symbol,
            vec![
                ("Type".to_string(), "Power Port".to_string()),
                ("Value".to_string(), "+3V3".to_string()),
                ("Library ID".to_string(), "power:bar".to_string()),
                ("Locked".to_string(), "No".to_string()),
            ],
        );
        let texts: Vec<String> = (texts.into_iter())
            .map(|t| t.replace(['\u{2068}', '\u{2069}'], ""))
            .collect();
        let at = |want: &str| texts.iter().position(|t| t == want);
        assert!(
            at("Power Port").is_some() && at("+3V3").is_some(),
            "{texts:?}"
        );
        let (state, locked, footer) = (at("STATE"), at("Locked"), at("1 object selected"));
        assert!(
            state.is_some() && state < locked && locked < footer,
            "{texts:?}"
        );
        let net_parameters = at("NET PARAMETERS");
        assert!(net_parameters.is_some_and(|n| Some(n) < state), "{texts:?}");
    }
}
