//! The Filter files field of the Projects panel, the filter itself, and the
//! card shown when it matches no file.

use iced::widget::{column, container, row, svg};
use iced::{Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_widgets::tree_view::TreeNode;
use signex_widgets::{text, text_input};

use super::super::graphite::hint_tooltip;
use super::super::{PanelContext, PanelMsg};
use super::card::icon_button;
use super::{ProjectsMsg, SP, TS};
use crate::styles::tokens::resolve;
use crate::styles::{self, ChromeMetrics, Radii, ti};

/// Side of the search glyph at the start of the field.
const SEARCH_GLYPH: f32 = 13.0;
/// Side of the clear (x) button inside the field, and of its glyph.
const CLEAR_BUTTON: f32 = 18.0;
const CLEAR_GLYPH: f32 = 12.0;
/// Gap between the field's glyph, text and clear button.
const FIELD_GAP: f32 = 6.0;
/// Padding inside the no-match card.
const HINT_PAD: f32 = 12.0;

/// The rows whose label contains `needle` (folded by `i18n.fold_for_search`),
/// each with the branches above it; a matching branch keeps all of its rows.
pub(super) fn filtered(i18n: &I18n, nodes: &[TreeNode], needle: &str) -> Vec<TreeNode> {
    nodes
        .iter()
        .filter_map(|node| {
            let label_matches = || i18n.fold_for_search(&node.label).contains(needle);
            if label_matches() && !node.children.is_empty() {
                return Some(node.clone());
            }
            if node.children.is_empty() {
                return (!node.is_folder && label_matches()).then(|| node.clone());
            }
            let kids = filtered(i18n, &node.children, needle);
            (!kids.is_empty()).then(|| {
                let mut out = node.clone();
                out.children = kids;
                out.expanded = true;
                out
            })
        })
        .collect()
}

/// The Filter files field: a search glyph and the text; while it holds
/// text, an accent border and a clear (x) button.
pub(super) fn filter_field<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let (tokens, i18n) = (&ctx.tokens, &*ctx.i18n);
    let r = resolve(tokens);
    let (ink, ink4) = (ti(tokens.text), r.ink4);
    let typed = !ctx.projects_view.filter.is_empty();
    let glyph = svg(crate::icons::icon_chrome_search(ctx.theme_id))
        .width(SEARCH_GLYPH)
        .height(SEARCH_GLYPH)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(if typed { ink } else { ink4 }),
            ..Default::default()
        });
    let input = text_input(
        t!(i18n, ids::PANELS_PROJECTS_FILTER),
        &ctx.projects_view.filter,
    )
    .on_input(|s| PanelMsg::Projects(ProjectsMsg::FilterChanged(s)))
    .padding(iced::Padding::ZERO)
    .size(TS.body)
    .width(Length::Fill)
    .style(move |_: &Theme, _| text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border::default(),
        placeholder: ink4,
        value: ink,
        selection: Color { a: 0.4, ..ink },
    });
    let mut line = row![glyph.boxed(), input.boxed()]
        .spacing(FIELD_GAP)
        .align_y(iced::Alignment::Center);
    if typed {
        let clear = icon_button(
            crate::icons::icon_close(ctx.theme_id),
            CLEAR_BUTTON,
            CLEAR_GLYPH,
            ti(tokens.text_secondary),
            PanelMsg::Projects(ProjectsMsg::FilterCleared),
            tokens,
        );
        line = line.push(hint_tooltip(
            clear,
            t!(i18n, ids::PANELS_PROJECTS_CLEAR_FILTER),
            tokens,
            iced::widget::tooltip::Position::Bottom,
        ));
    }
    container(line)
        .padding([0.0, SP.m])
        .height(ChromeMetrics::DEFAULT.field)
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill)
        .style(styles::field(tokens, typed))
        .boxed()
}

