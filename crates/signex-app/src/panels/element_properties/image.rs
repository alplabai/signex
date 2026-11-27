//! Properties ▸ Image (#1116, mockup row 204): the file (mono, with Replace…,
//! Show in folder, Relink… when it is missing) and its source facts; Size
//! (Keep aspect ratio, Scale, W / H, Reset to file size); Appearance
//! (Opacity, Placement, Border). A missing file adds a banner with Relink…
//! and Remove image. Every row sends `AnnotationPanelMsg::Image`; the
//! handlers are `app/images/panel.rs`.

use iced::widget::text::Wrapping;
use iced::widget::{Column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{Localized, UserText, ids, t};
use signex_types::schematic::{DrawingPlacement, SchImage};
use signex_widgets::{button, checkbox, text, text_input};

use super::super::{PanelContext, PanelMsg, form_look};
use super::annotation::{AnnotationPanelMsg, Rows, SP, TS};
use crate::app::images::ImagePanelMsg;
use crate::styles::ti;

/// A typed box of Properties ▸ Image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImagePanelField {
    Scale,
    Width,
    Height,
    Opacity,
}

/// What Properties ▸ Image shows besides the image itself.
#[derive(Debug, Clone, Default)]
pub struct ImagePanelState {
    /// The box being typed in: the image, the field and its raw text.
    pub draft: Option<(uuid::Uuid, ImagePanelField, String)>,
    /// The file is gone, refused or holds other content.
    pub missing: bool,
    /// The file's size in bytes, as the last scan read it.
    pub bytes: Option<u64>,
    /// `png`, `jpeg`, `gif` or `svg`, as the last scan read it.
    pub format: Option<String>,
}

fn msg(m: ImagePanelMsg) -> PanelMsg {
    PanelMsg::Annotation(AnnotationPanelMsg::Image(m))
}

/// The mono face a project path is shown in.
fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// A format's name as the Source row shows it: a file-format name, the same
/// in every language.
fn format_name(format: &str) -> String {
    match format {
        "png" => "PNG",
        "jpeg" => "JPEG",
        "gif" => "GIF",
        "svg" => "SVG",
        other => other,
    }
    .to_string()
}

/// A byte count as the Source row shows it.
fn size_text(ctx: &PanelContext, bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        let value = format!("{:.1}", bytes as f64 / (1024.0 * 1024.0));
        t!(ctx.i18n, ids::IMAGES_PANEL_SIZE_MB, "value" => value).into_string()
    } else {
        let value = format!("{}", bytes.div_ceil(1024));
        t!(ctx.i18n, ids::IMAGES_PANEL_SIZE_KB, "value" => value).into_string()
    }
}

/// The Source row's text: format, pixels and size, or "not found" with the
/// last known pixels.
pub(super) fn source_text(ctx: &PanelContext, image: &SchImage) -> Localized {
    let state = &ctx.annotation.image;
    let [w, h] = image.source_px.unwrap_or([0, 0]);
    if state.missing {
        return t!(ctx.i18n, ids::IMAGES_PANEL_SOURCE_MISSING, "width" => w, "height" => h);
    }
    let format = state
        .format
        .as_deref()
        .map_or_else(|| "-".to_string(), format_name);
    let size = state
        .bytes
        .map_or_else(|| "-".to_string(), |b| size_text(ctx, b));
    t!(
        ctx.i18n,
        ids::IMAGES_PANEL_SOURCE,
        "format" => format,
        "width" => w,
        "height" => h,
        "size" => size
    )
}

/// A box of `field` showing `value` or its draft; Enter applies it.
fn input<'a>(
    ctx: &'a PanelContext,
    id: uuid::Uuid,
    field: ImagePanelField,
    value: String,
) -> Element<'a, PanelMsg> {
    let draft = match &ctx.annotation.image.draft {
        Some((d, f, text)) if *d == id && *f == field => Some(text.clone()),
        _ => None,
    };
    let shown = draft.unwrap_or(value);
    text_input(signex_i18n::UserText::new(String::new()), shown)
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .on_input(move |s| msg(ImagePanelMsg::FieldTyped(field, s)))
        .on_submit(msg(ImagePanelMsg::FieldSubmitted(field)))
        .style(form_look::refusable_input_style(None))
        .width(Length::Fill)
        .boxed()
}

