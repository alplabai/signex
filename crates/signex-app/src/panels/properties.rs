//! Properties panel and pre-placement editor views.

use super::*;
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use iced::widget::column;
use signex_i18n::{ids, t};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Pre-placement configuration data — shown in Properties panel when Tab pressed.
#[derive(Debug, Clone)]
pub struct PrePlacementData {
    /// Which tool is being configured.
    pub tool_name: String,
    /// Semantic kind so the panel can render the right field set.
    pub kind: PrePlacementKind,
    /// Net label / text note text.
    pub label_text: String,
    /// Component designator override.
    pub designator: String,
    /// Rotation (degrees).
    pub rotation: f64,
    /// Font family (cosmetic until font switching ships).
    pub font: String,
    /// Font size in points (10 pt = Altium default).
    pub font_size_pt: u32,
    /// Horizontal justification.
    pub justify_h: signex_types::schematic::HAlign,
    /// Vertical justification (TextNote / Component fields).
    pub justify_v: signex_types::schematic::VAlign,
    /// Style toggles. Not yet applied to placed text; underline has no
    /// renderer support.
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// Stored cursor position for the X/Y readout; shown through the sheet's
    /// page origin by [`pre_placement_readout`].
    pub cursor_x_mm: f64,
    pub cursor_y_mm: f64,
    /// Stroke width for the shape tools (Line / Rect / Circle / Arc /
    /// Polygon). 0 = Standard default ≈ 0.15 mm.
    pub shape_width_mm: f64,
    /// Fill style for shapes that support it (Rect / Circle / Polygon).
    pub shape_fill: signex_types::schematic::FillType,
    /// Width of the wires the Wire tool routes (Tab form).
    pub wire_width: signex_types::schematic::WireWidthPreset,
    /// Colour of those wires; `None` = the theme's wire colour.
    pub wire_color: Option<signex_types::theme::Color>,
}

/// The look of the text a pre-placement form configures; what Repeat last
/// item carries over to the next copy.
#[derive(Debug, Clone, PartialEq)]
pub struct PrePlacementStyle {
    pub rotation: f64,
    pub font: String,
    pub font_size_pt: u32,
    pub justify_h: signex_types::schematic::HAlign,
    pub justify_v: signex_types::schematic::VAlign,
}

impl PrePlacementData {
    /// Pre-placement data with the one set of defaults every placement tool
    /// starts from.
    pub fn new(
        tool_name: String,
        kind: PrePlacementKind,
        label_text: String,
        cursor_x_mm: f64,
        cursor_y_mm: f64,
    ) -> Self {
        Self {
            tool_name,
            kind,
            label_text,
            designator: String::new(),
            rotation: 0.0,
            font: "Iosevka".to_string(),
            font_size_pt: 10,
            justify_h: signex_types::schematic::HAlign::Left,
            justify_v: Default::default(),
            bold: false,
            italic: false,
            underline: false,
            cursor_x_mm,
            cursor_y_mm,
            shape_width_mm: 0.0,
            shape_fill: signex_types::schematic::FillType::None,
            wire_width: Default::default(),
            wire_color: None,
        }
    }

    /// The text look this data configures.
    pub fn style(&self) -> PrePlacementStyle {
        PrePlacementStyle {
            rotation: self.rotation,
            font: self.font.clone(),
            font_size_pt: self.font_size_pt,
            justify_h: self.justify_h,
            justify_v: self.justify_v,
        }
    }

    /// Take over `style`.
    pub fn set_style(&mut self, style: PrePlacementStyle) {
        self.rotation = style.rotation;
        self.font = style.font;
        self.font_size_pt = style.font_size_pt;
        self.justify_h = style.justify_h;
        self.justify_v = style.justify_v;
    }
}

