//! Bill-of-materials export dialog catalog slice, laid out as the app's
//! dialog (mockup row 112): header with the project file, a toolbar with the
//! line filter, the one variant picker, the grouping switch and an issue
//! chip, the lines beside a 300 px Properties sidebar, and a footer with the
//! line and part counts, Cancel and a format-named Export button.

use iced::widget::{Column, Row, Space, column, container, row, svg};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_types::theme::ThemeTokens;
use signex_widgets::{raw, text};

use crate::catalog::Message;
use crate::icon::x_handle;
use crate::theme;

/// Width of the row-number gutter, as the app's dialog draws it.
const GUTTER_W: f32 = 34.0;

pub(crate) fn view<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    const MODAL_WIDTH: f32 = 1000.0;
    const MODAL_HEIGHT: f32 = 700.0;
    const HEADER_HEIGHT: f32 = 28.0;
    const SIDEBAR_WIDTH: f32 = 300.0;

    let panel_bg = theme::color(tokens.panel_bg);
    let toolbar_bg = theme::color(tokens.toolbar_bg);
    let text_color = theme::color(tokens.text);
    let muted = theme::color(tokens.text_secondary);
    let border = theme::color(tokens.border);
    let accent = theme::color(tokens.accent);

    let close_icon = svg(x_handle())
        .width(14)
        .height(14)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(text_color),
            ..Default::default()
        });
    let header = container(
        row![
            text(raw("Bill of Materials")).size(13).color(text_color),
            Space::new().width(8),
            text(raw("Loratis-SN.snxprj")).size(11).color(muted),
            Space::new().width(Length::Fill),
            container(close_icon)
                .width(46)
                .height(HEADER_HEIGHT)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center),
        ]
        .align_y(iced::Alignment::Center),
    )
    .height(HEADER_HEIGHT)
    .padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 16.0,
    })
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(toolbar_bg)),
        border: Border {
            width: 0.0,
            radius: iced::border::Radius::default().top_left(8.0).top_right(8.0),
            color: Color::TRANSPARENT,
            ..Border::default()
        },
        ..container::Style::default()
    });

    let toolbar = container(
        row![
            field_box("Filter lines", 180.0, tokens),
            Space::new().width(12),
            text(raw("Variant")).size(11).color(muted),
            Space::new().width(6),
            field_box("Production  ▾", 140.0, tokens),
            Space::new().width(12),
            segmented(&["Grouped", "Ungrouped", "Flat"], 0, tokens),
            Space::new().width(Length::Fill),
            chip("2 warnings", tokens),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([8, 12])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(toolbar_bg)),
        ..container::Style::default()
    });

    let content = row![
        container(table(tokens))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(panel_bg)),
                ..container::Style::default()
            }),
        container(Space::new())
            .width(1)
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(border)),
                ..container::Style::default()
            }),
        container(properties_sidebar(tokens))
            .width(Length::Fixed(SIDEBAR_WIDTH))
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(panel_bg)),
                ..container::Style::default()
            }),
    ]
    .height(Length::Fill);

    let footer = container(
        row![
            text(raw("10 of 10 lines · 73 parts · no cached prices"))
                .size(11)
                .color(muted),
            Space::new().width(Length::Fill),
            secondary_button("Cancel", tokens),
            Space::new().width(8),
            primary_button("Export CSV…", accent, Color::WHITE),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(toolbar_bg)),
        ..container::Style::default()
    });

    container(
        column![header, toolbar, content, footer]
            .spacing(0)
            .width(Length::Fixed(MODAL_WIDTH))
            .height(Length::Fixed(MODAL_HEIGHT)),
    )
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(panel_bg)),
        border: Border {
            width: 1.0,
            radius: 8.0.into(),
            color: border,
            ..Border::default()
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..container::Style::default()
    })
    .clip(true)
    .boxed()
}

