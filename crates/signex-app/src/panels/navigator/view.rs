//! The Navigator panel (Graphite mockup #54): the Filter sheets field, the Sheets heading
//! with its count, then one row per visible sheet placement (sheet icon,
//! name, file name in muted mono, page number on the right). The focused
//! sheet gets the accent bar and fill; problem rows read in the error
//! colour, loose pages in muted italic. Row clicks and chevrons emit the
//! tree messages the dock handler already routes (`PanelMsg::Navigator`).

use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Column, Space, column, container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Theme, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::chips::{ChipMetrics, ChipTone, kind_chip};
use signex_widgets::tree_view::{TreeIcon, TreeMsg};
use signex_widgets::{text, text_input};

use super::{NavigatorMsg, NavigatorRow};
use crate::panels::graphite::selection_fill;
use crate::panels::{EmptyStateAction, PanelContext, PanelMsg, empty_state_card_with_actions};
use crate::styles::{ChromeMetrics, HAIRLINE, Radii, Spacing, TypeScale, ti, tokens::resolve};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
const CM: ChromeMetrics = ChromeMetrics::DEFAULT;
/// Indent per level of the hierarchy.
const INDENT: f32 = TS.body * 1.35;
/// Width of the page-number column.
const NUMBER_W: f32 = TS.body * 2.0;

fn msg(m: NavigatorMsg) -> PanelMsg {
    PanelMsg::NavigatorPanel(m)
}

/// Render the panel.
pub(crate) fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    let navigator = &ctx.navigator;
    if navigator.tree.is_empty() {
        return container(empty_state_card_with_actions(
            &ctx.tokens,
            t!(i18n, ids::PANELS_NAVIGATOR_NO_PROJECT),
            t!(i18n, ids::PANELS_NAVIGATOR_NO_PROJECT_BODY),
            None,
            vec![EmptyStateAction {
                label: t!(i18n, ids::PANELS_NAVIGATOR_OPEN_PROJECT),
                on_press: msg(NavigatorMsg::OpenProjectPressed),
                primary: true,
            }],
        ))
        .padding([SP.l, SP.l])
        .width(Length::Fill)
        .boxed();
    }
    let rows = navigator.visible_rows(i18n);
    let body: Element<'a, PanelMsg> = if rows.is_empty() {
        no_match(ctx)
    } else {
        let mut list: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
        for line in rows {
            list = list.push(sheet_row(ctx, line));
        }
        iced::widget::scrollable(list)
            .width(Length::Fill)
            .height(Length::Shrink)
            .style(crate::panels::form_look::scrollbar_style)
            .boxed()
    };
    let muted = resolve(&ctx.tokens).ink3;
    column![
        filter_field(ctx),
        heading(ctx),
        body,
        container(
            text(t!(i18n, ids::PANELS_NAVIGATOR_HINT))
                .size(TS.caption)
                .color(muted),
        )
        .padding([SP.s, SP.l]),
    ]
    .width(Length::Fill)
    .boxed()
}

fn filter_field<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    container(
        text_input(
            t!(ctx.i18n, ids::PANELS_NAVIGATOR_FILTER),
            &ctx.navigator.filter,
        )
        .on_input(|s| msg(NavigatorMsg::FilterChanged(s)))
        .style(crate::panels::form_look::text_input_style)
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .width(Length::Fill),
    )
    .padding(iced::Padding {
        top: SP.m,
        right: SP.l,
        bottom: SP.xs,
        left: SP.l,
    })
    .width(Length::Fill)
    .boxed()
}