/// Stable identifiers for every numeric drawing-field editor so the
/// panel keeps a transient string buffer per field across rerenders.
/// Erasing a text_input leaves an empty string in the buffer until
/// the user types a valid f64, at which point UpdateDrawingEdit fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrawingFieldId {
    LineStartX,
    LineStartY,
    LineEndX,
    LineEndY,
    LineWidth,
    RectStartX,
    RectStartY,
    RectWidth,
    RectHeight,
    RectBorder,
    CircleCenterX,
    CircleCenterY,
    CircleRadius,
    CircleBorder,
    ArcCenterX,
    ArcCenterY,
    ArcRadius,
    ArcStartAngle,
    ArcEndAngle,
    ArcWidth,
    PolyBorder,
}

impl DrawingFieldId {
    /// A Y coordinate on the sheet: shown and typed from the page origin.
    pub fn is_page_y(self) -> bool {
        matches!(
            self,
            Self::LineStartY
                | Self::LineEndY
                | Self::RectStartY
                | Self::CircleCenterY
                | Self::ArcCenterY
        )
    }
}

/// Distinguishes placement flavors so the pre-placement form only shows
/// fields relevant to what the user is about to drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrePlacementKind {
    Wire,
    Bus,
    BusEntry,
    NoConnect,
    NetLabel,
    GlobalPort,
    HierPort,
    PowerPort,
    TextNote,
    Component,
    Line,
    Rectangle,
    Circle,
    Arc,
    Polygon,
    Other,
}

// ─── Properties Panel (matched to Altium Designer) ───────────

/// Width of the property-row label column: the fixed grid column, so a label
/// such as "Designator" is never clipped by a narrow dock.
pub const PROPERTY_LABEL_WIDTH: f32 = 92.0;
/// Property-row label column: fixed, so a wide dock gives its extra width to
/// the fields (#2347).
pub const PROPERTY_LABEL_COLUMN: Length = Length::Fixed(PROPERTY_LABEL_WIDTH);
pub const PROPERTY_CONTROL_PORTION: u16 = 5;
pub const PROPERTY_ROW_PAD_X: u16 = 6;
/// Gap between a property row's label and its field.
pub const PROPERTY_ROW_GAP: f32 = 6.0;