/// A text field or a closed picker: a bordered box with its text.
fn field_box<'a>(label: &str, width: f32, tokens: &ThemeTokens) -> Element<'a, Message> {
    let muted = theme::color(tokens.text_secondary);
    let border = theme::color(tokens.border);
    container(text(raw(label.to_string())).size(11).color(muted))
        .width(width)
        .height(24)
        .padding([4, 8])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A segmented switch with `active` picked.
fn segmented<'a>(labels: &[&str], active: usize, tokens: &ThemeTokens) -> Element<'a, Message> {
    let text_color = theme::color(tokens.text);
    let accent = theme::color(tokens.accent);
    let border = theme::color(tokens.border);
    let mut cells = Row::new().spacing(0);
    for (index, label) in labels.iter().enumerate() {
        let fill = if index == active {
            Color { a: 0.18, ..accent }
        } else {
            Color::TRANSPARENT
        };
        cells = cells.push(
            container(text(raw(label.to_string())).size(11).color(text_color))
                .padding([4, 10])
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(fill)),
                    ..container::Style::default()
                }),
        );
    }
    container(cells)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The issue chip that opens the Rules tab.
fn chip<'a>(label: &str, tokens: &ThemeTokens) -> Element<'a, Message> {
    let warning = theme::color(tokens.warning);
    container(text(raw(label.to_string())).size(11).color(warning))
        .padding([2, 8])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(Color { a: 0.14, ..warning })),
            border: Border {
                width: 0.0,
                radius: 10.0.into(),
                color: Color::TRANSPARENT,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

fn secondary_button<'a>(label: &str, tokens: &ThemeTokens) -> Element<'a, Message> {
    let text_color = theme::color(tokens.text);
    let border = theme::color(tokens.border);
    container(text(raw(label.to_string())).size(11).color(text_color))
        .padding([5, 14])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

fn primary_button<'a>(label: &str, accent: Color, text_color: Color) -> Element<'a, Message> {
    container(text(raw(label.to_string())).size(11).color(text_color))
        .padding([5, 18])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(accent)),
            border: Border {
                width: 0.0,
                radius: 3.0.into(),
                color: Color::TRANSPARENT,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The lines: a row-number gutter, text columns, and Qty right-aligned. The
/// second line is flagged by a rule: a soft warning tint and a 2 px edge.
fn table<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let text_color = theme::color(tokens.text);
    let muted = theme::color(tokens.text_secondary);
    let toolbar_bg = theme::color(tokens.toolbar_bg);
    let warning = theme::color(tokens.warning);
    let header_cell = |label: &'static str, width: f32, right: bool| -> Element<'a, Message> {
        let align = if right {
            iced::alignment::Horizontal::Right
        } else {
            iced::alignment::Horizontal::Left
        };
        container(text(raw(label)).size(11).color(text_color))
            .width(Length::Fixed(width))
            .padding([4, 8])
            .align_x(align)
            .boxed()
    };
    let header = container(
        row![
            header_cell("#", GUTTER_W, true),
            header_cell("Designator ▲", 170.0, false),
            header_cell("Value", 90.0, false),
            header_cell("Description", 220.0, false),
            header_cell("Footprint", 130.0, false),
            header_cell("Qty", 50.0, true),
        ]
        .spacing(0),
    )
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(toolbar_bg)),
        ..container::Style::default()
    });

    let rows = [
        (
            "BR1, BR2",
            "SBR1M100",
            "BRIDGE RECT 1P 100V",
            "U-DFN3030",
            "2",
        ),
        (
            "C1, C28, C29, C34…",
            "10µF",
            "CAP CER 10UF 10V",
            "C0603",
            "5",
        ),
        (
            "C2, C3, C7, C8, C17…",
            "0.1µF",
            "CAP CER 0.1UF 10V",
            "C0603",
            "35",
        ),
        ("C4, C61, C65", "4.7µF", "CAP CER 4.7UF 6.3V", "C0603", "3"),
        ("C5, C6", "30pF", "CAP CER 30PF 50V", "C0402", "2"),
        (
            "C9, C10, C11, C12",
            "12nF",
            "CAP CER 0.012UF 16V",
            "C0402",
            "4",
        ),
        ("C13, C14", "1nF", "CAP CER 1000PF 2KV", "C1206", "2"),
        ("C15, C25", "100µF", "CAP ALUM POLY 10V", "WCAP-PHGP", "2"),
        ("C16, C52", "10µF", "CAP CER 10UF 10V", "C0805", "2"),
        (
            "C19, C20, C21, C22…",
            "10µF",
            "CAP CER 10UF 6.3V",
            "C0402",
            "16",
        ),
    ];
    let mut body: Column<Element<'a, Message>> = Column::new();
    for (index, values) in rows.iter().enumerate() {
        let flagged = index == 1;
        let background = if flagged {
            Color { a: 0.10, ..warning }
        } else {
            Color::TRANSPARENT
        };
        let edge = if flagged { warning } else { Color::TRANSPARENT };
        let cell = |value: &str, width: f32, right: bool| -> Element<'a, Message> {
            let align = if right {
                iced::alignment::Horizontal::Right
            } else {
                iced::alignment::Horizontal::Left
            };
            container(text(raw(value.to_string())).size(10).color(text_color))
                .width(Length::Fixed(width))
                .padding([3, 8])
                .align_x(align)
                .boxed()
        };
        let gutter = row![
            container(Space::new())
                .width(2)
                .height(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(edge)),
                    ..container::Style::default()
                }),
            container(text(raw((index + 1).to_string())).size(10).color(muted))
                .width(Length::Fixed(GUTTER_W - 2.0))
                .padding([3, 8])
                .align_x(iced::alignment::Horizontal::Right),
        ]
        .height(Length::Shrink);
        body = body.push(
            container(
                row![
                    gutter,
                    cell(values.0, 170.0, false),
                    cell(values.1, 90.0, false),
                    cell(values.2, 220.0, false),
                    cell(values.3, 130.0, false),
                    cell(values.4, 50.0, true),
                ]
                .spacing(0),
            )
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(background)),
                ..container::Style::default()
            })
            .boxed(),
        );
    }
    column![header, body].boxed()
}

