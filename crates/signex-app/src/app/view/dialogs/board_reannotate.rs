//! The Positional Re-Annotate dialog (#1312, #2533): two Order cards with a
//! walk diagram each, the Bottom side segmented control, the preview list
//! (renamed parts, then kept ones with their reason, then the parts already
//! in order when asked) under a heading that carries the summary, the
//! Update PCB banner, and the footer. State and rules:
//! `app/board_reannotate/`.

use iced::widget::{Column, Space, canvas, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_engine::pcb::designators::{BottomSideOrder, Kept, KeptReason, ReannotateOrder};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{
    CheckboxLabel as _, Label, button, checkbox, rich_text, segmented, span, text,
};

use super::sizes;
use crate::app::board_reannotate::preview::{PreviewRow, rows};
use crate::app::{BoardReannotateMsg, BoardReannotateState, Message, ReannotateProblem, Signex};
use crate::styles::{
    DialogButtonKind, PillTone, Radii, Spacing, TypeScale, dialog_button, dialog_footer_split, ti,
    tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Width of the designator column of the preview.
const COLUMN_W: f32 = TS.body * 10.0;
/// Width of the arrow column of the preview.
const ARROW_W: f32 = TS.body * 2.5;
/// The walk diagram: the mockup's 52 x 38 at the 12 px body.
const DIAGRAM_W: f32 = TS.body * 52.0 / 12.0;
const DIAGRAM_H: f32 = TS.body * 38.0 / 12.0;
/// Side of an option card's radio mark.
const RADIO: f32 = TS.body * 14.0 / 12.0;
/// Width of the Bottom side column: the mockup's 150 at the 12 px body.
const BOTTOM_W: f32 = TS.body * 150.0 / 12.0;

fn ra(msg: BoardReannotateMsg) -> Message {
    Message::from(msg)
}

fn s(i18n: &I18n, id: MessageId) -> String {
    t!(i18n, id).into_string()
}

impl Signex {
    pub(in crate::app::view) fn view_board_reannotate_dialog(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(self.view_board_reannotate_card(), sizes::BOARD_REANNOTATE)
    }

    fn view_board_reannotate_card(&self) -> Element<'_, Message> {
        let Some(state) = self.ui_state.board_reannotate.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_view.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let cancel = Some(ra(BoardReannotateMsg::CancelRequested));
        let header = super::header::graphite_header_closing(
            s(i18n, ids::PCB_REANNOTATE_TITLE),
            Some(
                t!(i18n, ids::PCB_REANNOTATE_SUBTITLE, "board" => state.board_name.as_str())
                    .into_string(),
            ),
            None,
            cancel.clone(),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let body = container(body(state, i18n, tokens))
            .padding([SP.l, SP.xl])
            .height(Length::Fill);
        let renamed = state.plan.renames.len();
        let footer = dialog_footer_split(
            vec![
                text(t!(i18n, ids::PCB_REANNOTATE_FOOTER_NOTE))
                    .size(TS.body)
                    .color(tokens::muted_text(tokens))
                    .boxed(),
            ],
            vec![
                dialog_button(
                    t!(i18n, ids::PCB_REANNOTATE_CANCEL),
                    DialogButtonKind::Cancel,
                    cancel,
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::PCB_REANNOTATE_EXECUTE_COUNT, "count" => renamed),
                    super::graphite::call_to_action(tokens),
                    state
                        .can_execute()
                        .then(|| ra(BoardReannotateMsg::ExecuteRequested)),
                    tokens,
                ),
            ],
        );
        container(column![header, body, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, sizes::BOARD_REANNOTATE.0),
        )))
        .height(Length::Fixed(sizes::BOARD_REANNOTATE.1))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}

