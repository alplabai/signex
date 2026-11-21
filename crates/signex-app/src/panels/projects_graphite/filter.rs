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
            ..iced::Font::DEFAULT
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
}
