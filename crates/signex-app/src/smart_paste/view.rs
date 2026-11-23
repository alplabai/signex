//! The Smart Paste dialog (mockup row 155, "Smart Paste · offset" and
//! "Smart Paste · array"): the Place switch and its offset or array
//! fields, the Designators and Net names choices with the After paste
//! table on the left; the live drawing of the source and the ghost copies
//! on the right, whose first copy can be dragged; the footer. A pure view.

use iced::widget::{Column, Space, canvas, column, container, row, scrollable};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme,
    Widget as _, mouse,
};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{SegmentedStyle, button, radio, segmented, text, text_input};

use super::plan::{DesignatorChoice, NetAfter, NetChoice, PlaceMode, snapped};
use super::state::{PreviewShape, SmartPasteMsg, SmartPasteState};
use crate::styles::{
    DialogButtonKind, Radii, Spacing, TypeScale, chrome_px, dialog_button, dialog_footer_split, ti,
    tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Width of the left pane (mockup: 400 px).
const LEFT_W: f32 = chrome_px(400.0 * 11.0 / 12.0);
/// Label column of the form grid (mockup: 110 px).
const LABEL_W: f32 = chrome_px(110.0 * 11.0 / 12.0);
/// A number field (mockup: 90 px).
const FIELD_W: f32 = chrome_px(90.0 * 11.0 / 12.0);
/// Header height (mockup: 44 px).
const HEAD_H: f32 = chrome_px(44.0 * 11.0 / 12.0);

fn segmented_style(theme_tokens: &ThemeTokens) -> SegmentedStyle {
    let r = tokens::resolve(theme_tokens);
    r.segmented(TS.body)
        .resting_ink(ti(theme_tokens.text_secondary))
}

fn group<'a>(label: Localized, theme_tokens: &ThemeTokens) -> Element<'a, SmartPasteMsg> {
    text(label)
        .size(TS.caption)
        .color(tokens::resolve(theme_tokens).ink3)
        .boxed()
}

fn caption<'a>(label: Localized, theme_tokens: &ThemeTokens) -> Element<'a, SmartPasteMsg> {
    text(label)
        .size(TS.caption)
        .color(tokens::muted_text(theme_tokens))
        .boxed()
}

/// One form-grid row: the label column, then the controls.
fn form_row<'a>(
    label: Localized,
    control: Element<'a, SmartPasteMsg>,
    enabled: bool,
    theme_tokens: &ThemeTokens,
) -> Element<'a, SmartPasteMsg> {
    let ink = if enabled {
        tokens::muted_text(theme_tokens)
    } else {
        tokens::resolve(theme_tokens).ink4
    };
    row![
        text(label)
            .size(TS.label)
            .color(ink)
            .width(Length::Fixed(LABEL_W)),
        control,
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

type OnInput = fn(String) -> SmartPasteMsg;

/// A field's message, as the fn pointer [`mm_field`] takes.
fn edits(on_input: OnInput) -> Option<OnInput> {
    Some(on_input)
}

/// A number field with its unit; read-only (no message) when disabled.
fn mm_field<'a>(
    value: &'a str,
    unit: Option<Localized>,
    on_input: Option<OnInput>,
) -> Element<'a, SmartPasteMsg> {
    let mut input = text_input(UserText::new(String::new()), value)
        .size(TS.body)
        .width(Length::Fixed(FIELD_W));
    if let Some(on_input) = on_input {
        input = input.on_input(on_input);
    }
    let mut line = row![input].spacing(SP.xs).align_y(Alignment::Center);
    if let Some(unit) = unit {
        line = line.push(text(unit).size(TS.caption).boxed());
    }
    line.boxed()
}

