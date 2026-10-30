//! Properties panel and pre-placement editor views.

use super::*;
use crate::styles::{Spacing, TypeScale};
use iced::widget::column;

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
    /// Style toggles (currently cosmetic — engine wiring tracks v0.7+).
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// Most-recent cursor world position (for the X/Y readout).
    pub cursor_x_mm: f64,
    pub cursor_y_mm: f64,
    /// Stroke width for the shape tools (Line / Rect / Circle / Arc /
    /// Polygon). 0 = Standard default ≈ 0.15 mm.
    pub shape_width_mm: f64,
    /// Fill style for shapes that support it (Rect / Circle / Polygon).
    pub shape_fill: signex_types::schematic::FillType,
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

pub const PROPERTY_LABEL_PORTION: u16 = 3;
pub const PROPERTY_CONTROL_PORTION: u16 = 5;
pub const PROPERTY_ROW_PAD_X: u16 = 6;

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
    if let Some(detail) = ctx.library_row_detail.as_ref() {
        return view_library_row_properties(
            detail,
            muted,
            primary,
            border_c,
            theme_ext::warning_color(&ctx.tokens),
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
            ctx.custom_filter_presets.clone(),
            ctx.active_custom_filter_tab,
            &ctx.collapsed_sections,
            ctx.unit,
        );
    }

    if !ctx.has_schematic {
        // Don't mislead the user into thinking nothing is loaded when
        // they've just switched to a PCB tab — distinguish "no project
        // yet" from "project open, but the active tab isn't a
        // schematic". GitHub issue #51.
        let hint = if ctx.has_pcb {
            "Properties are available when a schematic is active"
        } else {
            "Open a project"
        };
        return container(
            column![
                text("Properties").size(TS.label).color(primary),
                Space::new().height(12.0),
                text(hint).size(TS.body).color(muted),
            ]
            .spacing(SP.s)
            .padding(SP.m),
        )
        .width(Length::Fill)
        .into();
    }

    // ── Pre-placement properties (TAB pressed during tool) ──
    // TAB pauses placement and edits the properties the NEXT click will
    // commit with. We render a full Altium-style Location + Properties
    // form bound to the pre_placement data — not the live engine.
    if let Some(ref pp) = ctx.pre_placement {
        return view_pre_placement(pp, ctx, muted, primary, border_c, input_bg, input_bdr);
    }

    // ── Context-aware: if something is selected, show element properties (Altium style) ──
    if ctx.selection_count == 1 && !ctx.selection_info.is_empty() {
        return view_selected_element_properties(ctx, muted, primary, border_c);
    }
    if ctx.selection_count > 1 {
        return super::multi_selection::view_multi_selection(ctx, muted, primary, border_c);
    }

    // ── Nothing selected: show Document Options (Altium default) ──
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    col = col.push(
        container(text("Document Options").size(TS.body).color(primary))
            .padding([6.0, SP.m])
            .width(Length::Fill),
    );

    // ── General | Parameters tab bar ──
    let tab = ctx.properties_tab;
    let tab_hover = crate::styles::ti(ctx.tokens.hover);
    let text_inactive = crate::styles::ti(ctx.tokens.text_secondary);
    col = col.push(
        container(
            row![
                props_tab_btn(
                    "General",
                    tab == 0,
                    PanelMsg::PropertiesTab(0),
                    primary,
                    text_inactive,
                    tab_hover,
                    border_c
                ),
                props_tab_btn(
                    "Parameters",
                    tab == 1,
                    PanelMsg::PropertiesTab(1),
                    primary,
                    text_inactive,
                    tab_hover,
                    border_c
                ),
            ]
            .spacing(SP.xs),
        )
        .padding([SP.s, SP.m]),
    );
    col = col.push(thin_sep(border_c));

    // ── Tab content ──
    if tab == 0 {
        col = col.push(view_properties_general(ctx, muted, primary, border_c));
    } else {
        col = col.push(view_properties_parameters(
            muted,
            primary,
            border_c,
            crate::styles::ti(ctx.tokens.selection),
            crate::styles::ti(ctx.tokens.accent),
            crate::styles::ti(ctx.tokens.hover),
        ));
    }

    // ── Status: Nothing selected ──
    col = col.push(Space::new().height(8.0));
    col = col.push(thin_sep(border_c));
    col = col.push(
        container(text("Nothing selected").size(TS.caption).color(muted))
            .padding([6.0, SP.m])
            .width(Length::Fill),
    );

    scrollable(col).width(Length::Fill).into()
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
    let label_text = pp.label_text.clone();
    let designator = pp.designator.clone();
    let rotation = pp.rotation;
    let tool_name = pp.tool_name.clone();
    let kind = pp.kind;
    let pos_str = format!("{:.2}, {:.2}", pp.cursor_x_mm, pp.cursor_y_mm);
    let rot_label = format!("{:.0} Degrees", rotation);
    let font = pp.font.clone();
    let font_size_pt = pp.font_size_pt;
    let justify_h = pp.justify_h;

    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    // Header — kind-labelled, Altium style.
    col = col.push(
        container(
            row![
                text(tool_name.clone()).size(TS.label).color(primary),
                Space::new().width(Length::Fill),
                iced::widget::button(text("OK").size(TS.caption).color(Color::WHITE))
                    .padding([SP.xs, 10.0])
                    .on_press(PanelMsg::ConfirmPrePlacement)
                    .style(iced::widget::button::primary),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([6.0, SP.m])
        .width(Length::Fill),
    );

    col = col.push(container(Space::new()).height(1).width(Length::Fill).style(
        move |_: &Theme| container::Style {
            background: Some(Background::Color(border_c)),
            ..container::Style::default()
        },
    ));

    // ── Location ──
    col = col.push(collapsible_section(
        "preplace_location",
        "Location",
        &ctx.collapsed_sections,
        primary,
        border_c,
        move || {
            let mut c = Column::new().spacing(0).width(Length::Fill);
            c = c.push(form_input_row(
                "(X/Y)", &pos_str, muted, input_bg, input_bdr,
            ));
            let rotation_opts: Vec<String> = vec![
                "0 Degrees".into(),
                "90 Degrees".into(),
                "180 Degrees".into(),
                "270 Degrees".into(),
            ];
            c = c.push(form_pick_row(
                "Rotation",
                rotation_opts,
                rot_label.clone(),
                |s| {
                    let deg = s
                        .split_whitespace()
                        .next()
                        .and_then(|n| n.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    PanelMsg::SetPrePlacementRotation(deg)
                },
                muted,
            ));
            c
        },
    ));

    // ── Properties (kind-specific) ──
    let text_label_for_kind = match kind {
        PrePlacementKind::NetLabel => "Net Name",
        PrePlacementKind::GlobalPort => "Port Name",
        PrePlacementKind::HierPort => "Sheet Name",
        PrePlacementKind::PowerPort => "Net Name",
        PrePlacementKind::TextNote => "Text",
        PrePlacementKind::Component => "Value",
        _ => "",
    };

    let show_text_field = !text_label_for_kind.is_empty();
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
        col = col.push(collapsible_section(
            "preplace_props",
            "Properties",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                if show_text_field {
                    c = c.push(form_edit_row(
                        text_label_for_kind,
                        &label_text,
                        muted,
                        PanelMsg::SetPrePlacementText,
                    ));
                }
                if show_designator {
                    c = c.push(form_edit_row(
                        "Designator",
                        &designator,
                        muted,
                        PanelMsg::SetPrePlacementDesignator,
                    ));
                }
                if show_text_styling {
                    let font_opts: Vec<String> = crate::fonts::system_font_families().clone();
                    c = c.push(form_pick_row(
                        "Font",
                        font_opts,
                        font.clone(),
                        PanelMsg::SetPrePlacementFont,
                        muted,
                    ));
                    let size_opts: Vec<String> = [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72]
                        .iter()
                        .map(|n| n.to_string())
                        .collect();
                    c = c.push(form_pick_row(
                        "Font Size",
                        size_opts,
                        font_size_pt.to_string(),
                        |s| PanelMsg::SetPrePlacementFontSize(s.parse().unwrap_or(10)),
                        muted,
                    ));
                    c = c.push(font_style_row(muted, primary, input_bg, input_bdr));
                    c = c.push(form_label("Justification", muted));
                    c = c.push(
                        container(preplacement_justification_grid(
                            justify_h,
                            input_bg,
                            input_bdr,
                            primary,
                            ctx.theme_id,
                        ))
                        .padding([SP.s, SP.m]),
                    );
                }
                c
            },
        ));
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
        col = col.push(collapsible_section(
            "preplace_shape",
            "Properties",
            &ctx.collapsed_sections,
            primary,
            border_c,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = c.push(form_edit_row_f64(
                    "Width (mm)",
                    width,
                    crate::panels::NumericFieldKey::ShapeWidth,
                    numeric_draft,
                    muted,
                ));
                if show_fill {
                    c = c.push(shape_fill_row(fill, muted, border_c));
                }
                c
            },
        ));
    } else {
        col = col.push(
            container(
                text("Click to place. No per-instance options.")
                    .size(TS.caption)
                    .color(muted),
            )
            .padding([8, PROPERTY_ROW_PAD_X]),
        );
    }

    container(scrollable(col).width(Length::Fill))
        .width(Length::Fill)
        .into()
}