/// The intro, the problem if any, the Order cards, the Bottom side
/// control, the preview and the Update PCB banner.
fn body<'a>(
    state: &'a BoardReannotateState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let muted = tokens::muted_text(tokens);
    let options = state.options;
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.l).height(Length::Fill);
    col = col.push(
        text(t!(i18n, ids::PCB_REANNOTATE_INTRO))
            .size(TS.body)
            .color(muted)
            .boxed(),
    );
    if let Some(problem) = state.problem.as_ref() {
        col = col.push(
            text(problem_text(problem, i18n))
                .size(TS.body)
                .color(tokens::danger(tokens))
                .boxed(),
        );
    }
    let cards = Column::new()
        .spacing(SP.s)
        .push(section_label(t!(i18n, ids::PCB_REANNOTATE_ORDER), tokens))
        .push(order_card(
            ReannotateOrder::XThenY,
            options.order,
            (
                t!(i18n, ids::PCB_REANNOTATE_ORDER_COLUMNS),
                t!(i18n, ids::PCB_REANNOTATE_ORDER_COLUMNS_HINT),
            ),
            tokens,
        ))
        .push(order_card(
            ReannotateOrder::YThenX,
            options.order,
            (
                t!(i18n, ids::PCB_REANNOTATE_ORDER_ROWS),
                t!(i18n, ids::PCB_REANNOTATE_ORDER_ROWS_HINT),
            ),
            tokens,
        ));
    // Bottom side sits in a column beside the Order cards (mockup row 83).
    col = col.push(
        row![
            cards.width(Length::Fill).boxed(),
            bottom_side(options.bottom, i18n, tokens)
        ]
        .spacing(SP.l)
        .boxed(),
    );
    col = col.push(preview(state, i18n, tokens));
    col = col.push(info_banner(i18n, tokens));
    col.boxed()
}

fn section_label<'a>(label: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(label)
        .size(TS.label)
        .color(tokens::muted_text(tokens))
        .boxed()
}

/// One Order card: the radio mark, the name and its hint, and the walk
/// diagram on the right.
fn order_card<'a>(
    order: ReannotateOrder,
    picked: ReannotateOrder,
    (name, hint): (Localized, Localized),
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let on = order == picked;
    let words = column![
        text(name).size(TS.body).font(iced::Font {
            weight: iced::font::Weight::Semibold,
            ..signex_widgets::ui_font::regular()
        }),
        text(hint)
            .size(TS.caption)
            .color(tokens::muted_text(tokens)),
    ]
    .spacing(SP.xs)
    .width(Length::Fill);
    let content = row![radio_mark(on, tokens), words, walk_diagram(order, tokens)]
        .spacing(SP.m)
        .align_y(Alignment::Center);
    button(content)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .on_press(ra(BoardReannotateMsg::OrderPicked(order)))
        .style(super::graphite_annotate::toggle_style(tokens, on))
        .boxed()
}

