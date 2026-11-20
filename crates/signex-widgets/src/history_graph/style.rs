//! Sizes and colours of the history graph. Every length is an em of the
//! body text (the mockup's px at the 12 px body); every colour is a theme
//! token, except the fixed branch and author hues defined here.

use iced::Color;
use signex_types::theme::{Color as SxColor, ThemeTokens};

use crate::theme_ext::{Radii, TypeScale, to_color};

/// Row height; the dot sits at its centre.
pub const ROW_EM: f32 = 46.0 / 12.0;
/// Width of the graph column for one or two lanes.
pub const COLUMN_EM: f32 = 46.0 / 12.0;
/// Centre of lane 0 from the column's left edge.
pub const LANE_X0_EM: f32 = 14.0 / 12.0;
/// Distance between two lane centres.
pub const LANE_STEP_EM: f32 = 18.0 / 12.0;
pub const DOT_RADIUS_EM: f32 = 4.5 / 12.0;
/// Ring under a merge commit's dot.
pub const RING_RADIUS_EM: f32 = 7.0 / 12.0;
pub const RING_STROKE_EM: f32 = 2.0 / 12.0;
pub const WORKING_TREE_RADIUS_EM: f32 = 5.0 / 12.0;
pub const WORKING_TREE_STROKE_EM: f32 = 1.5 / 12.0;
/// Dash and gap of the working tree's ring.
pub const WORKING_TREE_DASH_EM: f32 = 2.0 / 12.0;
pub const LINE_EM: f32 = 2.0 / 12.0;
/// Dash and gap of the working tree's lane.
pub const LANE_DASH_EM: f32 = 3.0 / 12.0;

/// Row padding (vertical, horizontal), gap between its two lines, gap
/// between the items of line 1 and of line 2.
pub const ROW_PAD_EM: (f32, f32) = (5.0 / 12.0, 8.0 / 12.0);
pub const ROW_LINE_GAP_EM: f32 = 2.0 / 12.0;
pub const TITLE_GAP_EM: f32 = 6.0 / 12.0;
pub const CAPTION_GAP_EM: f32 = 5.0 / 12.0;
/// Width of the selected row's accent bar.
pub const SELECTED_BAR_EM: f32 = 2.0 / 12.0;
/// Space left of the graph column inside the panel.
pub const GRAPH_INSET_EM: f32 = 4.0 / 12.0;

pub const AVATAR_EM: f32 = 14.0 / 12.0;
pub const AVATAR_TEXT_EM: f32 = 7.0 / 12.0;

/// Branch and tag pills.
pub const PILL_TEXT_EM: f32 = 10.0 / 12.0;
pub const PILL_HEIGHT_EM: f32 = 15.0 / 12.0;
pub const PILL_PAD_EM: f32 = 6.0 / 12.0;
pub const PILL_RADIUS_EM: f32 = 8.0 / 12.0;

/// The working-tree row's Commit… button.
pub const COMMIT_BUTTON_EM: f32 = 20.0 / 12.0;
pub const SMALL_BUTTON_PAD_EM: f32 = 8.0 / 12.0;

/// The detail strip's ⋯ icon button and its glyph.
pub const ICON_BUTTON_EM: f32 = 24.0 / 12.0;
pub const ICON_EM: f32 = 14.0 / 12.0;

/// Detail strip: vertical margin, padding (vertical, horizontal), gaps.
pub const DETAIL_MARGIN_EM: f32 = 10.0 / 12.0;
pub const DETAIL_PAD_EM: (f32, f32) = (8.0 / 12.0, 10.0 / 12.0);
pub const DETAIL_GAP_EM: f32 = 6.0 / 12.0;
pub const DETAIL_HEAD_GAP_EM: f32 = 8.0 / 12.0;
pub const DETAIL_FILE_GAP_EM: f32 = 3.0 / 12.0;
pub const STATUS_LETTER_EM: f32 = 14.0 / 12.0;

/// Lane 1 and its branch pills; also the Added status letter.
pub const BRANCH_GREEN: Color = Color::from_rgb8(0x3C, 0xA8, 0x6A);
/// The Modified status letter.
pub const MODIFIED_AMBER: Color = Color::from_rgb8(0xC8, 0x9A, 0x2A);
const BRANCH_PURPLE: Color = Color::from_rgb8(0xA0, 0x58, 0xC8);
const BRANCH_BLUE: Color = Color::from_rgb8(0x38, 0x6C, 0xD0);
const BRANCH_TEAL: Color = Color::from_rgb8(0x2A, 0x9A, 0xA8);
/// Initials on an author disc.
pub const AVATAR_INK: Color = Color::WHITE;
/// Author disc fills, picked by a hash of the e-mail.
pub const AVATAR_FILLS: [Color; 5] = [
    BRANCH_BLUE,
    BRANCH_GREEN,
    BRANCH_PURPLE,
    MODIFIED_AMBER,
    BRANCH_TEAL,
];

/// Lanes past the palette's end reuse it from the start.
pub const LANE_COLORS: usize = 6;

/// Colours and the text size the metrics derive from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HistoryGraphStyle {
    /// Body text size; every length is an em of it.
    pub text_size: f32,
    pub caption_size: f32,
    pub radii: Radii,
    pub panel: Color,
    pub hover: Color,
    pub field: Color,
    pub sunken: Color,
    pub border: Color,
    pub ink: Color,
    pub ink3: Color,
    pub ink4: Color,
    pub accent: Color,
    pub err: Color,
    /// Lane colours: lane 0 the accent, lane 1 the mockup's green, then
    /// fixed hues and theme status colours.
    pub lanes: [Color; LANE_COLORS],
}

impl HistoryGraphStyle {
    /// Colours from the theme; an extended token a theme leaves unset falls
    /// back to the nearest base token.
    pub fn from_tokens(tokens: &ThemeTokens, scale: &TypeScale) -> Self {
        let ext = &tokens.extended;
        let or = |c: Option<SxColor>, base: SxColor| to_color(&c.unwrap_or(base));
        let accent = to_color(&tokens.accent);
        Self {
            text_size: scale.body,
            caption_size: scale.caption,
            radii: Radii::GRAPHITE,
            panel: to_color(&tokens.panel_bg),
            hover: to_color(&tokens.hover),
            field: or(ext.field, tokens.panel_bg),
            sunken: or(ext.sunken, tokens.toolbar_bg),
            border: to_color(&tokens.border),
            ink: to_color(&tokens.text),
            ink3: or(ext.ink3, tokens.text_secondary),
            ink4: or(ext.ink4, tokens.text_secondary),
            accent,
            err: to_color(&tokens.error),
            lanes: [
                accent,
                BRANCH_GREEN,
                or(ext.info, tokens.accent),
                BRANCH_PURPLE,
                to_color(&tokens.warning),
                BRANCH_BLUE,
            ],
        }
    }

    pub fn em(&self, em: f32) -> f32 {
        self.text_size * em
    }

    /// The colour of `lane`, wrapping past the palette's end.
    pub fn lane_color(&self, lane: usize) -> Color {
        self.lanes[lane % LANE_COLORS]
    }

    /// The fill of an author's initials disc.
    pub fn avatar_fill(email: &str) -> Color {
        AVATAR_FILLS[super::model::author_color_index(email, AVATAR_FILLS.len())]
    }
}