fn header<'a>(
    state: &'a SmartPasteState,
    i18n: &I18n,
    theme_tokens: &ThemeTokens,
) -> Element<'a, SmartPasteMsg> {
    let s = &state.summary;
    let subtitle = t!(i18n, ids::SMART_PASTE_SUBTITLE,
        "parts" => s.parts.join(", "),
        "wires" => s.wires as i64,
        "labels" => s.labels as i64);
    let close = button(
        text(t!(i18n, ids::SMART_PASTE_CLOSE))
            .size(TS.heading)
            .color(tokens::muted_text(theme_tokens)),
    )
    .padding([0.0, SP.s])
    .on_press(SmartPasteMsg::Cancelled)
    .style(crate::styles::ghost_button(theme_tokens));
    container(
        row![
            text(t!(i18n, ids::SMART_PASTE_TITLE))
                .size(TS.heading)
                .color(ti(theme_tokens.text)),
            text(subtitle)
                .size(TS.label)
                .color(tokens::resolve(theme_tokens).ink3),
            Space::new().width(Length::Fill),
            close,
        ]
        .spacing(SP.l)
        .align_y(Alignment::Center),
    )
    .padding([0.0, SP.l])
    .height(Length::Fixed(HEAD_H))
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill)
    .boxed()
}

fn place_fields<'a>(
    state: &'a SmartPasteState,
    i18n: &I18n,
    theme_tokens: &ThemeTokens,
) -> Element<'a, SmartPasteMsg> {
    let mm = || Some(t!(i18n, ids::SMART_PASTE_MM));
    let pair = |x: &'a str, y: &'a str, on_x: OnInput, on_y: OnInput| {
        row![
            mm_field(x, mm(), edits(on_x)),
            mm_field(y, mm(), edits(on_y))
        ]
        .spacing(SP.s)
        .boxed()
    };
    match state.mode {
        PlaceMode::Offset => form_row(
            t!(i18n, ids::SMART_PASTE_OFFSET),
            pair(
                &state.offset_x,
                &state.offset_y,
                SmartPasteMsg::OffsetXEdited,
                SmartPasteMsg::OffsetYEdited,
            ),
            true,
            theme_tokens,
        ),
        PlaceMode::Array => column![
            form_row(
                t!(i18n, ids::SMART_PASTE_COPIES),
                mm_field(&state.copies, None, edits(SmartPasteMsg::CopiesEdited)),
                true,
                theme_tokens,
            ),
            form_row(
                t!(i18n, ids::SMART_PASTE_STEP),
                pair(
                    &state.step_x,
                    &state.step_y,
                    SmartPasteMsg::StepXEdited,
                    SmartPasteMsg::StepYEdited,
                ),
                true,
                theme_tokens,
            ),
        ]
        .spacing(SP.s)
        .boxed(),
    }
}

