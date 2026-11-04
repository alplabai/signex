#[allow(unused_imports)]
use iced_core::Font;
pub use iced_fonts_macros::{generate_icon_advanced_functions, generate_icon_functions};

#[cfg(feature = "bootstrap")]
/// The default icon font bytes for loading the font into iced.
pub const BOOTSTRAP_FONT_BYTES: &[u8] = include_bytes!("../fonts/bootstrap.ttf");
#[cfg(feature = "bootstrap")]
/// The lucide icon font.
pub const BOOTSTRAP_FONT: Font = Font::with_name("bootstrap-icons");
#[cfg(feature = "bootstrap")]
generate_icon_functions!(
    "fonts/bootstrap.ttf",
    bootstrap,
    BOOTSTRAP_FONT,
    "https://icons.getbootstrap.com/icons",
);

#[cfg(feature = "codicon")]
/// The default icon font bytes for loading the font into iced.
pub const CODICON_FONT_BYTES: &[u8] = include_bytes!("../fonts/codicon.ttf");
#[cfg(feature = "codicon")]
/// The codicon font.
pub const CODICON_FONT: Font = Font::with_name("codicon");
#[cfg(feature = "codicon")]
generate_icon_functions!("fonts/codicon.ttf", codicon, CODICON_FONT);

#[cfg(feature = "devicon")]
/// The default icon font bytes for loading the font into iced.
pub const DEVICON_FONT_BYTES: &[u8] = include_bytes!("../fonts/devicons.otf");
#[cfg(feature = "devicon")]
/// The devicon icon font.
pub const DEVICON_FONT: Font = Font::with_name("Devicons-NerdFont-Regular");
#[cfg(feature = "devicon")]
generate_icon_advanced_functions!("fonts/devicons.otf", devicon, DEVICON_FONT);

#[cfg(feature = "fontawesome")]
/// The default icon font bytes for loading the font into iced.
pub const FONTAWESOME_FONT_BYTES: &[u8] = include_bytes!("../fonts/FontAwesome.otf");
#[cfg(feature = "fontawesome")]
/// The fontawesome icon font.
pub const FONTAWESOME_FONT: Font = Font::with_name("FA-NerdFont-Regular");
#[cfg(feature = "fontawesome")]
generate_icon_advanced_functions!("fonts/FontAwesome.otf", fontawesome, FONTAWESOME_FONT,);

#[cfg(feature = "lucide")]
/// The default icon font bytes for loading the font into iced.
pub const LUCIDE_FONT_BYTES: &[u8] = include_bytes!("../fonts/lucide.ttf");
#[cfg(feature = "lucide")]
/// The lucide icon font.
pub const LUCIDE_FONT: Font = Font::with_name("lucide");
#[cfg(feature = "lucide")]
generate_icon_functions!(
    "fonts/lucide.ttf",
    lucide,
    LUCIDE_FONT,
    "https://lucide.dev/icons"
);

#[cfg(feature = "nerd")]
/// The default icon font bytes for loading the font into iced.
pub const NERD_FONT_BYTES: &[u8] = include_bytes!("../fonts/nerd.ttf");
#[cfg(feature = "nerd")]
/// The nerd icon font.
pub const NERD_FONT: Font = Font::with_name("Symbols Nerd Font Mono");
#[cfg(feature = "nerd")]
generate_icon_functions!("fonts/nerd.ttf", nerd, NERD_FONT);

#[cfg(feature = "octicons")]
/// The default icon font bytes for loading the font into iced.
pub const OCTICONS_FONT_BYTES: &[u8] = include_bytes!("../fonts/octicons.otf");
#[cfg(feature = "octicons")]
/// The octicon icon font.
pub const OCTICONS_FONT: Font = Font::with_name("OcticonsNerdFont-Regular");
#[cfg(feature = "octicons")]
generate_icon_advanced_functions!("fonts/octicons.otf", octicons, OCTICONS_FONT);

#[cfg(feature = "pomicons")]
/// The default icon font bytes for loading the font into iced.
pub const POMICONS_FONT_BYTES: &[u8] = include_bytes!("../fonts/pomicons.otf");
#[cfg(feature = "pomicons")]
/// The pomicon icon font.
pub const POMICONS_FONT: Font = Font::with_name("Pomicons");
#[cfg(feature = "pomicons")]
generate_icon_functions!("fonts/pomicons.otf", pomicons, POMICONS_FONT);
