//! Small edit values carried by [`super::PanelMsg`].

/// One edit of a footprint's generated features (mockup row 124). Typed
/// values carry the field's text; a refused one stays in the field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FootprintFeatureEdit {
    SilkOutlineToggled,
    SilkLineWidthTyped(String),
    PadClearanceTyped(String),
    PinOneMarkerToggled,
    AssemblyOutlineToggled,
}

/// One change to the active sheet's frame settings (#1134).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetFrameEdit {
    ShowBorder(bool),
    ShowTitleBlock(bool),
    /// Zones lettered A, B, C… down the sides.
    VerticalZones(u8),
    /// Zones numbered 1, 2, 3… along the top and bottom.
    HorizontalZones(u8),
}

impl SheetFrameEdit {
    /// `settings` with this change applied.
    pub fn apply(
        self,
        settings: signex_types::schematic::SheetFrameSettings,
    ) -> signex_types::schematic::SheetFrameSettings {
        let mut settings = settings;
        match self {
            Self::ShowBorder(on) => settings.show_border = on,
            Self::ShowTitleBlock(on) => settings.show_title_block = on,
            Self::VerticalZones(n) => settings.vertical_zones = n,
            Self::HorizontalZones(n) => settings.horizontal_zones = n,
        }
        settings
    }
}