/// The After paste table: Net | After paste.
fn after_table<'a>(
    state: &'a SmartPasteState,
    i18n: &I18n,
    theme_tokens: &ThemeTokens,
) -> Element<'a, SmartPasteMsg> {
    let r = tokens::resolve(theme_tokens);
    let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
    let power_ink = ti(theme_tokens.error);
    let taken = state.taken_names();
    let cell = |content: Element<'a, SmartPasteMsg>, w: Length| {
        container(content)
            .padding([0.0, SP.s])
            .width(w)
            .height(Length::Fixed(chrome_px(26.0 * 11.0 / 12.0)))
            .align_y(iced::alignment::Vertical::Center)
    };
    let mut table: Column<Element<'a, SmartPasteMsg>> = Column::new().width(Length::Fill);
    table = table.push(
        row![
            cell(
                caption(t!(i18n, ids::SMART_PASTE_NET), theme_tokens),
                Length::FillPortion(2)
            ),
            cell(
                caption(t!(i18n, ids::SMART_PASTE_AFTER), theme_tokens),
                Length::FillPortion(3)
            ),
        ]
        .boxed(),
    );
    for (net, after) in state.after_rows() {
        let power = matches!(after, NetAfter::Power);
        let name = text(UserText::new(net.clone()))
            .size(TS.body)
            .font(mono)
            .color(if power {
                power_ink
            } else {
                ti(theme_tokens.text)
            })
            .boxed();
        let after: Element<'a, SmartPasteMsg> = match after {
            NetAfter::Power => text(t!(i18n, ids::SMART_PASTE_KEPT_POWER))
                .size(TS.body)
                .color(r.ink3)
                .boxed(),
            NetAfter::Joins => text(t!(i18n, ids::SMART_PASTE_JOINS, "net" => net))
                .size(TS.body)
                .font(mono)
                .color(r.ink3)
                .boxed(),
            NetAfter::Renamed(names) => {
                let clashes = names.iter().any(|n| taken.iter().any(|t| t == n.trim()));
                text(UserText::new(names.join(", ")))
                    .size(TS.body)
                    .font(mono)
                    .color(if clashes {
                        ti(theme_tokens.error)
                    } else {
                        ti(theme_tokens.text)
                    })
                    .wrapping(iced::widget::text::Wrapping::None)
                    .boxed()
            }
        };
        table = table.push(
            row![
                cell(name, Length::FillPortion(2)),
                cell(after, Length::FillPortion(3)),
            ]
            .boxed(),
        );
    }
    let line = r.line_soft;
    container(table)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            border: Border {
                color: line,
                width: 1.0,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

fn left<'a>(
    state: &'a SmartPasteState,
    i18n: &I18n,
    theme_tokens: &ThemeTokens,
) -> Element<'a, SmartPasteMsg> {
    let mode = segmented(
        [
            (PlaceMode::Offset, t!(i18n, ids::SMART_PASTE_ONCE).into()),
            (PlaceMode::Array, t!(i18n, ids::SMART_PASTE_ARRAY).into()),
        ],
        state.mode,
        segmented_style(theme_tokens),
        SmartPasteMsg::ModePicked,
    );
    let next_free = if state.next_free_text.is_empty() {
        t!(i18n, ids::SMART_PASTE_NEXT_FREE_PLAIN)
    } else {
        t!(i18n, ids::SMART_PASTE_NEXT_FREE, "mapping" => state.next_free_text.as_str())
    };
    let designators = column![
        radio(
            next_free,
            DesignatorChoice::NextFree,
            Some(state.designators),
            SmartPasteMsg::DesignatorsPicked
        )
        .text_size(TS.body)
        .size(TS.heading),
        radio(
            t!(i18n, ids::SMART_PASTE_KEEP_DESIGNATORS),
            DesignatorChoice::KeepIfFree,
            Some(state.designators),
            SmartPasteMsg::DesignatorsPicked
        )
        .text_size(TS.body)
        .size(TS.heading),
    ]
    .spacing(SP.xs);
    let nets = column![
        radio(
            t!(i18n, ids::SMART_PASTE_KEEP_NETS),
            NetChoice::Keep,
            Some(state.nets),
            SmartPasteMsg::NetsPicked
        )
        .text_size(TS.body)
        .size(TS.heading),
        radio(
            t!(i18n, ids::SMART_PASTE_REWRITE_NETS),
            NetChoice::Rewrite,
            Some(state.nets),
            SmartPasteMsg::NetsPicked
        )
        .text_size(TS.body)
        .size(TS.heading),
    ]
    .spacing(SP.xs);
    // Pattern and Start grey out while Keep is chosen.
    let rewriting = state.nets == NetChoice::Rewrite;
    let pattern = form_row(
        t!(i18n, ids::SMART_PASTE_PATTERN),
        text_input(UserText::new(String::new()), &state.pattern)
            .on_input_maybe(edits(SmartPasteMsg::PatternEdited).filter(|_| rewriting))
            .size(TS.body)
            .width(Length::Fill)
            .boxed(),
        rewriting,
        theme_tokens,
    );
    let start = form_row(
        t!(i18n, ids::SMART_PASTE_START),
        mm_field(
            &state.start,
            None,
            edits(SmartPasteMsg::StartEdited).filter(|_| rewriting),
        ),
        rewriting,
        theme_tokens,
    );
    let mut body = column![
        group(t!(i18n, ids::SMART_PASTE_PLACE), theme_tokens),
        mode,
        place_fields(state, i18n, theme_tokens),
        group(t!(i18n, ids::SMART_PASTE_DESIGNATORS), theme_tokens),
        designators,
        group(t!(i18n, ids::SMART_PASTE_NET_NAMES), theme_tokens),
        nets,
        pattern,
        start,
        after_table(state, i18n, theme_tokens),
    ]
    .spacing(SP.m)
    .padding([SP.l, SP.xl]);
    let taken = state.taken_names();
    if !taken.is_empty() {
        body = body.push(
            text(t!(i18n, ids::SMART_PASTE_NAMES_TAKEN, "names" => taken.join(", ")))
                .size(TS.caption)
                .color(ti(theme_tokens.error)),
        );
    }
    container(scrollable(body).height(Length::Fill))
        .width(Length::Fixed(LEFT_W))
        .height(Length::Fill)
        .boxed()
}

