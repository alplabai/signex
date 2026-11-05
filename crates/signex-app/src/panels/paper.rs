//! Sheet colour, page format / origin and paper-size helpers.

/// Sheet background colour presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetColor {
    #[default]
    Black,
    White,
    DarkGray,
    LightGray,
    Cream,
}

impl SheetColor {
    pub fn to_color(self) -> iced::Color {
        crate::render_config::to_iced(&self.to_theme_color())
    }

    /// The preset as a stored colour, opaque.
    pub const fn to_theme_color(self) -> signex_types::theme::Color {
        let (r, g, b) = match self {
            SheetColor::Black => (0x14, 0x14, 0x14),
            SheetColor::White => (0xFF, 0xFF, 0xFF),
            SheetColor::DarkGray => (0x2A, 0x2A, 0x2A),
            SheetColor::LightGray => (0xD0, 0xD0, 0xD0),
            SheetColor::Cream => (0xFB, 0xF4, 0xE0),
        };
        signex_types::theme::Color::new(r, g, b, 0xFF)
    }
    pub const ALL: &'static [SheetColor] = &[
        SheetColor::Black,
        SheetColor::White,
        SheetColor::DarkGray,
        SheetColor::LightGray,
        SheetColor::Cream,
    ];
}

impl std::fmt::Display for SheetColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SheetColor::Black => "Black",
            SheetColor::White => "White",
            SheetColor::DarkGray => "Dark Gray",
            SheetColor::LightGray => "Light Gray",
            SheetColor::Cream => "Cream",
        })
    }
}

/// Altium-style page formatting mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageFormatMode {
    Template,
    #[default]
    Standard,
    Custom,
}

/// A schematic sheet's Sheet Color (#2034). `Default` stores nothing and
/// follows the theme's paper colour; the presets are the symbol editor's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SchematicSheetColor {
    #[default]
    Default,
    Preset(SheetColor),
    /// A colour from the file that is no preset (shown, not offered).
    Custom(signex_types::theme::Color),
}

impl SchematicSheetColor {
    /// The picker's choices: Default, then every preset.
    pub fn options() -> Vec<Self> {
        std::iter::once(Self::Default)
            .chain(SheetColor::ALL.iter().copied().map(Self::Preset))
            .collect()
    }

    /// The choice a stored colour reads as.
    pub fn of(stored: Option<signex_types::theme::Color>) -> Self {
        let Some(color) = stored else {
            return Self::Default;
        };
        SheetColor::ALL
            .iter()
            .copied()
            .find(|preset| preset.to_theme_color() == color)
            .map_or(Self::Custom(color), Self::Preset)
    }

    /// The colour stored in the sheet; `None` for Default.
    pub fn stored(self) -> Option<signex_types::theme::Color> {
        match self {
            Self::Default => None,
            Self::Preset(preset) => Some(preset.to_theme_color()),
            Self::Custom(color) => Some(color),
        }
    }
}

impl std::fmt::Display for SchematicSheetColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default => f.write_str("Default"),
            Self::Preset(preset) => preset.fmt(f),
            Self::Custom(color) => write!(f, "Custom ({})", color.to_hex()),
        }
    }
}

/// Page coordinate origin; the model lives in signex-types.
pub use signex_types::schematic::PageOrigin;

/// Picker entries: the stored token of every named sheet size, in display
/// order. The model lives in `signex_types::schematic::SheetSize`.
pub fn paper_size_options() -> Vec<String> {
    use signex_types::schematic::{SheetOrientation, SheetSize};
    SheetSize::PICKABLE
        .iter()
        .map(|size| size.token(SheetOrientation::Landscape))
        .collect()
}

/// (width_mm, height_mm) for a stored paper-size token, read through the
/// shared sheet-size model.
pub fn paper_dimensions(size: &str) -> (f32, f32) {
    let (size, orientation) = signex_types::schematic::SheetSize::parse(size);
    let (w, h) = size.dimensions_mm(orientation);
    (w as f32, h as f32)
}
