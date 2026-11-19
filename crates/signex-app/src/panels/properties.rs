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
    /// The cursor, snapped as a click would place: untyped Location fields
    /// follow it.
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
    /// Typed Position X / Y (a wire's Start and End): where OK places the
    /// next object. Never carried over to the object after it.
    pub position: super::pre_placement::PlacementPosition,
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
            position: Default::default(),
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

    // Tab pauses placement; the form edits what the next object is placed
    // with (#2532).
    if let Some(ref pp) = ctx.pre_placement {
        return super::pre_placement::view(pp, ctx);
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
        col = col.push(
            view_properties_parameters(
                &ctx.tokens,
                muted,
                primary,
                crate::styles::ti(ctx.tokens.selection),
                &ctx.i18n,
            )
            .boxed(),
        );
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

#[cfg(test)]
#[path = "properties_tests.rs"]
mod tests;