/// The card shown when the filter matches no file: what was typed, and how
/// to see every file again. iced draws no dashed border; the mock's dashed
/// edge is a solid `line_strong` hairline here.
pub(super) fn no_match_hint<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let (tokens, i18n) = (&ctx.tokens, &*ctx.i18n);
    let r = resolve(tokens);
    let filter = ctx.projects_view.filter.trim();
    let title = text(t!(i18n, ids::PANELS_PROJECTS_NO_MATCH_TITLE, "filter" => filter))
        .size(TS.body)
        .color(ti(tokens.text))
        .font(iced::Font {
            weight: iced::font::Weight::Medium,
            ..signex_widgets::ui_font::regular()
        })
        .align_x(iced::alignment::Horizontal::Center)
        .width(Length::Fill);
    let body = text(t!(i18n, ids::PANELS_PROJECTS_NO_MATCH_BODY))
        .size(TS.body)
        .color(r.ink3)
        .align_x(iced::alignment::Horizontal::Center)
        .width(Length::Fill);
    let line = r.line_strong;
    let card = container(
        column![title.boxed(), body.boxed()]
            .spacing(SP.s)
            .width(Length::Fill),
    )
    .padding(HINT_PAD)
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    container(card).padding(SP.m).width(Length::Fill).boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_widgets::tree_view::TreeIcon;

    fn labels(nodes: &[TreeNode]) -> Vec<&str> {
        nodes.iter().map(|n| n.label.as_str()).collect()
    }

    #[test]
    fn a_turkish_filter_finds_dotted_and_dotless_i_labels() {
        let tr = I18n::with_message_locale_for_test("tr");
        let nodes = [
            TreeNode::leaf("İSTANBUL.snxsch", TreeIcon::File),
            TreeNode::leaf("ISPARTA.snxsch", TreeIcon::File),
        ];
        assert_eq!(
            labels(&filtered(&tr, &nodes, &tr.fold_for_search("istanbul"))),
            ["İSTANBUL.snxsch"]
        );
        assert_eq!(
            labels(&filtered(&tr, &nodes, &tr.fold_for_search("ısparta"))),
            ["ISPARTA.snxsch"]
        );
        assert!(filtered(&tr, &nodes, &tr.fold_for_search("isparta")).is_empty());
    }

    #[test]
    fn en_us_is_unchanged() {
        let en = I18n::en_us();
        let nodes = [TreeNode::leaf("Main.snxsch", TreeIcon::File)];
        assert_eq!(
            labels(&filtered(&en, &nodes, &en.fold_for_search("MAIN"))).len(),
            1
        );
        assert!(filtered(&en, &nodes, &en.fold_for_search("pcb")).is_empty());
    }

    /// A two-sheet project whose channel sheet sits under its top sheet, as
    /// `build_project_tree` shapes a hierarchy.
    fn two_sheet_project() -> Vec<TreeNode> {
        let channel = TreeNode::leaf("channel.snxsch", TreeIcon::SnxSchematic);
        let top = TreeNode::branch("top.snxsch", TreeIcon::SnxSchematic, vec![channel]);
        let source = TreeNode::branch(super::super::SOURCE, TreeIcon::Folder, vec![top]);
        vec![TreeNode::branch("dcdc-buck", TreeIcon::Folder, vec![source]).with_key("dcdc-buck")]
    }

    fn grouped(roots: &[TreeNode]) -> Vec<TreeNode> {
        super::super::regroup(roots, &super::super::ProjectsView::default())
    }

    /// Mockup row 19: a filtered sheet row keeps its page position, counted
    /// over the whole hierarchy, not over the rows the filter left.
    #[test]
    fn filtered_sheet_rows_keep_their_page_position() {
        let en = I18n::en_us();
        let shown = filtered(&en, &grouped(&two_sheet_project()), "channel");
        let top = &shown[0].children[0].children[0];
        assert_eq!(top.label, "top.snxsch");
        assert_eq!(top.meta.as_deref(), Some("1/2"));
        assert_eq!(top.children[0].meta.as_deref(), Some("2/2"));
    }

    /// The flat tree of mockup row 5 numbers its sheets the same way, and a
    /// lone sheet shows no position.
    #[test]
    fn flat_sheets_are_numbered_and_a_lone_sheet_is_not() {
        let flat = |names: &[&str]| {
            let kids = names
                .iter()
                .map(|n| TreeNode::leaf(*n, TreeIcon::SnxSchematic))
                .collect();
            let source = TreeNode::branch(super::super::SOURCE, TreeIcon::Folder, kids);
            grouped(&[TreeNode::branch("p", TreeIcon::Folder, vec![source]).with_key("p")])
        };
        let two = flat(&["a.snxsch", "b.snxsch"]);
        let metas: Vec<_> = two[0].children[0]
            .children
            .iter()
            .map(|n| n.meta.as_deref())
            .collect();
        assert_eq!(metas, [Some("1/2"), Some("2/2")]);
        let one = flat(&["a.snxsch"]);
        assert_eq!(one[0].children[0].children[0].meta, None);
    }

    /// The panel draws both positions right-aligned on their rows while
    /// the filter shows only the channel sheet and its parent.
    #[test]
    fn the_filtered_panel_draws_right_aligned_positions() {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_view.panel_ctx.clone();
        ctx.project_tree = two_sheet_project();
        ctx.projects_view.filter = "channel".to_string();
        let shown = crate::layout_probe::lay_out_texts(
            super::super::view(&ctx),
            iced::Size::new(360.0, 900.0),
        );
        let find = |s: &str| {
            shown
                .iter()
                .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == s)
                .unwrap_or_else(|| panic!("{s} is not drawn"))
                .bounds
        };
        let (top, channel) = (find("top.snxsch"), find("channel.snxsch"));
        let (first, second) = (find("1/2"), find("2/2"));
        assert!(first.x > top.x + top.width && second.x > channel.x + channel.width);
        assert!((first.center_y() - top.center_y()).abs() < top.height);
        assert!((second.center_y() - channel.center_y()).abs() < channel.height);
        let right = |r: iced::Rectangle| r.x + r.width;
        assert!((right(first) - right(second)).abs() < 0.5);
    }
}