/// "SHEETS" with the count of sheet placements as a pill.
fn heading<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let r = resolve(&ctx.tokens);
    let title = crate::panels::widgets::section_title(
        t!(ctx.i18n, ids::PANELS_NAVIGATOR_SHEETS),
        &ctx.tokens,
        &ctx.i18n,
    );
    let count = kind_chip(
        t!(ctx.i18n, ids::PANELS_NAVIGATOR_SHEET_COUNT, "count" => ctx.navigator.sheet_count()),
        ChipTone {
            fill: r.sunken,
            ink: r.ink3,
        },
        ChipMetrics::from_text_size(TS.body),
    );
    container(row![title, count].spacing(SP.s).align_y(Alignment::Center))
        .padding([SP.xs, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// One sheet placement. A click selects it (a second click opens it), the
/// chevron folds its branch.
fn sheet_row<'a>(ctx: &'a PanelContext, line: NavigatorRow) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let name_ink = if line.is_problem {
        ti(tokens.error)
    } else if line.is_loose {
        r.ink3
    } else {
        ti(tokens.text)
    };
    let name_font = if line.is_loose {
        Font {
            style: iced::font::Style::Italic,
            ..signex_widgets::ui_font::regular()
        }
    } else {
        signex_widgets::ui_font::regular()
    };
    let chevron: Element<'a, PanelMsg> = if line.has_children {
        signex_widgets::button(signex_widgets::chevron::chevron(
            line.is_expanded,
            TS.body,
            r.ink3,
        ))
        .padding(0)
        .on_press(PanelMsg::Navigator(TreeMsg::Toggle(line.key.clone())))
        .style(crate::styles::ghost_button(tokens))
        .boxed()
    } else {
        Space::new()
            .width(signex_widgets::chevron::side(TS.body))
            .height(Length::Shrink)
            .boxed()
    };
    let icon = svg(TreeIcon::Sheet.svg()).width(TS.body).height(TS.body);
    let number: signex_widgets::Label<'a> = match line.number {
        Some(number) => t!(ctx.i18n, ids::PANELS_NAVIGATOR_SHEET_NUMBER, "number" => number).into(),
        None => UserText::new(String::new()).into(),
    };
    let content = row![
        Space::new()
            .width(INDENT * line.depth as f32)
            .height(Length::Shrink),
        chevron,
        icon,
        // The name keeps its width and the file name takes the rest; each
        // ends in an ellipsis where the row runs out (#2999, mockup row 54),
        // so the page-number column stays where it is.
        container(
            row![
                text(UserText::new(line.label))
                    .size(TS.body)
                    .font(name_font)
                    .color(name_ink)
                    .wrapping(Wrapping::None)
                    .ellipsis(Ellipsis::End),
                text(UserText::new(line.file))
                    .size(TS.caption)
                    .font(mono)
                    .color(r.ink3)
                    .wrapping(Wrapping::None)
                    .ellipsis(Ellipsis::End)
                    .width(Length::Fill),
            ]
            .width(Length::Fill)
            .spacing(SP.s)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .clip(true),
        container(text(number).size(TS.caption).font(mono).color(r.ink3))
            .width(NUMBER_W)
            .align_x(iced::alignment::Horizontal::Right),
    ]
    .spacing(SP.xs)
    .height(CM.row)
    .align_y(Alignment::Center);
    let is_selected = ctx.navigator.selected.as_deref() == Some(line.path.as_slice());
    let fill = if line.is_active {
        Some(signex_widgets::theme_ext::accent_soft(tokens))
    } else if is_selected {
        Some(selection_fill(tokens))
    } else {
        None
    };
    let bar_ink = line.is_active.then(|| ti(tokens.accent));
    let pressed = signex_widgets::button(content)
        .width(Length::Fill)
        .padding([0.0, SP.s])
        .on_press(PanelMsg::Navigator(TreeMsg::Select(line.key)))
        .style(crate::styles::menu_item(tokens));
    let framed = row![accent_bar(bar_ink), pressed].align_y(Alignment::Center);
    container(framed)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            border: Border {
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The accent bar on the left of the focused sheet's row; a clear strip of
/// the same width on every other row so the rows line up.
fn accent_bar<'a>(ink: Option<Color>) -> Element<'a, PanelMsg> {
    container(Space::new())
        .width(2.0 * HAIRLINE)
        .height(CM.row)
        .style(move |_: &Theme| container::Style {
            background: ink.map(Background::Color),
            ..container::Style::default()
        })
        .boxed()
}

/// No row holds the filter text: say so, with Clear filter.
fn no_match<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n = &*ctx.i18n;
    container(empty_state_card_with_actions(
        &ctx.tokens,
        t!(i18n, ids::PANELS_NAVIGATOR_NO_MATCH),
        t!(
            i18n,
            ids::PANELS_NAVIGATOR_NO_MATCH_BODY,
            "filter" => ctx.navigator.filter.trim(),
            "total" => ctx.navigator.sheet_count()
        ),
        None,
        vec![EmptyStateAction {
            label: t!(i18n, ids::PANELS_NAVIGATOR_CLEAR_FILTER),
            on_press: msg(NavigatorMsg::FilterCleared),
            primary: false,
        }],
    ))
    .padding([SP.xs, SP.l])
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use iced::{Rectangle, Widget as _};
    use signex_types::schematic::InstancePath;

    use super::*;
    use crate::app::sheet_hierarchy::{SheetHierarchy, SheetNode, SheetNodeKind, normalised_key};
    use crate::layout_probe::LaidOutText;
    use crate::panels::navigator::NavigatorModel;

    const LONG_FILE: &str = "debug_board_with_a_very_long_sheet_file_name.snxsch";
    const LONG_NAME: &str = "Debug and bring-up connectors with a very long sheet name";
    /// A Navigator dock at its default width.
    const DOCK: iced::Size = iced::Size::new(240.0, 600.0);

    fn node(name: &str, file: &str) -> SheetNode {
        SheetNode {
            name: name.to_string(),
            filename: file.to_string(),
            placed_as: None,
            path: PathBuf::from("/proj").join(file),
            kind: SheetNodeKind::Present,
            detail: None,
            children: Vec::new(),
        }
    }

    /// Page 1 `Debug` with a long file name, page 2 `Notes` with a short
    /// one and page 3 with a long sheet name, both loose pages.
    fn ctx() -> PanelContext {
        let hierarchy = SheetHierarchy {
            fold: Default::default(),
            root: Some(node("Debug", LONG_FILE)),
            loose: vec![node("Notes", "n.snxsch"), node(LONG_NAME, "x.snxsch")],
        };
        let fold = hierarchy.fold;
        let key = |file: &str| normalised_key(&PathBuf::from("/proj").join(file), fold);
        let top = || InstancePath::from_chain(Vec::new());
        let mut model = NavigatorModel::default();
        model.replace_hierarchy(hierarchy, None, &[]);
        model.numbers = [
            ((key(LONG_FILE), top()), 1),
            ((key("n.snxsch"), top()), 2),
            ((key("x.snxsch"), top()), 3),
        ]
        .into_iter()
        .collect();
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_view.panel_ctx.clone();
        ctx.navigator = model;
        ctx
    }

    fn plain(text: &LaidOutText) -> String {
        text.text.replace(['\u{2068}', '\u{2069}'], "")
    }

    /// The text `label` laid out on the row of `beside` (the first match
    /// when `beside` is `None`).
    fn on_row(texts: &[LaidOutText], label: &str, beside: Option<Rectangle>) -> Rectangle {
        texts
            .iter()
            .filter(|t| plain(t) == label)
            .map(|t| t.bounds)
            .find(|b| beside.is_none_or(|row| (b.center_y() - row.center_y()).abs() < CM.row / 2.0))
            .unwrap_or_else(|| panic!("{label:?} in {texts:#?}"))
    }

    /// #2999, mockup row 54: in a dock narrower than the row, a long file
    /// name or sheet name is cut inside the row, before the page number, and
    /// the number column stays aligned on every row.
    #[test]
    fn a_long_name_is_cut_before_the_number_column() {
        let ctx = ctx();
        let texts = crate::layout_probe::lay_out_texts(view(&ctx), DOCK);
        let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
        let natural = crate::layout_probe::natural::<PanelMsg>(
            text(UserText::new(LONG_FILE))
                .size(TS.caption)
                .font(mono)
                .boxed(),
            LONG_FILE,
        );
        let file = on_row(&texts, LONG_FILE, None);
        assert!(
            natural.width > file.width,
            "the dock must be narrower than the file name: {file:?}, natural {natural:?}"
        );
        let short = on_row(&texts, "n.snxsch", None);
        let name = on_row(&texts, LONG_NAME, None);
        let numbers = [
            (file, on_row(&texts, "1", Some(file))),
            (short, on_row(&texts, "2", Some(short))),
            (name, on_row(&texts, "3", Some(name))),
        ];
        for (text, number) in numbers {
            assert!(
                text.x + text.width <= number.x + 0.5,
                "{text:?} runs into the number {number:?}"
            );
            assert!(number.x + number.width <= DOCK.width + 0.5, "{number:?}");
        }
        let right = |b: Rectangle| b.x + b.width;
        assert!(
            numbers
                .iter()
                .all(|(_, n)| (right(*n) - right(numbers[0].1)).abs() < 0.5),
            "the number column moved: {numbers:?}"
        );
    }

    /// The sheet name keeps its width and the file name follows it on the
    /// same row.
    #[test]
    fn the_file_name_follows_the_sheet_name() {
        let ctx = ctx();
        let texts = crate::layout_probe::lay_out_texts(view(&ctx), DOCK);
        let short = on_row(&texts, "n.snxsch", None);
        let notes = on_row(&texts, "Notes", Some(short));
        assert!(short.x > notes.x + notes.width, "{short:?} after {notes:?}");
        assert!(short.width > 0.0);
    }
}
