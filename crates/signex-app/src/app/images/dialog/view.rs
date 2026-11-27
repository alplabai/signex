//! The Place Image card (mockup row 204): the project's images as a filtered
//! grid of thumbnails at the left, the chosen one's preview, facts and size
//! at the right, Cancel and Place in the footer. Pure: it reads the dialog
//! state, whose pictures were read in a `Task`.

use iced::widget::{Column, Row, Space, column, container, image, row, scrollable, svg};
use iced::{Alignment, Background, Border, ContentFit, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, checkbox, text, text_input};

use super::{DialogFile, PlaceDialog, PlaceDialogMsg};
use crate::app::images::ImageEntry;
use crate::styles::{
    DialogButtonKind, Radii, Spacing, TypeScale, chrome_px, dialog_button, dialog_footer_split, ti,
    tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Card width (640 px at the default base).
const DIALOG_W: f32 = chrome_px(640.0 * 11.0 / 12.0);
/// The left column of thumbnails (280 px at the default base).
const LIST_W: f32 = chrome_px(280.0 * 11.0 / 12.0);
/// A thumbnail's picture (72 px high at the default base).
const THUMB_H: f32 = chrome_px(72.0 * 11.0 / 12.0);
/// The thumbnail grid's height before it scrolls.
const GRID_H: f32 = chrome_px(300.0 * 11.0 / 12.0);
/// The preview's height (190 px at the default base).
const PREVIEW_H: f32 = chrome_px(190.0 * 11.0 / 12.0);
/// The facts' label column (64 px at the default base).
const FACT_W: f32 = chrome_px(64.0 * 11.0 / 12.0);

/// The picture of `entry`, fitted into its box.
fn picture<'a>(entry: Option<&ImageEntry>) -> Element<'a, PlaceDialogMsg> {
    match entry {
        Some(ImageEntry::Raster(handle)) => image(handle.clone())
            .content_fit(ContentFit::Contain)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed(),
        Some(ImageEntry::Vector(handle)) => svg(handle.clone())
            .content_fit(ContentFit::Contain)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed(),
        _ => Space::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed(),
    }
}

/// A framed box on white, as a picture is placed on paper.
fn paper<'a>(
    content: Element<'a, PlaceDialogMsg>,
    height: f32,
    tokens: &ThemeTokens,
) -> Element<'a, PlaceDialogMsg> {
    let edge = ti(tokens.border);
    container(content)
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .padding(SP.xs)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(iced::Color::WHITE)),
            border: Border {
                width: 1.0,
                color: edge,
                radius: Radii::GRAPHITE.control.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The facts line under a thumbnail: format and size, "new" for a pick the
/// save copies in, or why the file was refused.
fn thumb_meta(file: &DialogFile, i18n: &I18n) -> Localized {
    match &file.read {
        Ok(_) if file.is_new => t!(i18n, ids::IMAGES_DIALOG_NEW),
        Ok(picked) => t!(
            i18n,
            ids::IMAGES_DIALOG_THUMB_META,
            "format" => picked.facts.format.name(),
            "kb" => picked.facts.bytes.div_ceil(1024)
        ),
        Err(_) => t!(i18n, ids::IMAGES_DIALOG_REFUSED),
    }
}

fn thumb<'a>(
    index: usize,
    file: &'a DialogFile,
    chosen: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PlaceDialogMsg> {
    let r = tokens::resolve(tokens);
    let accent = ti(tokens.accent);
    let face = column![
        paper(picture(file.thumb.as_ref()), THUMB_H, tokens),
        text(UserText::new(file.name().to_string()))
            .size(TS.caption)
            .color(ti(tokens.text))
            .wrapping(iced::widget::text::Wrapping::None)
            .ellipsis(iced::widget::text::Ellipsis::End),
        text(thumb_meta(file, i18n)).size(TS.caption).color(r.ink3),
    ]
    .spacing(SP.xs)
    .width(Length::Fill);
    button(face)
        .padding(SP.xs)
        .width(Length::Fill)
        .on_press(PlaceDialogMsg::Chosen(index))
        .style(move |_: &Theme, status: button::Status| {
            let hovered = matches!(status, button::Status::Hovered);
            button::Style {
                background: (chosen || hovered)
                    .then(|| Background::Color(iced::Color { a: 0.12, ..accent })),
                border: Border {
                    width: if chosen { 1.5 } else { 0.0 },
                    color: accent,
                    radius: Radii::GRAPHITE.control.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        })
        .boxed()
}

/// The left column: the filter, the thumbnails two a row, Add image file…
fn list<'a>(
    dialog: &'a PlaceDialog,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PlaceDialogMsg> {
    let r = tokens::resolve(tokens);
    let shown: Vec<(usize, &DialogFile)> = dialog.shown().collect();
    let mut grid: Column<Element<'a, PlaceDialogMsg>> = Column::new().spacing(SP.s);
    for pair in shown.chunks(2) {
        let mut line: Row<Element<'a, PlaceDialogMsg>> = Row::new().spacing(SP.s);
        for &(index, file) in pair {
            line = line.push(thumb(
                index,
                file,
                dialog.chosen == Some(index),
                tokens,
                i18n,
            ));
        }
        if pair.len() == 1 {
            line = line.push_boxed(Space::new().width(Length::Fill));
        }
        grid = grid.push(line.boxed());
    }
    let grid: Element<'a, PlaceDialogMsg> = if dialog.loading {
        text(t!(i18n, ids::IMAGES_DIALOG_LOADING))
            .size(TS.caption)
            .color(r.ink3)
            .boxed()
    } else if shown.is_empty() {
        text(t!(i18n, ids::IMAGES_DIALOG_EMPTY))
            .size(TS.caption)
            .color(r.ink3)
            .boxed()
    } else {
        scrollable(grid)
            .height(Length::Fixed(GRID_H))
            .width(Length::Fill)
            .boxed()
    };
    column![
        row![
            text(t!(i18n, ids::IMAGES_DIALOG_PROJECT_IMAGES))
                .size(TS.caption)
                .color(ti(tokens.text)),
            text(UserText::new(
                signex_types::image_asset::IMAGE_ASSET_DIR.to_string() + "/"
            ))
            .size(TS.caption)
            .color(r.ink3),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
        text_input(t!(i18n, ids::IMAGES_DIALOG_FILTER), dialog.filter.as_str())
            .on_input(PlaceDialogMsg::FilterChanged)
            .size(TS.body)
            .padding([SP.xs, SP.s])
            .width(Length::Fill),
        grid,
        dialog_button(
            t!(i18n, ids::IMAGES_DIALOG_ADD_FILE),
            DialogButtonKind::Secondary,
            Some(PlaceDialogMsg::AddFilePressed),
            tokens,
        ),
        text(t!(i18n, ids::IMAGES_DIALOG_ADD_HINT))
            .size(TS.caption)
            .color(r.ink3),
    ]
    .spacing(SP.s)
    .width(Length::Fixed(LIST_W))
    .boxed()
}

/// One fact: a label and its value.
fn fact<'a>(
    label: Localized,
    value: Localized,
    tokens: &ThemeTokens,
) -> Element<'a, PlaceDialogMsg> {
    let r = tokens::resolve(tokens);
    row![
        text(label).size(TS.caption).color(r.ink3).width(FACT_W),
        text(value).size(TS.caption).color(ti(tokens.text)),
    ]
    .spacing(SP.s)
    .boxed()
}