/// The inks the drawing uses.
#[derive(Debug, Clone, Copy)]
struct PreviewInk {
    wire: Color,
    body: Color,
    text: Color,
    power: Color,
    accent: Color,
}

impl PreviewInk {
    fn of(theme_tokens: &ThemeTokens) -> Self {
        Self {
            wire: ti(theme_tokens.success),
            body: ti(theme_tokens.text_secondary),
            text: ti(theme_tokens.text),
            power: ti(theme_tokens.error),
            accent: ti(theme_tokens.accent),
        }
    }
}

/// The live drawing: the source, then each ghost copy with an accent
/// outline; the first copy can be dragged (1.27 mm grid).
struct Preview<'a> {
    state: &'a SmartPasteState,
    ink: PreviewInk,
}

/// A drag of the first copy: where the press was and the offset then.
#[derive(Debug, Default)]
struct DragState {
    from: Option<(Point, (f64, f64))>,
}

impl Preview<'_> {
    /// Scale (px per mm) and origin so the source and every copy fit.
    fn fit(&self, size: Size) -> (f32, Point) {
        let ((x0, y0), (x1, y1)) = self.state.bounds;
        let (dx, dy) = self.state.first_offset();
        let n = f64::from(self.state.copy_count());
        let min_x = x0.min(x0 + dx * n);
        let max_x = x1.max(x1 + dx * n);
        let min_y = y0.min(y0 + dy * n);
        let max_y = y1.max(y1 + dy * n);
        let w = (max_x - min_x).max(1.0) as f32;
        let h = (max_y - min_y).max(1.0) as f32;
        let pad = 2.0 * SP.xl;
        let scale = ((size.width - pad) / w)
            .min((size.height - pad) / h)
            .clamp(1.2, 8.0);
        let origin = Point::new(SP.xl - min_x as f32 * scale, SP.xl - min_y as f32 * scale);
        (scale, origin)
    }

    /// The first copy's box on screen.
    fn first_copy_box(&self, size: Size) -> Rectangle {
        let (scale, o) = self.fit(size);
        let ((x0, y0), (x1, y1)) = self.state.bounds;
        let (dx, dy) = self.state.first_offset();
        Rectangle::new(
            Point::new(
                o.x + (x0 + dx) as f32 * scale,
                o.y + (y0 + dy) as f32 * scale,
            ),
            Size::new((x1 - x0) as f32 * scale, (y1 - y0) as f32 * scale),
        )
    }
}