/// A box with a caption before it ("W", "H") or a unit after it ("%").
fn labelled_input<'a>(
    rows: &Rows<'a>,
    prefix: Option<signex_i18n::MessageId>,
    field: ImagePanelField,
    value: String,
    unit: Option<signex_i18n::MessageId>,
) -> Element<'a, PanelMsg> {
    let mut line: iced::widget::Row<Element<'a, PanelMsg>> = iced::widget::Row::new()
        .spacing(SP.xs)
        .align_y(Alignment::Center)
        .width(Length::Fill);
    if let Some(prefix) = prefix {
        line = line.push_boxed(text(rows.t(prefix)).size(TS.caption).color(rows.faint()));
    }
    line = line.push(input(rows.ctx, rows.id, field, value));
    if let Some(unit) = unit {
        line = line.push_boxed(text(rows.t(unit)).size(TS.caption).color(rows.faint()));
    }
    line.boxed()
}

/// Two cells, one chosen: Behind / Above, None / Line.
fn segmented<'a>(
    rows: &Rows<'a>,
    cells: [(signex_i18n::MessageId, bool, ImagePanelMsg); 2],
) -> Element<'a, PanelMsg> {
    let cells: Vec<Element<'a, PanelMsg>> = cells
        .into_iter()
        .map(|(label, active, press)| {
            button(
                text(rows.t(label))
                    .size(TS.body)
                    .align_x(iced::alignment::Horizontal::Center)
                    .wrapping(Wrapping::None),
            )
            .padding([SP.s, SP.xs])
            .on_press(msg(press))
            .style(move |theme: &Theme, status: button::Status| {
                let hovered = matches!(status, button::Status::Hovered);
                form_look::segment_cell(theme, active, hovered && !active)
            })
            .boxed()
        })
        .collect();
    container(signex_widgets::FitRow::new(cells).fill(true))
        .padding(SP.xs)
        .width(Length::Fill)
        .style(form_look::segmented_frame)
        .boxed()
}

/// The missing-file banner: the path, Relink… and Remove image.
fn missing_banner<'a>(rows: &Rows<'a>, image: &SchImage) -> Element<'a, PanelMsg> {
    let ctx = rows.ctx;
    let warn = signex_widgets::theme_ext::warning_color(&ctx.tokens);
    let body: Column<Element<'a, PanelMsg>> = Column::new()
        .spacing(SP.xs)
        .push_boxed(
            text(rows.t(ids::IMAGES_PANEL_NOT_FOUND))
                .size(TS.body)
                .color(warn),
        )
        .push_boxed(
            text(UserText::new(image.file.clone()))
                .size(TS.caption)
                .color(rows.ink())
                .font(mono()),
        )
        .push_boxed(
            row![
                rows.action(ids::IMAGES_PANEL_RELINK, msg(ImagePanelMsg::RelinkPressed)),
                rows.action(ids::IMAGES_PANEL_REMOVE, msg(ImagePanelMsg::RemovePressed)),
            ]
            .spacing(SP.s),
        );
    container(
        container(body)
            .padding(SP.s)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(iced::Color { a: 0.08, ..warn })),
                border: Border {
                    width: 1.0,
                    color: warn,
                    radius: 4.0.into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }),
    )
    .padding([SP.s, SP.l])
    .width(Length::Fill)
    .boxed()
}

fn file_rows<'a>(rows: &Rows<'a>, image: &SchImage) -> Vec<Element<'a, PanelMsg>> {
    let missing = rows.ctx.annotation.image.missing;
    let path = text(UserText::new(image.file.clone()))
        .size(TS.body)
        .color(rows.ink())
        .font(mono())
        .wrapping(Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::Start);
    let mut buttons = vec![rows.action(
        ids::IMAGES_PANEL_REPLACE,
        msg(ImagePanelMsg::ReplacePressed),
    )];
    if missing {
        buttons.push(rows.action(ids::IMAGES_PANEL_RELINK, msg(ImagePanelMsg::RelinkPressed)));
    } else {
        buttons.push(rows.action(
            ids::IMAGES_PANEL_SHOW_IN_FOLDER,
            msg(ImagePanelMsg::ShowInFolderPressed),
        ));
    }
    let source_ink = if missing {
        signex_widgets::theme_ext::warning_color(&rows.ctx.tokens)
    } else {
        rows.faint()
    };
    vec![
        rows.field(rows.t(ids::IMAGES_PANEL_FILE), path.boxed()),
        rows.field(
            rows.t(ids::IMAGES_PANEL_BLANK),
            iced::widget::Row::with_children(buttons)
                .spacing(SP.s)
                .boxed(),
        ),
        rows.field(
            rows.t(ids::IMAGES_PANEL_SOURCE_LABEL),
            text(source_text(rows.ctx, image))
                .size(TS.caption)
                .color(source_ink)
                .boxed(),
        ),
    ]
}