pub fn view_properties<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let palette = PanelPalette::from_tokens(&ctx.tokens);
    let PanelPalette {
        muted,
        primary,
        border: border_c,
        input_bg,
        input_bdr,
        ..
    } = palette;

    // Library Browser tab — Properties panel surfaces the selected
    // row's metadata + Pick Symbol / Pick Footprint. F15 (2026-05-03
    // library polish): "right pane can be opened on properties
    // instead." Takes precedence over the schematic / pre-placement /
    // symbol-editor branches so when a Library Browser tab is active
    // the panel stays focused on it.
    if let Some(sheet) = ctx.component_sheet.as_ref() {
        return super::view_component_sheet(sheet, &ctx.tokens, &ctx.i18n);
    }
    // A channel tab of a repeated sheet shows its channel, read-only (#1279).
    if let Some(channel) = super::element_properties::channel_view_properties(ctx) {
        return channel;
    }
    if let Some(detail) = ctx.library_row_detail.as_ref() {
        return view_library_row_properties(
            detail,
            muted,
            primary,
            border_c,
            theme_ext::warning_color(&ctx.tokens),
            &ctx.i18n,
        );
    }

    // Symbol-editor tab takes precedence — when the user is editing a
    // `.snxsym` the right-dock Properties panel shows symbol/pin
    // properties driven by `panel_ctx.symbol_editor`. Matches Altium's
    // SchLib editor flow where the same Properties panel switches mode
    // based on selection. (#62 / v0.9 phase 1)
    if let Some(sym) = ctx.symbol_editor.as_ref() {
        return view_symbol_editor_properties(
            sym,
            ctx.numeric_draft.as_ref(),
            muted,
            primary,
            border_c,
            &ctx.tokens,
            &ctx.i18n,
        );
    }

    // v0.14.2 — Footprint-editor tab. Properties panel switches body
    // based on (mode × selection): Pads-mode pad selected → pad
    // properties; Sketch-mode entity selected → sketch entity
    // properties; nothing selected → footprint summary + solve stats.
    if let Some(fp) = ctx.footprint_editor.as_ref() {
        return view_footprint_editor_properties(
            fp,
            palette,
            &ctx.collapsed_sections,
            ctx.unit,
            &ctx.i18n,
        );
    }

    if !ctx.has_schematic
        && let Some(board) = ctx.pcb_board.as_ref()
    {
        return super::pcb_board_properties::view(ctx, board);
    }
    if !ctx.has_schematic {
        // Don't mislead the user into thinking nothing is loaded when
        // they've just switched to a PCB tab — distinguish "no project
        // yet" from "project open, but the active tab isn't a
        // schematic". GitHub issue #51.
        let hint = if ctx.has_pcb {
            ids::PANELS_PROPERTIES_NEEDS_SCHEMATIC
        } else {
            ids::PANELS_PROPERTIES_OPEN_PROJECT
        };
        return container(
            column![
                text(t!(ctx.i18n, ids::PANELS_KIND_PROPERTIES))
                    .size(TS.label)
                    .color(primary),
                Space::new().height(12.0),
                text(t!(ctx.i18n, hint)).size(TS.body).color(muted),
            ]
            .spacing(SP.s)
            .padding(SP.m),
        )
        .width(Length::Fill)
        .boxed();
    }

    // ── Pre-placement properties (TAB pressed during tool) ──
    // TAB pauses placement and edits the properties the NEXT click will
    // commit with. We render a full Altium-style Location + Properties
    // form bound to the pre_placement data — not the live engine.
    if let Some(ref pp) = ctx.pre_placement {
        return view_pre_placement(pp, ctx, muted, primary, border_c, input_bg, input_bdr);
    }

    // #1280: the harness definitions view, while it is open.
    if ctx.harness.definitions_open {
        return super::view_harness_definitions(ctx, muted, primary);
    }

    // ── Context-aware: if something is selected, show element properties (Altium style) ──
    if ctx.selection_count == 1 && !ctx.selection_info.is_empty() {
        return view_selected_element_properties(ctx, muted, primary, border_c);
    }
    if ctx.selection_count > 1 {
        return super::multi_selection::view_multi_selection(ctx, muted, primary, border_c);
    }

    // ── Nothing selected: show Document Options (Altium default) ──
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);

    col = col.push(
        container(
            text(signex_i18n::t!(
                ctx.i18n,
                signex_i18n::ids::PANELS_PROPERTIES_DOCUMENT_OPTIONS
            ))
            .size(TS.title)
            .color(primary),
        )
        .padding([10.0, 12.0])
        .width(Length::Fill)
        .boxed(),
    );

    // ── General | Parameters tab bar ──
    let tab = ctx.properties_tab;
    let text_inactive = crate::styles::ti(ctx.tokens.text_secondary);
    col = col.push(
        container(
            row![
                props_tab_btn(
                    t!(ctx.i18n, ids::PANELS_PROPERTIES_TAB_GENERAL),
                    tab == 0,
                    PanelMsg::PropertiesTab(0),
                    primary,
                    text_inactive
                ),
                props_tab_btn(
                    t!(ctx.i18n, ids::PANELS_PROPERTIES_TAB_PARAMETERS),
                    tab == 1,
                    PanelMsg::PropertiesTab(1),
                    primary,
                    text_inactive
                ),
            ]
            .spacing(SP.xs),
        )
        .padding([SP.s, 12.0])
        .boxed(),
    );
    // ── Tab content ──
    if tab == 0 {
        col = col.push(view_properties_general(ctx, primary).boxed());
    } else {
        col = col.push(view_properties_parameters(ctx, primary).boxed());
    }

    // ── Status: Nothing selected ──
    col = col.push(Space::new().height(8.0).boxed());
    col = col.push(
        container(
            text(t!(ctx.i18n, ids::PANELS_PROPERTIES_NOTHING_SELECTED))
                .size(TS.caption)
                .color(muted),
        )
        .padding([6.0, 12.0])
        .width(Length::Fill)
        .boxed(),
    );

    scrollable(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(super::form_look::scrollbar_style)
        .boxed()
}

