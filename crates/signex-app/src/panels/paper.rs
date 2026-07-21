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
        match self {
            SheetColor::Black => iced::Color::from_rgb8(0x14, 0x14, 0x14),
            SheetColor::White => iced::Color::WHITE,
            SheetColor::DarkGray => iced::Color::from_rgb8(0x2A, 0x2A, 0x2A),
            SheetColor::LightGray => iced::Color::from_rgb8(0xD0, 0xD0, 0xD0),
            SheetColor::Cream => iced::Color::from_rgb8(0xFB, 0xF4, 0xE0),
        }
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

/// Altium-style page coordinate origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageOrigin {
    UpperLeft,
    #[default]
    LowerLeft,
}

impl std::fmt::Display for PageOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PageOrigin::UpperLeft => "Upper Left",
            PageOrigin::LowerLeft => "Lower Left",
        })
    }
}

/// Supported paper sizes (Altium-compatible subset).
pub const PAPER_SIZES: &[&str] = &[
    "A0", "A1", "A2", "A3", "A4", "A5", "B5", "Letter", "Legal", "Tabloid",
];

/// (width_mm, height_mm) for a paper size string.
pub fn paper_dimensions(size: &str) -> (f32, f32) {
    match size {
        "A0" => (1189.0, 841.0),
        "A1" => (841.0, 594.0),
        "A2" => (594.0, 420.0),
        "A3" => (420.0, 297.0),
        "A4" => (297.0, 210.0),
        "A5" => (210.0, 148.0),
        "B5" => (257.0, 182.0),
        "Letter" => (279.4, 215.9),
        "Legal" => (355.6, 215.9),
        "Tabloid" => (431.8, 279.4),
        _ => (297.0, 210.0),
    }
}
