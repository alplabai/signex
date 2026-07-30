//! Bill-of-materials modal catalog slice.

use iced::widget::{Column, Space, column, container, row, svg, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

use crate::catalog::Message;
use crate::icon::x_handle;
use crate::theme;

pub(crate) fn view<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    const MODAL_WIDTH: f32 = 1100.0;
    const MODAL_HEIGHT: f32 = 660.0;
    const HEADER_HEIGHT: f32 = 28.0;
    const SIDEBAR_WIDTH: f32 = 320.0;

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
        });
    let header = container(
        row![
            text("Bill of Materials for Variant [Production] of Project [Loratis-SN]")
                .size(13)
                .color(text_color),
            Space::new().width(Length::Fill),
            container(close_icon)
                .width(46)
                .height(HEADER_HEIGHT)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.78, 0.22, 0.22, 1.0))),
                    border: Border {
                        radius: iced::border::Radius::default().top_right(8.0),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }),
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
        },
        ..container::Style::default()
    });

    let toolbar = container(
        row![
            dropdown("Production", tokens),
            Space::new().width(Length::Fill),
            info_icon(tokens),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(4),
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
            .padding([6, 6])
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

    let status = container(
        row![
            text("84 of 84 lines visible").size(11).color(muted),
            Space::new().width(16),
            text("|").size(11).color(muted),
            Space::new().width(16),
            text("Current variant: Production").size(11).color(muted),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([6, 14])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(toolbar_bg)),
        ..container::Style::default()
    });

    let buttons = container(
        row![
            Space::new().width(Length::Fill),
            secondary_button("Export…", tokens),
            Space::new().width(8),
            primary_button("OK", accent, text_color),
            Space::new().width(8),
            secondary_button("Cancel", tokens),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .width(Length::Fill);

    container(
        column![header, toolbar, content, status, buttons]
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
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..container::Style::default()
    })
    .clip(true)
    .into()
}

fn dropdown<'a>(label: &str, tokens: &ThemeTokens) -> Element<'a, Message> {
    let text_color = theme::color(tokens.text);
    let border = theme::color(tokens.border);
    container(
        row![
            Space::new().width(Length::Fill),
            text(label.to_string()).size(11).color(text_color),
            Space::new().width(Length::Fill),
            text("▾").size(10).color(text_color),
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(140)
    .height(24)
    .padding([0, 10])
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: border,
        },
        ..container::Style::default()
    })
    .into()
}

fn secondary_button<'a>(label: &str, tokens: &ThemeTokens) -> Element<'a, Message> {
    let text_color = theme::color(tokens.text);
    let border = theme::color(tokens.border);
    container(text(label.to_string()).size(11).color(text_color))
        .padding([5, 14])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..container::Style::default()
        })
        .into()
}

fn primary_button<'a>(label: &str, accent: Color, text_color: Color) -> Element<'a, Message> {
    container(text(label.to_string()).size(11).color(text_color))
        .padding([5, 18])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(accent)),
            border: Border {
                width: 0.0,
                radius: 3.0.into(),
                color: Color::TRANSPARENT,
            },
            ..container::Style::default()
        })
        .into()
}

fn info_icon<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let accent = theme::color(tokens.accent);
    container(text("i").size(11).color(Color::WHITE))
        .width(20)
        .height(20)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(Color { a: 0.7, ..accent })),
            border: Border {
                width: 0.0,
                radius: 10.0.into(),
                color: Color::TRANSPARENT,
            },
            ..container::Style::default()
        })
        .into()
}

