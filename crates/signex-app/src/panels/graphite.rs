//! Graphite looks of the panel contents: the project tree rows, the
//! panel buttons and list rows. Legacy themes
//! never reach these; each caller branches on `is_graphite`.

use iced::Color;
use signex_types::theme::ThemeTokens;
use signex_widgets::tree_view::{BadgeTone, TreeIcon, TreeNode, TreeStyle};

use crate::styles::tokens::{relative_luminance, resolve};
use crate::styles::{PillTone, pill_colors, ti};

/// Table row height of the Messages and ERC tables.
pub const TABLE_ROW_HEIGHT: f32 = 28.0;
/// Corner radius of a Properties field.
pub const PROPERTY_FIELD_RADIUS: f32 = 7.0;

/// Row look of the Projects tree: accent-soft selection, 24 px rows.
pub fn tree_style(tokens: &ThemeTokens) -> TreeStyle {
    let r = resolve(tokens);
    let tone = |t: PillTone| pill_colors(tokens, t);
    // The mock's violet has no token: a fixed pair per light / dark panel.
    let violet = if relative_luminance(ti(tokens.panel_bg)) < 0.5 {
        (
            Color::from_rgb8(0x2A, 0x21, 0x38),
            Color::from_rgb8(0xC9, 0xB3, 0xEE),
        )
    } else {
        (
            Color::from_rgb8(0xF1, 0xEA, 0xF8),
            Color::from_rgb8(0x6B, 0x3F, 0xA0),
        )
    };
    TreeStyle::new(r.accent_soft, r.accent_soft_ink).badge_tones([
        tone(PillTone::Neutral),
        tone(PillTone::Info),
        tone(PillTone::Ok),
        violet,
        tone(PillTone::Warn),
    ])
}

/// Short kind badge of a document row, with its tint: SCH info, PCB ok,
/// LIB violet, BOM warn, OUT neutral.
fn kind_badge(node: &TreeNode) -> Option<(&'static str, BadgeTone)> {
    let label = node.label.to_ascii_lowercase();
    let ext = |e: &str| label.contains(e);
    if ext(".snxbom") {
        return Some(("BOM", BadgeTone::Warn));
    }
    if ext(".snxout") {
        return Some(("OUT", BadgeTone::Neutral));
    }
    match node.icon {
        TreeIcon::SnxSchematic | TreeIcon::Schematic | TreeIcon::Sheet => {
            Some(("SCH", BadgeTone::Info))
        }
        TreeIcon::SnxPcb | TreeIcon::Pcb => Some(("PCB", BadgeTone::Ok)),
        TreeIcon::SnxLibrary | TreeIcon::SnxSymbol | TreeIcon::SnxFootprint => {
            Some(("LIB", BadgeTone::Violet))
        }
        _ => None,
    }
}

/// Copy of the project tree with Graphite badges: a count on each group
/// header (depth 1), a kind tag on each document row.
pub fn decorate_tree(roots: &[TreeNode]) -> Vec<TreeNode> {
    fn walk(node: &TreeNode, depth: usize) -> TreeNode {
        let mut out = node.clone();
        out.children = node.children.iter().map(|c| walk(c, depth + 1)).collect();
        if depth == 1 && node.is_folder {
            out.badge = Some(count_documents(node).to_string());
        } else if node.children.is_empty() && !node.is_folder {
            if let Some((tag, tone)) = kind_badge(node) {
                out.badge = Some(tag.to_string());
                out.badge_tone = tone;
            }
        }
        out
    }
    fn count_documents(node: &TreeNode) -> usize {
        node.children
            .iter()
            .map(|c| {
                if c.children.is_empty() && !c.is_folder {
                    1
                } else {
                    1 + count_documents(c)
                }
            })
            .sum()
    }
    roots.iter().map(|n| walk(n, 0)).collect()
}

/// Fill of the active / selected row in a library list: the selection
/// colour, or the soft accent on Graphite.
pub fn active_fill(tokens: &ThemeTokens) -> Color {
    if crate::styles::is_graphite(tokens) {
        resolve(tokens).accent_soft
    } else {
        ti(tokens.selection)
    }
}

/// Translucent state wash at `alpha`: white on legacy themes (their old
/// look), the text colour on Graphite so it also works on the light theme.
pub fn wash_fn(tokens: &ThemeTokens) -> impl Fn(f32) -> Color + Copy + 'static {
    let base = if crate::styles::is_graphite(tokens) {
        ti(tokens.text)
    } else {
        Color::WHITE
    };
    move |alpha| Color { a: alpha, ..base }
}

/// Corner radius of a small panel button: 7 on Graphite, 3 elsewhere.
pub fn button_radius(tokens: &ThemeTokens) -> f32 {
    if crate::styles::is_graphite(tokens) {
        PROPERTY_FIELD_RADIUS
    } else {
        3.0
    }
}