/// General / Columns / Rules tabs; General holds the one not-fitted option
/// and the format.
fn properties_sidebar<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let text_color = theme::color(tokens.text);
    let muted = theme::color(tokens.text_secondary);
    let border = theme::color(tokens.border);
    let sidebar_title = container(text(raw("Properties")).size(13).color(text_color))
        .padding([8, 12])
        .width(Length::Fill);

    let tabs = container(segmented(&["General", "Columns", "Rules"], 0, tokens)).padding([0, 12]);

    let checkbox = |label: &'static str, checked: bool| -> Element<'a, Message> {
        let background = if checked {
            Color::from_rgb(0.00, 0.47, 0.84)
        } else {
            Color::from_rgba(1.0, 1.0, 1.0, 0.04)
        };
        let check = if checked { "✓" } else { " " };
        row![
            container(text(raw(check)).size(9).color(Color::WHITE))
                .width(12)
                .height(12)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(background)),
                    border: Border {
                        width: 1.0,
                        radius: 2.0.into(),
                        color: border,
                        ..Border::default()
                    },
                    ..container::Style::default()
                }),
            Space::new().width(8),
            text(raw(label)).size(11).color(text_color),
        ]
        .align_y(iced::Alignment::Center)
        .boxed()
    };

    let section = |title: &'static str, body: Element<'a, Message>| -> Element<'a, Message> {
        column![
            container(text(raw(title)).size(11).color(muted)).padding([6, 12]),
            container(body).padding([4, 12]),
        ]
        .boxed()
    };

    let bom_items = column![checkbox(
        "Include not-fitted (DNP), excluded-from-BOM and off-board parts",
        false
    )];
    let export_options = row![
        text(raw("Format")).size(11).color(muted),
        Space::new().width(12),
        segmented(&["CSV", "XLSX", "HTML"], 0, tokens),
    ]
    .align_y(iced::Alignment::Center);

    column![
        sidebar_title,
        tabs,
        Space::new().height(8),
        section("BOM Items", bom_items.boxed()),
        Space::new().height(4),
        section("Export Options", export_options.boxed()),
    ]
    .boxed()
}