impl canvas::Program<SmartPasteMsg> for Preview<'_> {
    type State = DragState;

    fn update(
        &self,
        drag: &mut DragState,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<SmartPasteMsg>> {
        let iced::Event::Mouse(event) = event else {
            return None;
        };
        match event {
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                let at = cursor.position_in(bounds)?;
                if !self.first_copy_box(bounds.size()).contains(at) {
                    return None;
                }
                drag.from = Some((at, self.state.first_offset()));
                Some(canvas::Action::capture())
            }
            mouse::Event::CursorMoved { .. } => {
                let (from, (ox, oy)) = drag.from?;
                let at = cursor.position_in(bounds)?;
                let (scale, _) = self.fit(bounds.size());
                let dx = snapped(ox + f64::from((at.x - from.x) / scale));
                let dy = snapped(oy + f64::from((at.y - from.y) / scale));
                Some(
                    canvas::Action::publish(SmartPasteMsg::FirstCopyDragged { dx, dy })
                        .and_capture(),
                )
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) => {
                drag.from.take()?;
                Some(canvas::Action::capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _drag: &DragState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let (scale, o) = self.fit(bounds.size());
        let at = |(x, y): (f64, f64), (dx, dy): (f64, f64)| {
            Point::new(o.x + (x + dx) as f32 * scale, o.y + (y + dy) as f32 * scale)
        };
        let (dx, dy) = self.state.first_offset();
        let rows = self.state.after_rows();
        let copies = self.state.copy_count();
        for k in 0..=copies {
            let shift = (dx * f64::from(k), dy * f64::from(k));
            let ghost = k > 0;
            let alpha = |c: Color| if ghost { Color { a: 0.6, ..c } } else { c };
            for shape in &self.state.shapes {
                match shape {
                    PreviewShape::Line { from, to } => frame.stroke(
                        &canvas::Path::line(at(*from, shift), at(*to, shift)),
                        canvas::Stroke::default()
                            .with_color(alpha(self.ink.wire))
                            .with_width(1.2),
                    ),
                    PreviewShape::Part {
                        min,
                        max,
                        reference,
                    } => {
                        let a = at(*min, shift);
                        let b = at(*max, shift);
                        frame.stroke(
                            &canvas::Path::rectangle(a, Size::new(b.x - a.x, b.y - a.y)),
                            canvas::Stroke::default()
                                .with_color(alpha(self.ink.body))
                                .with_width(1.0),
                        );
                        frame.fill_text(canvas::Text {
                            content: reference.clone(),
                            position: Point::new(b.x + 2.0, a.y),
                            color: alpha(self.ink.text),
                            size: iced::Pixels(TS.caption),
                            ..canvas::Text::default()
                        });
                    }
                    PreviewShape::Label { at: p, net, power } => {
                        let name = if ghost && !power {
                            renamed(&rows, net, k).unwrap_or_else(|| net.clone())
                        } else {
                            net.clone()
                        };
                        let ink = if *power {
                            self.ink.power
                        } else {
                            self.ink.text
                        };
                        frame.fill_text(canvas::Text {
                            content: name,
                            position: at(*p, shift),
                            color: alpha(ink),
                            size: iced::Pixels(TS.caption),
                            ..canvas::Text::default()
                        });
                    }
                }
            }
            if ghost {
                let ((x0, y0), (x1, y1)) = self.state.bounds;
                let a = at((x0, y0), shift);
                let b = at((x1, y1), shift);
                let outline = canvas::Path::rounded_rectangle(
                    Point::new(a.x - 4.0, a.y - 4.0),
                    Size::new(b.x - a.x + 8.0, b.y - a.y + 8.0),
                    6.0_f32.into(),
                );
                frame.stroke(
                    &outline,
                    canvas::Stroke {
                        line_dash: canvas::LineDash {
                            segments: &[4.0, 3.0],
                            offset: 0,
                        },
                        ..canvas::Stroke::default()
                            .with_color(self.ink.accent)
                            .with_width(1.0)
                    },
                );
            }
        }
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        drag: &DragState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        let over = cursor
            .position_in(bounds)
            .is_some_and(|at| self.first_copy_box(bounds.size()).contains(at));
        if drag.from.is_some() {
            mouse::Interaction::Grabbing
        } else if over {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}

/// What copy `k` (1-based) calls `net` after the paste.
fn renamed(rows: &[(String, NetAfter)], net: &str, k: u32) -> Option<String> {
    rows.iter()
        .find(|(n, _)| n == net)
        .and_then(|(_, after)| match after {
            NetAfter::Renamed(names) => names.get(k as usize - 1).cloned(),
            NetAfter::Joins | NetAfter::Power => None,
        })
}

fn right<'a>(
    state: &'a SmartPasteState,
    i18n: &I18n,
    theme_tokens: &ThemeTokens,
) -> Element<'a, SmartPasteMsg> {
    let copies = state.copy_count();
    let head = row![
        text(t!(i18n, ids::SMART_PASTE_PREVIEW))
            .size(TS.label)
            .color(ti(theme_tokens.text))
            .width(Length::Fill),
        caption(
            t!(i18n, ids::SMART_PASTE_COPY_COUNT, "count" => i64::from(copies)),
            theme_tokens
        ),
    ]
    .align_y(Alignment::Center);
    let drawing = canvas(Preview {
        state,
        ink: PreviewInk::of(theme_tokens),
    })
    .width(Length::Fill)
    .height(Length::Fill);
    column![
        container(head).padding([SP.s, SP.m]),
        rule(theme_tokens),
        container(drawing)
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .height(Length::Fill),
        container(caption(t!(i18n, ids::SMART_PASTE_DRAG_HINT), theme_tokens))
            .padding([SP.xs, SP.m]),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

fn rule<'a>(theme_tokens: &ThemeTokens) -> Element<'a, SmartPasteMsg> {
    let colour = tokens::resolve(theme_tokens).line_soft;
    container(Space::new())
        .width(Length::Fill)
        .height(1.0)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(colour)),
            ..container::Style::default()
        })
        .boxed()
}

fn vrule<'a>(theme_tokens: &ThemeTokens) -> Element<'a, SmartPasteMsg> {
    let colour = tokens::resolve(theme_tokens).line_soft;
    container(Space::new())
        .width(1.0)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(colour)),
            ..container::Style::default()
        })
        .boxed()
}

