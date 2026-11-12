//! Graphite looks of the panel contents: the project tree rows, the
//! panel buttons and list rows.

use iced::widget::{column, container, row};
use iced::{Border, Color, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;
use signex_widgets::tree_view::{BadgeTone, TreeIcon, TreeNode, TreeStyle};

use super::{ErcSeverityLite, PanelContext, PanelMsg};
use crate::styles::tokens::{relative_luminance, resolve};
use crate::styles::{PillTone, Radii, TypeScale, pill_colors, ti};

const TS: TypeScale = TypeScale::DEFAULT;

/// Table row height of the Messages and ERC tables.
pub const TABLE_ROW_HEIGHT: f32 = 28.0;
/// Corner radius of a Properties field.
pub const PROPERTY_FIELD_RADIUS: f32 = 7.0;

/// Row look of the Projects tree: the neutral selection, 24 px rows.
pub fn tree_style(tokens: &ThemeTokens) -> TreeStyle {
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
    TreeStyle::new(selection_fill(tokens), ti(tokens.text)).badge_tones([
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
    // File-extension match, not UI text: ASCII lowercase in every language.
    // ast-grep-ignore: no-case-in-view
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
        } else if node.children.is_empty()
            && !node.is_folder
            && let Some((tag, tone)) = kind_badge(node)
        {
            out.badge = Some(tag.to_string());
            out.badge_tone = tone;
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

/// Fill of the active row of the PCB Layers list: the soft accent.
pub fn active_fill(tokens: &ThemeTokens) -> Color {
    resolve(tokens).accent_soft
}

/// Fill of the selected row in a tree or library list (Projects, SCH
/// Library, Footprint Library): the theme's neutral selection, as in the
/// Library Browser, not an accent tint.
pub fn selection_fill(tokens: &ThemeTokens) -> Color {
    ti(tokens.selection)
}

/// Translucent state wash at `alpha`: the text colour, so it also works on
/// the light theme.
pub fn wash_fn(tokens: &ThemeTokens) -> impl Fn(f32) -> Color + Copy + 'static {
    let base = ti(tokens.text);
    move |alpha| Color { a: alpha, ..base }
}

/// Corner radius of a small panel button.
pub fn button_radius(_tokens: &ThemeTokens) -> f32 {
    PROPERTY_FIELD_RADIUS
}

/// "2 h ago" style age of a commit.
fn age(i18n: &I18n, time: chrono::DateTime<chrono::Utc>) -> Localized {
    let secs = (chrono::Utc::now() - time).num_seconds().max(0);
    match secs {
        0..=59 => t!(i18n, ids::PANELS_SUMMARY_AGE_NOW),
        60..=3599 => t!(i18n, ids::PANELS_SUMMARY_AGE_MINUTES, "count" => secs / 60),
        3600..=86399 => t!(i18n, ids::PANELS_SUMMARY_AGE_HOURS, "count" => secs / 3600),
        _ => t!(i18n, ids::PANELS_SUMMARY_AGE_DAYS, "count" => secs / 86400),
    }
}

/// One cell of the summary grid: a caption over a value.
fn summary_cell<'a>(
    tokens: &ThemeTokens,
    caption: Localized,
    value: Localized,
    color: Color,
) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    column![
        text(caption).size(TS.caption).color(r.ink4).boxed(),
        text(value)
            .size(TS.caption)
            .color(color)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..iced::Font::DEFAULT
            })
            .boxed(),
    ]
    .spacing(2)
    .width(Length::FillPortion(1))
    .boxed()
}