/// A radio mark: an accent ring with a dot when picked, a hairline ring
/// otherwise.
pub(super) fn radio_mark<'a>(on: bool, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let accent = tokens::accent(tokens);
    let edge = if on { accent } else { r.line_strong };
    let dot_side = RADIO / 2.0;
    let dot: Element<'a, Message> = if on {
        container(Space::new())
            .width(dot_side)
            .height(dot_side)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(accent)),
                border: Border {
                    radius: (dot_side / 2.0).into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed()
    } else {
        Space::new().boxed()
    };
    let field = r.field;
    container(dot)
        .center(RADIO)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(field)),
            border: Border {
                width: 1.0,
                color: edge,
                radius: (RADIO / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// Canvas of an Order card: three dashed lanes and the accent arrows that
/// walk them, down the columns (X then Y) or along the rows (Y then X).
struct WalkDiagram {
    order: ReannotateOrder,
    lane: iced::Color,
    accent: iced::Color,
}

/// Dash pattern of the lanes, in canvas units of the 64 x 48 drawing.
const LANE_DASH: &[f32] = &[2.0, 3.0];

impl<M> canvas::Program<M> for WalkDiagram {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        // The drawing is 64 x 48 units, scaled to the canvas.
        let (sx, sy) = (bounds.width / 64.0, bounds.height / 48.0);
        let at = |x: f32, y: f32| iced::Point::new(x * sx, y * sy);
        let stroke = |color: iced::Color, dashed: bool| {
            let mut s = canvas::Stroke::default().with_color(color).with_width(1.3);
            if dashed {
                s.line_dash = canvas::LineDash {
                    segments: LANE_DASH,
                    offset: 0,
                };
            }
            s
        };
        for lane in [14.0, 32.0, 50.0] {
            let (lane_from, lane_to, walk_to, head) = match self.order {
                ReannotateOrder::XThenY => (at(lane, 8.0), at(lane, 40.0), at(lane, 36.0), 0),
                ReannotateOrder::YThenX => {
                    let y = lane * 0.75 - 2.5;
                    (at(8.0, y), at(52.0, y), at(52.0, y), 1)
                }
            };
            frame.stroke(
                &canvas::Path::line(lane_from, lane_to),
                stroke(self.lane, true),
            );
            frame.stroke(
                &canvas::Path::line(lane_from, walk_to),
                stroke(self.accent, false),
            );
            let (tip, a, b) = if head == 0 {
                (
                    walk_to,
                    iced::Point::new(walk_to.x - 3.0 * sx, walk_to.y - 4.0 * sy),
                    iced::Point::new(walk_to.x + 3.0 * sx, walk_to.y - 4.0 * sy),
                )
            } else {
                (
                    walk_to,
                    iced::Point::new(walk_to.x - 4.0 * sx, walk_to.y - 3.0 * sy),
                    iced::Point::new(walk_to.x - 4.0 * sx, walk_to.y + 3.0 * sy),
                )
            };
            let arrow = canvas::Path::new(|p| {
                p.move_to(a);
                p.line_to(tip);
                p.line_to(b);
            });
            frame.stroke(&arrow, stroke(self.accent, false));
        }
        vec![frame.into_geometry()]
    }
}

fn walk_diagram<'a>(order: ReannotateOrder, tokens: &ThemeTokens) -> Element<'a, Message> {
    let diagram: canvas::Canvas<WalkDiagram, Message> = canvas::Canvas::new(WalkDiagram {
        order,
        lane: tokens::resolve(tokens).ink3,
        accent: tokens::accent(tokens),
    });
    container(
        diagram
            .width(Length::Fixed(DIAGRAM_W))
            .height(Length::Fixed(DIAGRAM_H)),
    )
    .style(diagram_tile(tokens))
    .boxed()
}

/// The walk diagram's tile: the field fill in a hairline border with the
/// small radius (mockup row 83).
fn diagram_tile(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let (fill, edge) = (tokens::resolve(tokens).field, ti(tokens.border));
    move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: 1.0,
            color: edge,
            radius: Radii::GRAPHITE.small.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Bottom side: After the top side / With the top side.
fn bottom_side<'a>(
    bottom: BottomSideOrder,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let style = r.segmented(TS.body);
    let options: [(BottomSideOrder, Label<'a>); 2] = [
        (
            BottomSideOrder::AfterTop,
            t!(i18n, ids::PCB_REANNOTATE_BOTTOM_AFTER).into(),
        ),
        (
            BottomSideOrder::WithTop,
            t!(i18n, ids::PCB_REANNOTATE_BOTTOM_WITH).into(),
        ),
    ];
    // The control stacks the two choices only because BOTTOM_W is narrower
    // than both labels side by side; SegmentedStyle has no column count, so
    // a locale with short labels would put them in one row.
    let control = segmented(options, bottom, style.fill_width(true), |picked| {
        ra(BoardReannotateMsg::BottomPicked(picked))
    });
    column![
        section_label(t!(i18n, ids::PCB_REANNOTATE_BOTTOM), tokens),
        control,
    ]
    .spacing(SP.s)
    .width(Length::Fixed(BOTTOM_W))
    .boxed()
}

/// The preview: its heading with the summary and the Show already in
/// order box, then the list in a hairline frame.
fn preview<'a>(
    state: &'a BoardReannotateState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let muted = tokens::muted_text(tokens);
    let plan = &state.plan;
    let summary = if plan.renames.is_empty() {
        t!(i18n, ids::PCB_REANNOTATE_NOTHING)
    } else {
        t!(
            i18n,
            ids::PCB_REANNOTATE_SUMMARY,
            "renamed" => plan.renames.len(),
            "unchanged" => plan.unchanged,
            "kept" => plan.kept.len()
        )
    };
    let toggle = checkbox(state.show_in_order)
        .size(TS.body)
        .labelled(t!(i18n, ids::PCB_REANNOTATE_SHOW_IN_ORDER))
        .text_size(TS.caption)
        .style(crate::panels::form_look::checkbox_style)
        .on_toggle(|show| ra(BoardReannotateMsg::InOrderShown(show)));
    let heading = row![
        section_label(t!(i18n, ids::PCB_REANNOTATE_PREVIEW), tokens),
        text(summary).size(TS.caption).color(muted),
        Space::new().width(Length::Fill),
        toggle,
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let cell = |label: Label<'a>, color, mono: bool| {
        let t = text(label).size(TS.body).color(color);
        if mono {
            t.font(super::graphite::mono())
        } else {
            t
        }
    };
    let mut list: Column<Element<'a, Message>> = Column::new().spacing(SP.xs).push(
        row![
            cell(t!(i18n, ids::PCB_REANNOTATE_COL_FROM).into(), muted, false).width(COLUMN_W),
            Space::new().width(ARROW_W),
            cell(t!(i18n, ids::PCB_REANNOTATE_COL_TO).into(), muted, false),
        ]
        .boxed(),
    );
    let ink = ti(tokens.text);
    let accent = tokens::accent(tokens);
    for line in rows(state) {
        let (from, to): (Label<'a>, Element<'a, Message>) = match line {
            PreviewRow::Renamed(rename) => (
                UserText::new(rename.from.clone()).into(),
                cell(UserText::new(rename.to.clone()).into(), accent, true).boxed(),
            ),
            PreviewRow::Kept(kept) => (
                UserText::new(kept.reference.clone()).into(),
                cell(kept_text(kept, i18n).into(), muted, false).boxed(),
            ),
            PreviewRow::InOrder(designator) => (
                UserText::new(designator.to_string()).into(),
                cell(UserText::new(designator.to_string()).into(), muted, true).boxed(),
            ),
        };
        list = list.push(
            row![
                cell(from, ink, true).width(COLUMN_W),
                text(t!(i18n, ids::PCB_REANNOTATE_ARROW))
                    .size(TS.body)
                    .color(muted)
                    .width(ARROW_W),
                to,
            ]
            .align_y(Alignment::Center)
            .boxed(),
        );
    }
    let r = tokens::resolve(tokens);
    let (fill, edge) = (r.field, r.line_soft);
    let frame = container(
        scrollable(container(list).padding([SP.s, SP.m]))
            .direction(crate::styles::thin_vertical())
            .style(crate::styles::scrollbar_style(tokens))
            .height(Length::Fill),
    )
    .height(Length::Fill)
    .clip(true)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: 1.0,
            color: edge,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    column![heading, frame]
        .spacing(SP.s)
        .height(Length::Fill)
        .boxed()
}

