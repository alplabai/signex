//! Graphite look of the Properties form controls.
//!
//! The form-row builders only receive resolved colours, so the controls
//! find the look in their style closures: `&Theme` names the Graphite
//! theme ([`FormLook::of`]).

use super::graphite::PROPERTY_FIELD_RADIUS;
use super::*;
use crate::styles::tokens::resolve;
use crate::styles::{TypeScale, ti};
use iced::Widget;
use iced::widget::scrollable;
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::{button, checkbox, text_input};

const TS: TypeScale = TypeScale::DEFAULT;

/// Corner radius of a segmented-control cell.
const SEGMENT_RADIUS: f32 = 6.0;
/// Corner radius of the frame around a segmented control.
const SEGMENT_FRAME_RADIUS: f32 = 8.0;
/// Alpha of the ink wash under a hovered, inactive segment: a neutral grey,
/// not the tinted accent pill.
const SEGMENT_HOVER_ALPHA: f32 = 0.08;
/// Corner radius of a checkbox box.
const CHECKBOX_RADIUS: f32 = 4.0;

/// The colours a Graphite form control draws with.
#[derive(Debug, Clone, Copy)]
pub(in crate::panels) struct FormLook {
    pub field: Color,
    pub raised: Color,
    pub sunken: Color,
    pub line: Color,
    pub line_strong: Color,
    pub ink: Color,
    pub ink3: Color,
    pub ink4: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub primary: Color,
    pub primary_ink: Color,
}

impl FormLook {
    /// The look of `theme`: Light for the Graphite Light theme, Dark for
    /// every other.
    pub fn of(theme: &Theme) -> Self {
        let tokens = theme_tokens(Self::theme_id(theme));
        let r = resolve(&tokens);
        Self {
            field: r.field,
            raised: r.raised,
            sunken: r.sunken,
            line: ti(tokens.border),
            line_strong: r.line_strong,
            ink: ti(tokens.text),
            ink3: r.ink3,
            ink4: r.ink4,
            accent: ti(tokens.accent),
            accent_soft: r.accent_soft,
            primary: r.primary,
            primary_ink: r.primary_ink,
        }
    }

    /// The Graphite theme `theme` stands for: Light for the Graphite Light
    /// theme, Dark for every other.
    fn theme_id(theme: &Theme) -> ThemeId {
        if theme.to_string().starts_with("Graphite Light") {
            ThemeId::GraphiteLight
        } else {
            ThemeId::GraphiteDark
        }
    }
}

/// Fill, border colour and radius of a read-only value box: Graphite's
/// field token and hairline.
pub(in crate::panels) fn value_box(theme: &Theme) -> (Color, Color, f32) {
    let l = FormLook::of(theme);
    (l.field, l.line, PROPERTY_FIELD_RADIUS)
}