/// The right column: the preview, the facts and the size to place at.
fn detail<'a>(
    dialog: &'a PlaceDialog,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PlaceDialogMsg> {
    let r = tokens::resolve(tokens);
    let chosen = dialog.chosen.and_then(|i| dialog.files.get(i));
    let mut out: Column<Element<'a, PlaceDialogMsg>> =
        Column::new().spacing(SP.s).width(Length::Fill);
    out = out.push(paper(
        picture(chosen.and_then(|f| f.thumb.as_ref())),
        PREVIEW_H,
        tokens,
    ));
    let Some(file) = chosen else {
        return out.boxed();
    };
    out = out.push(fact(
        t!(i18n, ids::IMAGES_DIALOG_FACT_FILE),
        t!(i18n, ids::IMAGES_DIALOG_FILE, "file" => file.file.as_str()),
        tokens,
    ));
    let Ok(picked) = &file.read else {
        return out.boxed();
    };
    let facts = &picked.facts;
    let format = if facts.format.is_vector() {
        t!(i18n, ids::IMAGES_DIALOG_FORMAT_VECTOR, "format" => facts.format.name())
    } else {
        t!(i18n, ids::IMAGES_DIALOG_FORMAT_RASTER, "format" => facts.format.name())
    };
    out = out.push(fact(
        t!(i18n, ids::IMAGES_DIALOG_FACT_FORMAT),
        format,
        tokens,
    ));
    out = out.push(fact(
        t!(i18n, ids::IMAGES_DIALOG_FACT_SIZE),
        t!(
            i18n,
            ids::IMAGES_DIALOG_SIZE,
            "width" => facts.width_px,
            "height" => facts.height_px,
            "kb" => facts.bytes.div_ceil(1024)
        ),
        tokens,
    ));
    let used = if file.is_new {
        t!(i18n, ids::IMAGES_DIALOG_COPIED_ON_SAVE)
    } else {
        t!(i18n, ids::IMAGES_DIALOG_PROJECT_FILE)
    };
    out = out.push(fact(t!(i18n, ids::IMAGES_DIALOG_FACT_USED), used, tokens));
    out = out.push_boxed(
        text(t!(i18n, ids::IMAGES_DIALOG_PLACE_AT))
            .size(TS.caption)
            .color(r.ink3),
    );
    let width_ok = dialog.at_file_size || dialog.placed_width().is_some();
    let edge = if width_ok {
        None
    } else {
        Some(tokens::danger(tokens))
    };
    out = out.push(
        row![
            text(t!(i18n, ids::IMAGES_DIALOG_WIDTH))
                .size(TS.body)
                .color(ti(tokens.text)),
            text_input(UserText::new(String::new()), dialog.width.as_str())
                .on_input(PlaceDialogMsg::WidthTyped)
                .size(TS.body)
                .padding([SP.xs, SP.s])
                .width(Length::Fixed(chrome_px(92.0 * 11.0 / 12.0)))
                .style(crate::panels::form_look::refusable_input_style(edge)),
            text(t!(i18n, ids::IMAGES_DIALOG_HEIGHT_FOLLOWS))
                .size(TS.caption)
                .color(r.ink3),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .boxed(),
    );
    let (w, h) = facts.natural_size_mm();
    out = out.push(
        row![
            checkbox(dialog.at_file_size).on_toggle(PlaceDialogMsg::FileSizePicked),
            column![
                text(t!(i18n, ids::IMAGES_DIALOG_FILE_SIZE))
                    .size(TS.body)
                    .color(ti(tokens.text)),
                text(t!(
                    i18n,
                    ids::IMAGES_DIALOG_FILE_SIZE_MM,
                    "width" => format!("{w:.2}"),
                    "height" => format!("{h:.2}")
                ))
                .size(TS.caption)
                .color(r.ink3),
            ],
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .boxed(),
    );
    out.boxed()
}

/// The Place Image card over a scrim, centred on the window.
pub(crate) fn view<'a>(
    dialog: &'a PlaceDialog,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PlaceDialogMsg> {
    let sheet = dialog
        .target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let header = crate::app::view::dialogs::graphite_header(
        t!(i18n, ids::IMAGES_DIALOG_TITLE).into_string(),
        Some(t!(i18n, ids::IMAGES_DIALOG_ON_SHEET, "sheet" => sheet).into_string()),
        None,
        PlaceDialogMsg::Closed,
        crate::app::view::dialogs::DIALOG_HEADER_H,
        tokens,
    );
    let body = container(
        row![list(dialog, tokens, i18n), detail(dialog, tokens, i18n)]
            .spacing(SP.l)
            .width(Length::Fill),
    )
    .padding(SP.l)
    .width(Length::Fill);
    let can_place = dialog.chosen_pick().is_some() && dialog.placed_width().is_some();
    let footer = dialog_footer_split(
        vec![
            text(t!(i18n, ids::IMAGES_DIALOG_STORES_PATH))
                .size(TS.caption)
                .color(tokens::resolve(tokens).ink3)
                .boxed(),
        ],
        vec![
            dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                DialogButtonKind::Cancel,
                Some(PlaceDialogMsg::Closed),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::IMAGES_DIALOG_PLACE),
                DialogButtonKind::Primary,
                can_place.then_some(PlaceDialogMsg::PlacePressed),
                tokens,
            ),
        ],
    );
    let card = container(column![header, body, footer].width(Length::Fixed(DIALOG_W)))
        .style(crate::styles::modal_card(tokens))
        .clip(true);
    container(
        column![
            Space::new().height(Length::Fill),
            row![
                Space::new().width(Length::Fill),
                card,
                Space::new().width(Length::Fill),
            ],
            Space::new().height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(crate::app::view::dialogs::scrim_style(tokens))
    .boxed()
}