/// The info banner naming Back Annotate as the follow-up step: the info
/// glyph and the sentence, the command in medium weight, on the soft info
/// fill with no outline (mockup row 83).
fn info_banner<'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let ink = ti(tokens.text);
    let spans: Vec<_> = banner_runs(i18n)
        .into_iter()
        .map(|(run, strong)| {
            // Runs of the already rendered catalog sentence: shown as they are.
            let s = span::<()>(UserText::new(run)).color(ink);
            if strong {
                s.font(super::board_output::kit::medium())
            } else {
                s
            }
        })
        .collect();
    let words = rich_text::<(), Message, Theme>(spans).size(TS.body).boxed();
    super::board_output::kit::banner(PillTone::Info, words, None, tokens)
}

/// The banner sentence as runs, the command's name flagged for emphasis.
/// Fluent wraps the name in bidi isolation marks, which locate it.
fn banner_runs(i18n: &I18n) -> Vec<(String, bool)> {
    let command = t!(i18n, ids::PCB_REANNOTATE_BACK_ANNOTATE).into_string();
    let sentence =
        t!(i18n, ids::PCB_DESIGNATOR_UPDATE_PCB_NOTE, "command" => command.as_str()).into_string();
    let split = sentence
        .split_once('\u{2068}')
        .and_then(|(before, rest)| rest.split_once('\u{2069}').map(|(n, a)| (before, n, a)));
    let Some((before, name, after)) = split else {
        return vec![(sentence, false)];
    };
    [(before, false), (name, true), (after, false)]
        .into_iter()
        .filter(|(run, _)| !run.is_empty())
        .map(|(run, strong)| (run.to_owned(), strong))
        .collect()
}

