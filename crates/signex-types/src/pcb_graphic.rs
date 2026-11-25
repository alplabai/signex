//! The kinds of board drawing the Graphics tools place (#2424), as
//! [`BoardGraphic::graphic_type`](crate::pcb::BoardGraphic) names them, and
//! the layers a board drawing may go on.

use crate::pcb::BoardGraphic;
use crate::pcb_layer::{PcbLayer, TechnicalLayer};

/// What a board drawing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoardGraphicKind {
    /// From `start` to `end`.
    Line,
    /// About `center` from `start` to `end`, the way of increasing angle in
    /// the model's axes; `radius` is the distance to `start`.
    Arc,
    /// About `center` through `end`, of `radius`.
    Circle,
    /// The upright rectangle with opposite corners `start` and `end`.
    Rectangle,
    /// `text` wrapped inside the upright box with opposite corners `start`
    /// and `end`, at `font_size`, the box drawn around it.
    TextFrame,
}

impl BoardGraphicKind {
    pub const ALL: [Self; 5] = [
        Self::Line,
        Self::Arc,
        Self::Circle,
        Self::Rectangle,
        Self::TextFrame,
    ];

    /// The `graphic_type` token on disk.
    pub fn token(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Arc => "arc",
            Self::Circle => "circle",
            Self::Rectangle => "rect",
            Self::TextFrame => "text_frame",
        }
    }

    /// The kind a `graphic_type` token names; `None` for a token this build
    /// does not place.
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.token() == token)
    }

    /// The name an agent passes (`kind=`): the token, but `rectangle`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Rectangle => "rectangle",
            other => other.token(),
        }
    }

    /// The kind an agent's `kind=` names.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.id() == id.trim())
    }
}

impl BoardGraphic {
    /// The kind of drawing this is; `None` for a kind no Graphics tool
    /// places (a polygon, say).
    pub fn kind(&self) -> Option<BoardGraphicKind> {
        BoardGraphicKind::from_token(&self.graphic_type)
    }
}

/// A text frame's text as a one-line field shows it: each line break as
/// `\n`.
pub fn text_to_field(text: &str) -> String {
    text.replace('\n', "\\n")
}

/// A one-line field's text as a text frame holds it: each `\n` a line
/// break.
pub fn text_from_field(field: &str) -> String {
    field.replace("\\n", "\n")
}

/// Whether a board drawing may go on `layer`: any layer but the keep-out
/// layers, whose areas the design rule check reads as rules. On copper a
/// drawing is copper of no net along its stroke (Caner, 2026-11-24, #2883).
pub fn takes_board_graphics(layer: PcbLayer) -> bool {
    !matches!(
        layer,
        PcbLayer::Technical(TechnicalLayer::KeepoutTop | TechnicalLayer::KeepoutBottom)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb_layer::CopperLayer;

    #[test]
    fn every_kind_reads_back_from_its_token_and_its_id() {
        for kind in BoardGraphicKind::ALL {
            assert_eq!(BoardGraphicKind::from_token(kind.token()), Some(kind));
            assert_eq!(BoardGraphicKind::from_id(kind.id()), Some(kind));
        }
        assert_eq!(BoardGraphicKind::from_token("poly"), None);
        assert_eq!(
            BoardGraphicKind::from_id("rectangle"),
            Some(BoardGraphicKind::Rectangle)
        );
    }

    #[test]
    fn a_line_break_is_written_as_backslash_n_in_a_field() {
        assert_eq!(text_to_field("REV A\nDATE"), "REV A\\nDATE");
        assert_eq!(text_from_field("REV A\\nDATE"), "REV A\nDATE");
        assert_eq!(text_from_field(&text_to_field("a\nb\nc")), "a\nb\nc");
    }

    /// Was `drawings_go_on_non_copper_layers_but_keep_outs`; copper takes a
    /// drawing since Caner's 2026-11-24 decision (#2883).
    #[test]
    fn drawings_go_on_every_layer_but_keep_outs() {
        assert!(takes_board_graphics(PcbLayer::Technical(
            TechnicalLayer::SilkscreenTop
        )));
        assert!(takes_board_graphics(PcbLayer::Technical(
            TechnicalLayer::CourtyardBottom
        )));
        assert!(takes_board_graphics(PcbLayer::Copper(CopperLayer::Top)));
        assert!(!takes_board_graphics(PcbLayer::Technical(
            TechnicalLayer::KeepoutTop
        )));
    }
}