fn size_rows<'a>(rows: &Rows<'a>, image: &SchImage) -> Vec<Element<'a, PanelMsg>> {
    let keep = checkbox(image.keep_aspect)
        .size(TS.heading)
        .style(form_look::checkbox_style)
        .on_toggle(|on| msg(ImagePanelMsg::KeepAspectToggled(on)));
    let keep = row![
        keep,
        text(rows.t(ids::IMAGES_PANEL_KEEP_ASPECT))
            .size(TS.body)
            .color(rows.ink())
            .wrapping(Wrapping::None),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    let scale = image
        .scale_percent()
        .map_or_else(String::new, |s| format!("{s:.1}"));
    let pair = row![
        labelled_input(
            rows,
            Some(ids::IMAGES_PANEL_W),
            ImagePanelField::Width,
            format!("{:.2}", image.width),
            None,
        ),
        labelled_input(
            rows,
            Some(ids::IMAGES_PANEL_H),
            ImagePanelField::Height,
            format!("{:.2}", image.height),
            None,
        ),
    ]
    .spacing(SP.s)
    .width(Length::Fill);
    let reset = button(
        text(rows.t(ids::IMAGES_PANEL_RESET))
            .size(TS.caption)
            .color(ti(rows.ctx.tokens.accent)),
    )
    .padding(0)
    .on_press_maybe(
        image
            .source_px
            .map(|_| msg(ImagePanelMsg::ResetToFileSizePressed)),
    )
    .style(|_: &Theme, _| button::Style::default());
    vec![
        container(keep).padding([SP.xs, SP.l]).boxed(),
        rows.field(
            rows.t(ids::IMAGES_PANEL_SCALE),
            labelled_input(
                rows,
                None,
                ImagePanelField::Scale,
                scale,
                Some(ids::IMAGES_PANEL_PERCENT),
            ),
        ),
        rows.field(rows.t(ids::IMAGES_PANEL_SIZE), pair.boxed()),
        rows.field(rows.t(ids::IMAGES_PANEL_BLANK), reset.boxed()),
    ]
}

fn appearance_rows<'a>(rows: &Rows<'a>, image: &SchImage) -> Vec<Element<'a, PanelMsg>> {
    let placement = rows.ctx.selected_drawing_placement;
    let behind = placement == DrawingPlacement::Behind;
    vec![
        rows.field(
            rows.t(ids::IMAGES_PANEL_OPACITY),
            labelled_input(
                rows,
                None,
                ImagePanelField::Opacity,
                image.opacity.to_string(),
                Some(ids::IMAGES_PANEL_PERCENT),
            ),
        ),
        rows.field(
            rows.t(ids::IMAGES_PANEL_PLACEMENT),
            segmented(
                rows,
                [
                    (
                        ids::PANELS_DRAWING_PLACEMENT_BEHIND,
                        behind,
                        ImagePanelMsg::PlacementPicked(DrawingPlacement::Behind),
                    ),
                    (
                        ids::PANELS_DRAWING_PLACEMENT_ABOVE,
                        !behind,
                        ImagePanelMsg::PlacementPicked(DrawingPlacement::Above),
                    ),
                ],
            ),
        ),
        rows.field(
            rows.t(ids::IMAGES_PANEL_BORDER),
            segmented(
                rows,
                [
                    (
                        ids::IMAGES_PANEL_BORDER_NONE,
                        !image.border,
                        ImagePanelMsg::BorderPicked(false),
                    ),
                    (
                        ids::IMAGES_PANEL_BORDER_LINE,
                        image.border,
                        ImagePanelMsg::BorderPicked(true),
                    ),
                ],
            ),
        ),
    ]
}

/// Properties of the selected image.
pub(super) fn view_image_properties<'a>(
    ctx: &'a PanelContext,
    image: &SchImage,
) -> Element<'a, PanelMsg> {
    let rows = Rows {
        ctx,
        id: image.uuid,
    };
    let mut col = Column::new().spacing(0).width(Length::Fill);
    col = col.push(rows.header(
        crate::icons::icon_dd_graphic(ctx.theme_id),
        rows.t(ids::IMAGES_PANEL_TITLE),
        UserText::new(image.file_name().to_string()),
    ));
    if ctx.annotation.image.missing {
        col = col.push(missing_banner(&rows, image));
    }
    col = col.push(rows.section(
        "image_file",
        ids::IMAGES_PANEL_SECTION_IMAGE,
        file_rows(&rows, image),
    ));
    col = col.push(rows.section(
        "image_size",
        ids::IMAGES_PANEL_SECTION_SIZE,
        size_rows(&rows, image),
    ));
    col = col.push(rows.section(
        "image_appearance",
        ids::IMAGES_PANEL_SECTION_APPEARANCE,
        appearance_rows(&rows, image),
    ));
    col.boxed()
}