fn problem_text(problem: &ReannotateProblem, i18n: &I18n) -> Localized {
    match problem {
        ReannotateProblem::ReadOnly => t!(i18n, ids::PCB_REANNOTATE_READ_ONLY),
        ReannotateProblem::Stale => t!(i18n, ids::PCB_REANNOTATE_STALE),
        ReannotateProblem::Refused(why) => {
            t!(i18n, ids::PCB_REANNOTATE_REFUSED, "why" => why.as_str())
        }
    }
}

fn kept_text(kept: &Kept, i18n: &I18n) -> Localized {
    let designator = kept.reference.as_str();
    match kept.reason {
        KeptReason::Locked => {
            t!(i18n, ids::PCB_REANNOTATE_KEPT_LOCKED, "designator" => designator)
        }
        KeptReason::NoPrefix if designator.trim().is_empty() => {
            t!(i18n, ids::PCB_REANNOTATE_KEPT_EMPTY)
        }
        KeptReason::NoPrefix => {
            t!(i18n, ids::PCB_REANNOTATE_KEPT_NO_PREFIX, "designator" => designator)
        }
        KeptReason::Unlinked => {
            t!(i18n, ids::PCB_REANNOTATE_KEPT_UNLINKED, "designator" => designator)
        }
    }
}

#[cfg(test)]
mod tests {
    use signex_engine::pcb::designators::{ReannotateOptions, ReannotatePlan};

    use crate::app::{BoardReannotateState, Signex};

    const WINDOW: iced::Size = iced::Size::new(1400.0, 900.0);

    /// Mockup row 83: the title matches the Tools menu row, Re-Annotate
    /// Board, and the subtitle keeps the word Positional before the board.
    #[test]
    fn the_title_is_the_menu_name_and_the_subtitle_keeps_positional() {
        let (mut app, _boot) = Signex::new();
        app.ui_state.i18n = signex_i18n::I18n::en_us();
        app.ui_state.board_reannotate = Some(BoardReannotateState {
            board: "dcdc-4ch.snxpcb".into(),
            board_name: "dcdc-4ch.snxpcb".into(),
            options: ReannotateOptions::default(),
            plan: ReannotatePlan::default(),
            revision: 1,
            problem: None,
            in_order: Vec::new(),
            show_in_order: false,
        });
        let shown: Vec<String> =
            crate::layout_probe::lay_out_texts(app.view_board_reannotate_card(), WINDOW)
                .into_iter()
                .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
                .collect();
        assert!(shown.iter().any(|t| t == "Re-Annotate Board"), "{shown:?}");
        assert!(
            shown
                .iter()
                .any(|t| t == "Positional \u{00B7} dcdc-4ch.snxpcb"),
            "{shown:?}"
        );
        assert!(
            !shown.iter().any(|t| t == "Positional Re-Annotate"),
            "{shown:?}"
        );
    }