/// The paste button's label: "Paste", or "Paste N copies".
fn paste_label(copies: u32) -> MessageId {
    if copies > 1 {
        ids::SMART_PASTE_PASTE_COPIES
    } else {
        ids::SMART_PASTE_PASTE
    }
}

/// The dialog card, `size` (width, height) in px.
pub fn view<'a>(
    state: &'a SmartPasteState,
    size: (f32, f32),
    theme_tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, SmartPasteMsg> {
    let copies = state.copy_count();
    let footer = dialog_footer_split(
        vec![caption(t!(i18n, ids::SMART_PASTE_FOOTER), theme_tokens)],
        vec![
            dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                DialogButtonKind::Cancel,
                Some(SmartPasteMsg::Cancelled),
                theme_tokens,
            ),
            dialog_button(
                t!(i18n, paste_label(copies), "count" => i64::from(copies)),
                DialogButtonKind::Primary,
                state.can_paste().then_some(SmartPasteMsg::PasteConfirmed),
                theme_tokens,
            ),
        ],
    );
    let body = row![
        left(state, i18n, theme_tokens),
        vrule(theme_tokens),
        right(state, i18n, theme_tokens),
    ]
    .height(Length::Fill);
    container(
        column![
            header(state, i18n, theme_tokens),
            rule(theme_tokens),
            body,
            rule(theme_tokens),
            footer,
        ]
        .width(Length::Fixed(size.0))
        .height(Length::Fixed(size.1)),
    )
    .style(crate::styles::modal_card(theme_tokens))
    .clip(true)
    .boxed()
}
