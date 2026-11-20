//! The data a placement tool carries for its next object: what the Tab form
//! edits.

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
    /// Font size in points (10 pt by default).
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
    pub position: super::PlacementPosition,
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