    fn open_dialog() -> Signex {
        let (mut app, _boot) = Signex::new();
        app.ui_state.i18n = signex_i18n::I18n::en_us();
        app.ui_state.board_reannotate = Some(BoardReannotateState {
            board: "dcdc-4ch.snxpcb".into(),
            board_name: "dcdc-4ch.snxpcb".into(),
            options: ReannotateOptions::default(),
            plan: ReannotatePlan::default(),
            revision: 1,
            problem: None,
            in_order: Vec::new(),
            show_in_order: false,
        });
        app
    }

    /// Mockup row 83: Bottom side is a column beside the Order cards, its
    /// heading level with Order and its two choices stacked, above the
    /// preview.
    #[test]
    fn bottom_side_sits_beside_the_order_cards_with_its_choices_stacked() {
        use crate::layout_probe::{find, lay_out_texts};
        let app = open_dialog();
        let texts = lay_out_texts(app.view_board_reannotate_card(), WINDOW);
        let at = |label: &str| find(&texts, label).bounds;
        let (order, card) = (at("Order"), at("By X, then by Y"));
        let (bottom, after, with) = (
            at("Bottom side"),
            at("After the top side"),
            at("With the top side"),
        );
        assert!(bottom.x > card.x + card.width, "{bottom:?} {card:?}");
        assert!((bottom.y - order.y).abs() < 2.0, "{bottom:?} {order:?}");
        assert!(with.y >= after.y + after.height, "{after:?} {with:?}");
        assert!(at("Preview").y > with.y + with.height, "{with:?}");
    }

    /// Mockup row 83: the banner names Back Annotate in its own run, the
    /// one drawn in medium weight.
    #[test]
    fn the_banner_sets_back_annotate_apart() {
        let i18n = signex_i18n::I18n::en_us();
        assert_eq!(
            super::banner_runs(&i18n),
            vec![
                (
                    "Update PCB renames parts back to the schematic's designators; use ".into(),
                    false
                ),
                ("Back Annotate".into(), true),
                (" to keep them.".into(), false),
            ]
        );
    }

    /// Mockup row 83: the banner is the soft info fill with no outline and
    /// an info glyph, not the text glyph the font may lack.
    #[test]
    fn the_banner_is_a_soft_fill_with_no_outline() {
        use crate::styles::tokens::composite_over;
        use signex_types::theme::{ThemeId, theme_tokens};
        let i18n = signex_i18n::I18n::en_us();
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            let shown =
                crate::layout_probe::lay_out_texts(super::info_banner(&i18n, &tokens), WINDOW);
            assert!(!shown.iter().any(|t| t.text == "\u{24D8}"), "{shown:?}");
            let window = iced::Size::new(560.0, 200.0);
            let size = crate::layout_probe::root_size(super::info_banner(&i18n, &tokens), window);
            let pixels =
                crate::layout_probe::draw_pixels(super::info_banner(&i18n, &tokens), window);
            let at = (size.height / 2.0) as usize * window.width as usize * 4;
            let fill = crate::styles::pill_colors(&tokens, crate::styles::PillTone::Info).0;
            let want = composite_over(fill, iced::Color::BLACK);
            let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            let want = [channel(want.r), channel(want.g), channel(want.b)];
            let drawn = &pixels[at..at + 3];
            assert!(
                drawn.iter().zip(want).all(|(d, w)| d.abs_diff(w) <= 3),
                "{id:?}: left edge {drawn:?}, fill {want:?}"
            );
        }
    }

    /// Mockup row 83: each walk diagram sits in a tile, the field fill in a
    /// hairline border.
    #[test]
    fn the_walk_diagram_sits_in_a_bordered_tile() {
        use signex_types::theme::{ThemeId, theme_tokens};
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            let style = super::diagram_tile(&tokens)(&iced::Theme::Light);
            let field = crate::styles::tokens::resolve(&tokens).field;
            assert_eq!(style.background, Some(iced::Background::Color(field)));
            assert_eq!(style.border.width, 1.0);
            assert_eq!(style.border.color, crate::styles::ti(tokens.border));
        }
    }
}