/// The Projects panel footer card: ERC counts, DRC and "Last commit".
/// ERC and DRC show a dash without data (no ERC findings recorded, no DRC
/// engine). "Unrouted" is left out: no routing status exists to count.
/// "Last commit" shows only when the active project has git and its history
/// has loaded.
pub(crate) fn summary_card<'a>(ctx: &PanelContext) -> Option<Element<'a, PanelMsg>> {
    let tokens = &ctx.tokens;
    let i18n = &*ctx.i18n;
    let r = resolve(tokens);
    let count = |severity| {
        ctx.erc_diagnostics
            .iter()
            .filter(|d| d.severity == severity)
            .count()
    };
    let (erc, erc_color) = if ctx.erc_diagnostics.is_empty() {
        (t!(i18n, ids::PANELS_SUMMARY_NO_VALUE), r.ink3)
    } else {
        let errors = count(ErcSeverityLite::Error);
        let warnings = count(ErcSeverityLite::Warning);
        let color = if errors > 0 {
            ti(tokens.error)
        } else if warnings > 0 {
            ti(tokens.warning)
        } else {
            r.ink3
        };
        (
            t!(
                i18n,
                ids::PANELS_SUMMARY_ERC_COUNTS,
                "errors" => errors,
                "warnings" => warnings
            ),
            color,
        )
    };
    let mut cells = vec![
        summary_cell(tokens, t!(i18n, ids::PANELS_SUMMARY_ERC), erc, erc_color),
        summary_cell(
            tokens,
            t!(i18n, ids::PANELS_SUMMARY_DRC),
            t!(i18n, ids::PANELS_SUMMARY_NO_VALUE),
            r.ink3,
        ),
    ];
    let has_git = ctx
        .projects
        .iter()
        .any(|p| p.is_active && p.version_control);
    if has_git
        && ctx.history.mode == super::history::HistoryRenderMode::Ready
        && let Some(last) = ctx.history.entries.first()
    {
        cells.push(summary_cell(
            tokens,
            t!(i18n, ids::PANELS_SUMMARY_LAST_COMMIT),
            age(i18n, last.time),
            r.ink3,
        ));
    }
    let mut grid: iced::widget::Column<Element<'a, PanelMsg>> =
        iced::widget::Column::new().spacing(8).width(Length::Fill);
    let mut cells = cells.into_iter();
    while let Some(first) = cells.next() {
        let second = cells.next().unwrap_or_else(|| {
            iced::widget::space::horizontal()
                .width(Length::FillPortion(1))
                .boxed()
        });
        let line: iced::widget::Row<Element<'a, PanelMsg>> =
            row![first, second].spacing(10).width(Length::Fill);
        grid = grid.push(line.boxed());
    }
    let (fill, edge) = (ti(tokens.toolbar_bg), ti(tokens.border));
    Some(
        container(grid)
            .padding(10)
            .width(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(fill)),
                border: Border {
                    width: 1.0,
                    color: edge,
                    radius: Radii::GRAPHITE.card.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed(),
    )
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};
    use signex_widgets::tree_view::TreeView;

    const THEMES: [ThemeId; 2] = [ThemeId::GraphiteLight, ThemeId::GraphiteDark];

    /// #2262: the Projects tree and the SCH / Footprint Library rows select
    /// with the neutral grey the Library Browser uses, not the accent tint.
    #[test]
    fn tree_and_library_rows_select_with_the_neutral_grey() {
        for id in THEMES {
            let t = theme_tokens(id);
            let neutral = ti(t.selection);
            assert_ne!(neutral, resolve(&t).accent_soft, "{id:?}");
            assert_eq!(selection_fill(&t), neutral, "{id:?} library rows");
            let style = tree_style(&t);
            assert_eq!(style.selected_fill, neutral, "{id:?} tree row");
            assert_eq!(style.selected_ink, ti(t.text), "{id:?} tree ink");
        }
    }

    /// The selected Projects row is drawn in that grey.
    #[test]
    fn the_selected_tree_row_is_drawn_in_the_neutral_grey() {
        for id in THEMES {
            let t = theme_tokens(id);
            let roots = [TreeNode::leaf("a.snxsch", TreeIcon::SnxSchematic).with_key("a")];
            let key = ["a".to_string()];
            let tree = TreeView::new(&roots, &t)
                .style(tree_style(&t))
                .selected(&key)
                .view();
            let (w, h) = (200usize, 24usize);
            let pixels =
                crate::layout_probe::draw_pixels(tree, iced::Size::new(w as f32, h as f32));
            // Middle of the row, right of the label and clear of the corners.
            let at = (h / 2 * w + w * 3 / 4) * 4;
            let drawn = [pixels[at], pixels[at + 1], pixels[at + 2]];
            let n = ti(t.selection);
            let want = [n.r, n.g, n.b].map(|v| (v * 255.0).round() as u8);
            for (d, e) in drawn.iter().zip(want) {
                assert!(
                    d.abs_diff(e) <= 2,
                    "{id:?}: row {drawn:?}, selection {want:?}"
                );
            }
        }
    }
}