fn table<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let text_color = theme::color(tokens.text);
    let muted = theme::color(tokens.text_secondary);
    let toolbar_bg = theme::color(tokens.toolbar_bg);
    let header_cell = |label: &'static str, width: f32| -> Element<'a, Message> {
        container(text(label).size(11).color(text_color))
            .width(Length::Fixed(width))
            .padding([4, 8])
            .into()
    };
    let header = container(
        row![
            header_cell("#", 32.0),
            header_cell("Name", 140.0),
            header_cell("Description", 200.0),
            header_cell("Designator", 160.0),
            header_cell("Footprint", 140.0),
            header_cell("LibRef", 120.0),
            header_cell("Quantity", 70.0),
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
            "1",
            "SBR1M100BLP-7",
            "BRIDGE RECT 1P 100V",
            "BR1, BR2",
            "DIODES U-DFN303…",
            "SBR1M100BLP-7",
            "2",
        ),
        (
            "2",
            "10µF",
            "CAP CER 10UF 10V",
            "C1, C28, C29, C34…",
            "CAP 0603/1608",
            "GRM188Z71A106K…",
            "5",
        ),
        (
            "3",
            "0.1µF",
            "CAP CER 0.1UF 10V…",
            "C2, C3, C7, C8, C17…",
            "CAP 0603/1608",
            "C0603X7S1A104K03…",
            "35",
        ),
        (
            "4",
            "4.7µF",
            "CAP CER 4.7UF 6.3…",
            "C4, C61, C65",
            "CAP 0603/1608",
            "CL10B475K6JNQNC",
            "3",
        ),
        (
            "5",
            "30pF",
            "CAP CER 30PF 50V…",
            "C5, C6",
            "CAP 0402/1005",
            "GRM1555C1H300J…",
            "2",
        ),
        (
            "6",
            "12nF",
            "CAP CER 0.012UF 1…",
            "C9, C10, C11, C12",
            "CAP 0402/1005",
            "06031C123KAT2A",
            "4",
        ),
        (
            "7",
            "1nF",
            "CAP CER 1000PF 2K…",
            "C13, C14",
            "CAP 1206/3216",
            "CL31B102KJHNNNE",
            "2",
        ),
        (
            "8",
            "100µF",
            "CAP ALUM POLY 10…",
            "C15, C25",
            "WURTH WCAP-PH…",
            "875015119003",
            "2",
        ),
        (
            "9",
            "10µF",
            "CAP CER 10UF 10V…",
            "C16, C52",
            "CAP 0805/2012",
            "C2012X7R1A106K1…",
            "2",
        ),
        (
            "10",
            "10µF",
            "CAP CER 10UF 6.3V…",
            "C19, C20, C21, C22…",
            "CAP 0402/1005",
            "C0402X5R1A106K…",
            "16",
        ),
    ];
    let mut body: Column<'a, Message> = Column::new();
    for (index, values) in rows.iter().enumerate() {
        let background = if index % 2 == 0 {
            Color::TRANSPARENT
        } else {
            Color::from_rgba(1.0, 1.0, 1.0, 0.025)
        };
        let cell = |value: &str, width: f32| -> Element<'a, Message> {
            container(text(value.to_string()).size(10).color(text_color))
                .width(Length::Fixed(width))
                .padding([3, 8])
                .into()
        };
        body = body.push(
            container(
                row![
                    container(text(values.0).size(10).color(muted))
                        .width(Length::Fixed(32.0))
                        .padding([3, 8])
                        .align_x(iced::alignment::Horizontal::Right),
                    cell(values.1, 140.0),
                    cell(values.2, 200.0),
                    cell(values.3, 160.0),
                    cell(values.4, 140.0),
                    cell(values.5, 120.0),
                    cell(values.6, 70.0),
                ]
                .spacing(0),
            )
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(background)),
                ..container::Style::default()
            }),
        );
    }
    column![header, body].into()
}

fn properties_sidebar<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let text_color = theme::color(tokens.text);
    let muted = theme::color(tokens.text_secondary);
    let border = theme::color(tokens.border);
    let accent = theme::color(tokens.accent);
    let sidebar_title = container(text("Properties").size(13).color(text_color))
        .padding([8, 12])
        .width(Length::Fill);

    let tab = |label: &'static str, active: bool| -> Element<'a, Message> {
        let background = if active {
            Color { a: 0.18, ..accent }
        } else {
            Color::TRANSPARENT
        };
        container(text(label).size(11).color(text_color))
            .padding([4, 12])
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(background)),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..container::Style::default()
            })
            .into()
    };
    let tabs = row![
        tab("General", true),
        Space::new().width(4),
        tab("Columns", false)
    ]
    .padding([0, 12]);

    let checkbox = |label: &'static str, checked: bool| -> Element<'a, Message> {
        let background = if checked {
            Color::from_rgb(0.00, 0.47, 0.84)
        } else {
            Color::from_rgba(1.0, 1.0, 1.0, 0.04)
        };
        let check = if checked { "✓" } else { " " };
        row![
            container(text(check).size(9).color(Color::WHITE))
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
                    },
                    ..container::Style::default()
                }),
            Space::new().width(8),
            text(label).size(11).color(text_color),
        ]
        .align_y(iced::Alignment::Center)
        .into()
    };

    let field = |label: &'static str, value: &'static str| -> Element<'a, Message> {
        row![
            container(text(label).size(11).color(muted)).width(Length::Fixed(80.0)),
            container(
                row![
                    text(value).size(11).color(text_color),
                    Space::new().width(Length::Fill),
                    text("▾").size(10).color(text_color),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([4, 8])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..container::Style::default()
            }),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(8)
        .into()
    };

    let section = |title: &'static str, body: Element<'a, Message>| -> Element<'a, Message> {
        let header = container(
            row![
                text("▾").size(10).color(muted),
                Space::new().width(6),
                text(title).size(11).color(text_color),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([6, 12])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.025))),
            ..container::Style::default()
        });
        column![header, container(body).padding([8, 12])].into()
    };

    let bom_items = column![
        checkbox("Show Not Fitted", false),
        Space::new().height(6),
        checkbox("Include DB Parameters in Variations", false),
    ];
    let export_options = column![
        field("File Format", "MS-Excel (*.xls, *.xlsx, *.xlsm)"),
        Space::new().height(8),
        field("Template", "No Template"),
        Space::new().height(8),
        checkbox("Add to Project", false),
        Space::new().height(6),
        checkbox("Open Exported", false),
    ];

    column![
        sidebar_title,
        tabs,
        Space::new().height(8),
        section("BOM Items", bom_items.into()),
        Space::new().height(4),
        section("Export Options", export_options.into()),
    ]
    .into()
}