/// The pre-placement X/Y readout: Y from the active sheet's page origin
/// (#2034), read when drawn so an origin change shows at once.
pub(crate) fn pre_placement_readout(pp: &PrePlacementData, ctx: &PanelContext) -> String {
    let y = ctx.shown_y(pp.cursor_y_mm);
    format!("{:.2}, {y:.2}", pp.cursor_x_mm)
}

/// Pre-placement properties — shown when TAB pressed during a placement tool.
fn view_pre_placement<'a>(
    pp: &PrePlacementData,
    ctx: &'a PanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
    input_bg: Color,
    input_bdr: Color,
) -> Element<'a, PanelMsg> {
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let label_text = pp.label_text.clone();
    let designator = pp.designator.clone();
    let rotation = pp.rotation;
    let tool_name = pp.tool_name.clone();
    let kind = pp.kind;
    let pos_str = pre_placement_readout(pp, ctx);
    let font = pp.font.clone();
    let font_size_pt = pp.font_size_pt;
    let justify_h = pp.justify_h;

    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);

    // Header — kind-labelled, Altium style.
    col = col.push(
        container(
            row![
                // raw: the tool name is worded by the placement dispatch, in
                // English (#2061: handler-worded state text).
                text(raw(tool_name.clone())).size(TS.label).color(primary),
                Space::new().width(Length::Fill),
                crate::styles::dialog_button(
                    t!(ctx.i18n, signex_i18n::ids::DIALOG_BUTTON_OK),
                    crate::styles::DialogButtonKind::Primary,
                    Some(PanelMsg::ConfirmPrePlacement),
                    &ctx.tokens,
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([6.0, SP.m])
        .width(Length::Fill)
        .boxed(),
    );

    col = col.push(
        container(Space::new())
            .height(1)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(border_c)),
                ..container::Style::default()
            })
            .boxed(),
    );

    // ── Location ──
    col = col.push(
        ctx.collapsible(
            "preplace_location",
            ids::PANELS_PLACED_LOCATION,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_input_row(
                    t!(i18n, ids::PROPERTIES_XY),
                    &pos_str,
                    muted,
                    input_bg,
                ));
                c = c.push(form_rotation_row(
                    i18n,
                    rotation,
                    PanelMsg::SetPrePlacementRotation,
                    muted,
                ));
                c
            },
        )
        .boxed(),
    );

    // ── Properties (kind-specific) ──
    let text_label_for_kind = match kind {
        PrePlacementKind::NetLabel => Some(ids::PROPERTIES_NET_NAME),
        PrePlacementKind::GlobalPort => Some(ids::PROPERTIES_PORT_NAME),
        PrePlacementKind::HierPort => Some(ids::PROPERTIES_SHEET_NAME),
        PrePlacementKind::PowerPort => Some(ids::PROPERTIES_NET_NAME),
        PrePlacementKind::TextNote => Some(ids::PROPERTIES_TEXT),
        PrePlacementKind::Component => Some(ids::PROPERTIES_VALUE),
        _ => None,
    };

    let show_text_field = text_label_for_kind.is_some();
    let show_designator = matches!(kind, PrePlacementKind::Component);
    let show_text_styling = matches!(
        kind,
        PrePlacementKind::NetLabel
            | PrePlacementKind::GlobalPort
            | PrePlacementKind::HierPort
            | PrePlacementKind::PowerPort
            | PrePlacementKind::TextNote
            | PrePlacementKind::Component
    );

    if show_text_field || show_text_styling {
        col = col.push(
            ctx.collapsible(
                "preplace_props",
                ids::PANELS_PLACED_PROPERTIES,
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    if let Some(label) = text_label_for_kind {
                        c = c.push(form_edit_row(
                            t!(i18n, label),
                            &label_text,
                            muted,
                            PanelMsg::SetPrePlacementText,
                        ));
                    }
                    if show_designator {
                        c = c.push(form_edit_row(
                            t!(i18n, ids::PROPERTIES_DESIGNATOR),
                            &designator,
                            muted,
                            PanelMsg::SetPrePlacementDesignator,
                        ));
                    }
                    if show_text_styling {
                        let font_opts: Vec<String> = crate::fonts::system_font_families().clone();
                        c = c.push(form_pick_row(
                            t!(i18n, ids::PROPERTIES_FONT),
                            font_opts,
                            font.clone(),
                            PanelMsg::SetPrePlacementFont,
                            muted,
                        ));
                        let size_opts: Vec<String> =
                            [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72]
                                .iter()
                                .map(|n| n.to_string())
                                .collect();
                        c = c.push(form_pick_row(
                            t!(i18n, ids::PROPERTIES_FONT_SIZE),
                            size_opts,
                            font_size_pt.to_string(),
                            |s| PanelMsg::SetPrePlacementFontSize(s.parse().unwrap_or(10)),
                            muted,
                        ));
                        c = c.push(font_style_row(muted, primary, input_bg, input_bdr));
                        c = c.push(form_label(signex_i18n::t!(
                            i18n,
                            signex_i18n::ids::PANELS_PROPERTIES_JUSTIFICATION
                        )));
                        c = c.push(
                            container(preplacement_justification_grid(
                                justify_h,
                                input_bg,
                                input_bdr,
                                primary,
                                ctx.theme_id,
                            ))
                            .padding([SP.s, SP.m])
                            .boxed(),
                        );
                    }
                    c
                },
            )
            .boxed(),
        );
    } else if matches!(
        kind,
        PrePlacementKind::Line
            | PrePlacementKind::Rectangle
            | PrePlacementKind::Circle
            | PrePlacementKind::Arc
            | PrePlacementKind::Polygon
    ) {
        // Shape tools — Altium-style Width + Fill so users can
        // preconfigure the next placement via TAB.
        let width = pp.shape_width_mm;
        let fill = pp.shape_fill;
        let show_fill = !matches!(kind, PrePlacementKind::Line | PrePlacementKind::Arc);
        let numeric_draft = ctx.numeric_draft.as_ref();
        col = col.push(
            ctx.collapsible(
                "preplace_shape",
                ids::PANELS_PLACED_PROPERTIES,
                &ctx.collapsed_sections,
                move || {
                    let mut c = Column::new().spacing(0).width(Length::Fill);
                    c = c.push(form_edit_row_f64(
                        t!(i18n, ids::PROPERTIES_WIDTH_MM),
                        width,
                        crate::panels::NumericFieldKey::ShapeWidth,
                        numeric_draft,
                        muted,
                    ));
                    if show_fill {
                        c = c.push(shape_fill_row(fill, muted, border_c, i18n));
                    }
                    c
                },
            )
            .boxed(),
        );
    } else if kind == PrePlacementKind::Wire {
        let (width, color, picker) = (pp.wire_width, pp.wire_color, ctx.wire_color_picker);
        col = col.push(
            ctx.collapsible(
                "preplace_wire",
                ids::PANELS_PLACED_PROPERTIES,
                &ctx.collapsed_sections,
                move || {
                    Column::new()
                        .spacing(0)
                        .width(Length::Fill)
                        .push(super::wire_width_row(
                            width,
                            PanelMsg::SetPrePlacementWireWidth,
                            muted,
                            i18n,
                        ))
                        .push(super::wire_color_row(
                            color,
                            picker,
                            std::rc::Rc::new(PanelMsg::SetPrePlacementWireColor),
                            muted,
                            border_c,
                            i18n,
                        ))
                },
            )
            .boxed(),
        );
    } else {
        col = col.push(
            container(
                text(t!(i18n, ids::PANELS_PROPERTIES_NO_PLACEMENT_OPTIONS))
                    .size(TS.caption)
                    .color(muted),
            )
            .padding([8, PROPERTY_ROW_PAD_X])
            .boxed(),
        );
    }

    container(
        scrollable(col)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(super::form_look::scrollbar_style),
    )
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
#[path = "properties_tests.rs"]
mod tests;