/// A value the panel only reports: the sunken fill with no border, so it
/// cannot be taken for a field (mockup rows 42, 43, 45 and 50).
pub(in crate::panels) fn read_only_value(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(FormLook::of(theme).sunken)),
        border: Border {
            radius: PROPERTY_FIELD_RADIUS.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// The tooltip that shows a field's whole value: a raised card with the
/// field's ink and hairline.
pub(in crate::panels) fn value_tip(theme: &Theme) -> container::Style {
    let l = FormLook::of(theme);
    container::Style {
        background: Some(Background::Color(l.raised)),
        text_color: Some(l.ink),
        border: Border {
            width: 1.0,
            radius: PROPERTY_FIELD_RADIUS.into(),
            color: l.line_strong,
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// `text_input` style for a form field.
pub(crate) fn text_input_style(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let l = FormLook::of(theme);
    let border = match status {
        text_input::Status::Focused { .. } => l.accent,
        text_input::Status::Hovered => l.line_strong,
        _ => l.line,
    };
    text_input::Style {
        background: Background::Color(l.field),
        border: Border {
            width: 1.0,
            radius: PROPERTY_FIELD_RADIUS.into(),
            color: border,
            ..Border::default()
        },
        placeholder: l.ink4,
        value: l.ink,
        selection: Color { a: 0.4, ..l.accent },
    }
}

/// [`text_input_style`] for a value that reads inline in a list: no fill or
/// outline until the box is hovered or focused.
pub(crate) fn inline_value_input_style(
    theme: &Theme,
    status: text_input::Status,
) -> text_input::Style {
    let mut style = text_input_style(theme, status);
    if !matches!(
        status,
        text_input::Status::Hovered | text_input::Status::Focused { .. }
    ) {
        style.background = Background::Color(Color::TRANSPARENT);
        style.border.color = Color::TRANSPARENT;
    }
    style
}

/// [`text_input_style`] with the outline in `error` while the box holds text
/// it refuses (#2767).
pub(crate) fn refusable_input_style(
    error: Option<Color>,
) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |theme, status| {
        let mut style = text_input_style(theme, status);
        if let Some(error) = error {
            style.border.color = error;
        }
        style
    }
}

/// [`text_input_style`] that greys while the field takes no input: the sunken
/// fill with faint ink (mockup row 111).
pub(crate) fn greyable_input_style(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = text_input_style(theme, status);
    if matches!(status, text_input::Status::Disabled) {
        let l = FormLook::of(theme);
        style.background = Background::Color(l.sunken);
        style.value = l.ink4;
    }
    style
}

/// `checkbox` style for a form toggle.
pub(crate) fn checkbox_style(theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    let base = checkbox::primary(theme, status);
    let l = FormLook::of(theme);
    let checked = match status {
        checkbox::Status::Active { is_checked }
        | checkbox::Status::Hovered { is_checked }
        | checkbox::Status::Disabled { is_checked } => is_checked,
    };
    checkbox::Style {
        background: Background::Color(if checked { l.primary } else { l.field }),
        icon_color: l.primary_ink,
        border: Border {
            width: 1.0,
            radius: CHECKBOX_RADIUS.into(),
            color: if checked { l.primary } else { l.line_strong },
            ..Border::default()
        },
        text_color: base.text_color,
    }
}

/// `checkbox` style of a group box whose rows are partly ticked: drawn
/// checked, on the soft accent with an accent hairline, so it reads as
/// neither all nor none.
pub(crate) fn mixed_checkbox_style(theme: &Theme, status: checkbox::Status) -> checkbox::Style {
    let l = FormLook::of(theme);
    checkbox::Style {
        background: Background::Color(l.accent_soft),
        icon_color: l.accent,
        border: Border {
            width: 1.0,
            radius: CHECKBOX_RADIUS.into(),
            color: l.accent,
            ..Border::default()
        },
        ..checkbox_style(theme, status)
    }
}

/// `scrollable` style of a dock panel: the thin Graphite scrollbar
/// ([`crate::styles::scrollbar_style`]) in the colours of `theme`.
pub(in crate::panels) fn scrollbar_style(
    theme: &Theme,
    status: scrollable::Status,
) -> scrollable::Style {
    let tokens = theme_tokens(FormLook::theme_id(theme));
    crate::styles::scrollbar_style(&tokens)(theme, status)
}

/// Frame of a segmented control: sunken fill, radius 8, 2 px inset.
pub(in crate::panels) fn segmented_frame(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(FormLook::of(theme).sunken)),
        border: Border {
            radius: SEGMENT_FRAME_RADIUS.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// One cell of a Graphite segmented control: raised and bold-ink when on,
/// flat when off.
pub(in crate::panels) fn segment_cell(theme: &Theme, active: bool, hovered: bool) -> button::Style {
    let l = FormLook::of(theme);
    let fill = if active {
        Some(Background::Color(l.raised))
    } else if hovered {
        Some(Background::Color(Color {
            a: SEGMENT_HOVER_ALPHA,
            ..l.ink
        }))
    } else {
        None
    };
    button::Style {
        background: fill,
        text_color: if active { l.ink } else { l.ink3 },
        border: Border {
            radius: SEGMENT_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// A justification cell and its dot, as ratios of the body text size
/// (22 px, 6 px and 8 px at a 12 px body, mockup row 44).
pub(in crate::panels) const ANCHOR_CELL_EM: f32 = 22.0 / 12.0;
const ANCHOR_DOT_EM: f32 = 6.0 / 12.0;
const ANCHOR_DOT_ON_EM: f32 = 8.0 / 12.0;

/// The 3 x 3 justification grid (mockup row 44): a sunken frame of dot
/// cells, `cells` row by row as (is the anchor, what a press sends). The
/// anchor's cell is raised with a shadow and an accent dot.
pub(in crate::panels) fn anchor_grid(
    cells: [[(bool, PanelMsg); 3]; 3],
) -> Element<'static, PanelMsg> {
    let gap = crate::styles::Spacing::DEFAULT.xs / 2.0;
    let mut grid = Column::<Element<'static, PanelMsg>>::new().spacing(gap);
    for line in cells {
        let mut cells_row = iced::widget::Row::<Element<'static, PanelMsg>>::new().spacing(gap);
        for (on, press) in line {
            cells_row = cells_row.push(anchor_cell(on, press));
        }
        grid = grid.push(cells_row.boxed());
    }
    container(grid)
        .padding(gap)
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(FormLook::of(theme).sunken)),
            border: Border {
                radius: crate::styles::Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// One cell of [`anchor_grid`]: a line dot, or the raised, shadowed cell
/// with the larger accent dot when `on`.
fn anchor_cell(on: bool, press: PanelMsg) -> Element<'static, PanelMsg> {
    let dot = TS.body * if on { ANCHOR_DOT_ON_EM } else { ANCHOR_DOT_EM };
    let side = TS.body * ANCHOR_CELL_EM;
    let mark = container(Space::new())
        .width(dot)
        .height(dot)
        .style(move |theme: &Theme| {
            let l = FormLook::of(theme);
            container::Style {
                background: Some(Background::Color(if on { l.accent } else { l.line_strong })),
                border: Border {
                    radius: (dot / 2.0).into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }
        });
    button(container(mark).center(Length::Fill))
        .width(side)
        .height(side)
        .padding(0)
        .on_press(press)
        .style(move |theme: &Theme, _| {
            let shadow = if on {
                iced::Shadow {
                    color: resolve(&theme_tokens(FormLook::theme_id(theme))).shadow,
                    offset: iced::Vector::new(0.0, 1.0),
                    blur_radius: 2.0,
                }
            } else {
                iced::Shadow::default()
            };
            button::Style {
                background: on.then(|| Background::Color(FormLook::of(theme).raised)),
                border: Border {
                    radius: crate::styles::Radii::GRAPHITE.small.into(),
                    ..Border::default()
                },
                shadow,
                ..button::Style::default()
            }
        })
        .boxed()
}

/// Ghost button: transparent, hairline, radius 7.
pub(crate) fn ghost_button(theme: &Theme, status: button::Status) -> button::Style {
    let l = FormLook::of(theme);
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: hovered.then_some(Background::Color(l.accent_soft)),
        text_color: l.ink,
        border: Border {
            width: 1.0,
            radius: PROPERTY_FIELD_RADIUS.into(),
            color: l.line,
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// Option tile of a Properties row (fill, placement, a class list): a ghost
/// button, the chosen one in accent-soft with an accent hairline.
pub(crate) fn option_tile(
    active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style + 'static {
    move |theme, status| {
        let mut style = ghost_button(theme, status);
        if active {
            let l = FormLook::of(theme);
            style.background = Some(Background::Color(l.accent_soft));
            style.border.color = l.accent;
        }
        style
    }
}

/// A value shown as a field that opens something when pressed (the canvas
/// font): field fill, hairline, ink text; the hairline strengthens on hover.
pub(crate) fn value_field_button(theme: &Theme, status: button::Status) -> button::Style {
    let l = FormLook::of(theme);
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(Background::Color(l.field)),
        text_color: l.ink,
        border: Border {
            width: 1.0,
            radius: PROPERTY_FIELD_RADIUS.into(),
            color: if hovered { l.line_strong } else { l.line },
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// Active tab chip of the Properties tab strip: accent-soft fill, radius 7.
pub(in crate::panels) fn tab_chip(theme: &Theme, active: bool, hovered: bool) -> button::Style {
    let l = FormLook::of(theme);
    button::Style {
        background: (active || hovered).then_some(Background::Color(l.accent_soft)),
        text_color: if active { l.ink } else { l.ink3 },
        border: Border {
            radius: PROPERTY_FIELD_RADIUS.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

/// Section label of a Graphite form: uppercase, muted, 12 px side padding.
pub(in crate::panels) fn section_label<'a, 'b, M: 'a>(
    title: impl Into<signex_widgets::Label<'b>>,
    tokens: &ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'a, M> {
    let ink = resolve(tokens).ink4;
    let title = title.into();
    container(
        // raw: the title as given, upper-cased for the locale.
        text(raw(i18n.upper_for_display(title.as_str())))
            .size(TS.section)
            .color(ink),
    )
    .padding(iced::Padding {
        top: 12.0,
        right: 12.0,
        bottom: 6.0,
        left: 12.0,
    })
    .width(Length::Fill)
    .boxed()
}

/// Collapsible-section header of a Graphite form: chevron plus the
/// uppercase label, no rule.
fn graphite_section_header<'a>(
    key: &str,
    title: &signex_i18n::Translatable,
    is_collapsed: bool,
    tokens: &ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Column<Element<'a, PanelMsg>> {
    let ink = resolve(tokens).ink4;
    let label = row![
        signex_widgets::chevron::chevron(!is_collapsed, TS.section, ink),
        // raw: the catalog title, upper-cased for the locale.
        text(raw(i18n.upper_for_display(&title.render(i18n))))
            .size(TS.section)
            .color(ink),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);
    let header = button(
        container(label)
            .padding(iced::Padding {
                top: 12.0,
                right: 12.0,
                bottom: 6.0,
                left: 12.0,
            })
            .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(PanelMsg::ToggleSection(key.to_string()))
    .style(|_: &Theme, _| button::Style::default());
    Column::new()
        .spacing(0)
        .width(Length::Fill)
        .push(header.boxed())
}

impl PanelContext {
    /// A collapsible form section on the Graphite uppercase-label chrome;
    /// `title` is a catalog message, rendered here.
    pub(in crate::panels) fn collapsible<'a>(
        &self,
        key: &str,
        title: impl Into<signex_i18n::Translatable>,
        collapsed: &CollapsedSections,
        content: impl FnOnce() -> Column<Element<'a, PanelMsg>>,
    ) -> Column<Element<'a, PanelMsg>> {
        let is_collapsed = collapsed.contains(key);
        let title = title.into();
        let mut col = graphite_section_header(key, &title, is_collapsed, &self.tokens, &self.i18n);
        if !is_collapsed {
            col = col.push(content().boxed());
        }
        col
    }
}
