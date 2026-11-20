//! Tools ▸ Document Options modal for a library's symbol and footprint
//! sheets (mockup row 132).
//!
//! Per-`.snxlib` view settings: sheet color, grid visibility, grid
//! spacing, coordinate display unit. Edits go to a working
//! [`crate::library::state::DocumentOptionsModalState::draft`];
//! Apply commits to `OpenLibrary.display`; Cancel discards.
//!
//! The body is a form grid with a fixed label column. Grid Spacing depends
//! on the Grid switch: while the grid is hidden it is greyed and takes no
//! input (disable-when-off).
//!
//! Mounted as a full-screen overlay backdrop via
//! `app/view/mod.rs::collect_overlays` when
//! `LibraryState::document_options.is_some()`.

use iced::widget::{Space, column, container, pick_list, row};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::enable_section::dependents_veil;
use signex_widgets::inert::Inert;
use signex_widgets::{
    Label, LengthFormat, SegmentedStyle, ToggleSwitchStyle, button, segmented, text, theme_ext,
    toggle_switch,
};

use super::messages::LibraryMessage;
use super::modal_chrome::{cancel_btn, footer_strip, header_with_note_closing, primary_btn};
use super::state::DocumentOptionsModalState;
use crate::canvas::grid::{GRID_SIZES_MM, pitch_mm_exact};
use crate::choice::Choice;
use crate::panels::SheetColor;
use crate::styles::tokens::{accent, on_accent, resolve};
use crate::styles::{Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The form's label column, px (the mock's `.frm` grid).
const LABEL_W: f32 = 120.0;
/// Width of the Grid Spacing dropdown, px. The mock's 130 less 10, so the
/// caption of all four presets stays on one line in the 460 px card.
const GRID_PICK_W: f32 = 120.0;
/// Width of the Unit segmented control: 60 px a segment, four segments.
const UNIT_SEG_W: f32 = 240.0;
/// Diameter of the Sheet Color swatch, px.
const SWATCH: f32 = 12.0;

/// The display units, in the segmented control's order.
const UNITS: [Unit; 4] = [Unit::Mm, Unit::Mil, Unit::Inch, Unit::Micrometer];

pub fn view<'a>(
    state: &'a DocumentOptionsModalState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let ink3 = resolve(tokens).ink3;
    let draft = &state.draft;

    let header = header_with_note_closing(
        &i18n.message_plain(ids::DIALOG_TITLE_DOCUMENT_OPTIONS, &[]),
        Some(i18n.message_plain(
            ids::LIBRARY_DOCUMENT_OPTIONS_NOTE,
            &[("name", state.library_name.clone().into())],
        )),
        LibraryMessage::DocumentOptionsCancel,
        windowed,
        tokens,
    );

    let spacing_row = form_row(
        t!(i18n, ids::LIBRARY_DOCUMENT_OPTIONS_GRID_SPACING),
        grid_spacing(draft.grid_size_mm, i18n, tokens),
        ink3,
    );
    // Disable-when-off: the spacing is greyed and inert while the grid is
    // hidden; it keeps its value for when the grid is shown again.
    let spacing_row = match dependents_veil(ti(tokens.paper), draft.grid_visible) {
        Some(veil) => Inert::new(spacing_row, veil).boxed(),
        None => spacing_row,
    };

    let body = column![
        form_row(
            t!(i18n, ids::LIBRARY_DOCUMENT_OPTIONS_SHEET_COLOR),
            sheet_color(draft.sheet_color, i18n, tokens),
            ink3,
        ),
        form_row(
            t!(i18n, ids::LIBRARY_DOCUMENT_OPTIONS_GRID),
            grid_switch(draft.grid_visible, i18n, tokens),
            ink3,
        ),
        spacing_row,
        form_row(
            t!(i18n, ids::LIBRARY_DOCUMENT_OPTIONS_UNIT),
            unit_segmented(draft.unit, i18n, tokens),
            ink3,
        ),
    ]
    .spacing(SP.m);

    let footer = footer_strip(
        Vec::new(),
        vec![
            cancel_btn(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                LibraryMessage::DocumentOptionsCancel,
                tokens,
            ),
            primary_btn(
                t!(i18n, ids::DIALOG_BUTTON_APPLY),
                LibraryMessage::DocumentOptionsApply,
                tokens,
            ),
        ],
        tokens,
    );

    container(
        column![
            header,
            container(body).padding(SP.xl).width(Length::Fill),
            footer
        ]
        .width(super::card_len(
            windowed,
            super::modal_chrome::DOCUMENT_OPTIONS_W,
        )),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

/// One row of the form grid: the muted label in the fixed column, then the
/// control.
fn form_row<'a>(
    label: impl Into<Label<'a>>,
    value: Element<'a, LibraryMessage>,
    ink3: Color,
) -> Element<'a, LibraryMessage> {
    row![
        text(label)
            .size(TS.body)
            .color(ink3)
            .width(LABEL_W)
            .wrapping(iced::widget::text::Wrapping::None),
        value,
    ]
    .spacing(SP.l)
    .align_y(iced::Alignment::Center)
    .width(Length::Fill)
    .boxed()
}

/// Sheet Color: a swatch of the picked preset, then the preset dropdown.
fn sheet_color<'a>(
    picked: SheetColor,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let choices = Choice::named(SheetColor::ALL, sheet_color_label, i18n);
    let selected = crate::choice::chosen(&choices, &picked);
    let fill = picked.to_color();
    let edge = resolve(tokens).line_strong;
    let swatch = container(Space::new().width(SWATCH).height(SWATCH)).style(move |_: &Theme| {
        container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: 1.0,
                color: edge,
                radius: (SWATCH / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        }
    });
    let picker = pick_list(selected, choices, |c: &Choice<SheetColor>| c.label.clone())
        .graphite()
        .on_select(|c: Choice<SheetColor>| LibraryMessage::DocumentOptionsSetSheetColor(c.value))
        .padding([SP.xs, SP.s])
        .width(Length::Fill);
    row![swatch, picker]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center)
        .boxed()
}

/// The catalog name of a sheet colour preset (the Properties panel's names).
fn sheet_color_label(color: SheetColor) -> MessageId {
    match color {
        SheetColor::Black => ids::PROPERTIES_SHEET_COLOR_BLACK,
        SheetColor::White => ids::PROPERTIES_SHEET_COLOR_WHITE,
        SheetColor::DarkGray => ids::PROPERTIES_SHEET_COLOR_DARK_GRAY,
        SheetColor::LightGray => ids::PROPERTIES_SHEET_COLOR_LIGHT_GRAY,
        SheetColor::Cream => ids::PROPERTIES_SHEET_COLOR_CREAM,
    }
}

/// Grid: the visibility switch and its "Visible" label; both toggle it.
fn grid_switch<'a>(
    is_visible: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let style = ToggleSwitchStyle::new(
        accent(tokens),
        resolve(tokens).line_strong,
        on_accent(tokens),
    );
    let label = button(
        text(t!(i18n, ids::LIBRARY_DOCUMENT_OPTIONS_GRID_VISIBLE))
            .size(TS.body)
            .color(theme_ext::text_primary(tokens)),
    )
    .padding(0)
    .style(|_: &Theme, _| signex_widgets::button::Style::default())
    .on_press(LibraryMessage::DocumentOptionsToggleGrid);
    row![
        toggle_switch(
            is_visible,
            style,
            Some(LibraryMessage::DocumentOptionsToggleGrid)
        ),
        label,
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// The pitches the Grid Spacing dropdown offers: the presets, plus the
/// library's current pitch when it is none of them (a preference-seeded
/// value stays visible and pickable).
fn grid_pitches(current_mm: f32) -> Vec<f32> {
    let mut pitches = GRID_SIZES_MM.to_vec();
    let is_preset = pitches
        .iter()
        .any(|p| (p - current_mm).abs() < f32::EPSILON);
    if !is_preset && current_mm.is_finite() && current_mm > 0.0 {
        pitches.push(current_mm);
        pitches.sort_by(f32::total_cmp);
    }
    pitches
}

/// A pitch as the dropdown and caption show it: millimetres to three
/// decimals ("1.270 mm").
fn pitch_number(i18n: &I18n, mm: f32) -> String {
    LengthFormat::new(i18n, Unit::Mm).field_text(pitch_mm_exact(mm))
}

/// Grid Spacing: the preset dropdown and a caption listing the presets.
fn grid_spacing<'a>(
    current_mm: f32,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let choices: Vec<Choice<f32>> = grid_pitches(current_mm)
        .into_iter()
        .map(|mm| Choice {
            value: mm,
            label: i18n.message_plain(
                ids::LIBRARY_DOCUMENT_OPTIONS_GRID_PITCH,
                &[("pitch", pitch_number(i18n, mm).into())],
            ),
        })
        .collect();
    let selected = choices
        .iter()
        .find(|c| (c.value - current_mm).abs() < f32::EPSILON)
        .cloned();
    let picker = pick_list(selected, choices, |c: &Choice<f32>| c.label.clone())
        .graphite()
        .on_select(|c: Choice<f32>| LibraryMessage::DocumentOptionsGridSpacingPicked(c.value))
        .padding([SP.xs, SP.s])
        .width(GRID_PICK_W);
    let presets = GRID_SIZES_MM
        .iter()
        .map(|&mm| pitch_number(i18n, mm))
        .collect::<Vec<_>>()
        .join(" \u{b7} ");
    let caption = text(t!(
        i18n,
        ids::LIBRARY_DOCUMENT_OPTIONS_GRID_PRESETS,
        "pitches" => presets
    ))
    .size(TS.caption)
    .color(resolve(tokens).ink4);
    // The mock's 6 px between the dropdown and its caption.
    row![picker, caption]
        .spacing(SP.s + SP.xs)
        .align_y(iced::Alignment::Center)
        .boxed()
}

/// The catalog name of a display unit.
fn unit_label(unit: Unit) -> MessageId {
    match unit {
        Unit::Mm => ids::LIBRARY_DOCUMENT_OPTIONS_UNIT_MM,
        Unit::Mil => ids::LIBRARY_DOCUMENT_OPTIONS_UNIT_MIL,
        Unit::Inch => ids::LIBRARY_DOCUMENT_OPTIONS_UNIT_INCH,
        Unit::Micrometer => ids::LIBRARY_DOCUMENT_OPTIONS_UNIT_UM,
    }
}

/// Unit: a segmented control over every display unit.
fn unit_segmented<'a>(
    unit: Unit,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let style = SegmentedStyle::new(
        r.sunken,
        r.raised,
        r.shadow,
        r.ink3,
        ti(tokens.text),
        TS.body,
    )
    .fill_width(true);
    let options = UNITS
        .iter()
        .map(|&u| (u, Label::from(t!(i18n, unit_label(u)))));
    container(segmented(
        options,
        unit,
        style,
        LibraryMessage::DocumentOptionsUnitPicked,
    ))
    .width(UNIT_SEG_W)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::ThemeId;

    /// The dropdown offers the presets; a pitch that is none of them is
    /// added in order, so the library's value is never hidden.
    #[test]
    fn the_spacing_dropdown_offers_the_presets_and_the_current_pitch() {
        assert_eq!(grid_pitches(1.27), GRID_SIZES_MM.to_vec());
        assert_eq!(grid_pitches(1.0), vec![0.635, 1.0, 1.27, 2.54, 5.08]);
        assert_eq!(grid_pitches(f32::NAN), GRID_SIZES_MM.to_vec());
    }

    /// Pitches read in millimetres to three decimals with a `.` mark.
    #[test]
    fn pitches_read_to_three_decimals() {
        let i18n = I18n::en_us();
        assert_eq!(pitch_number(&i18n, 1.27), "1.270");
        assert_eq!(pitch_number(&i18n, 0.635), "0.635");
        assert_eq!(
            i18n.message_plain(
                ids::LIBRARY_DOCUMENT_OPTIONS_GRID_PITCH,
                &[("pitch", pitch_number(&i18n, 2.54).into())],
            ),
            "2.540 mm"
        );
    }

    /// Every unit the model has is offered, named from the catalog.
    #[test]
    fn every_unit_is_offered() {
        let i18n = I18n::en_us();
        let names: Vec<String> = UNITS
            .iter()
            .map(|&u| i18n.message_plain(unit_label(u), &[]))
            .collect();
        assert_eq!(names, ["mm", "mil", "inch", "\u{b5}m"]);
    }

    /// The view builds in both Graphite themes, the grid shown and hidden,
    /// inline and windowed.
    #[test]
    fn the_view_builds_in_both_graphite_themes() {
        let i18n = I18n::en_us();
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = signex_types::theme::theme_tokens(id);
            for grid_visible in [true, false] {
                let mut state = DocumentOptionsModalState {
                    library_path: "l.snxlib".into(),
                    library_name: "passives".into(),
                    draft: Default::default(),
                };
                state.draft.grid_visible = grid_visible;
                for windowed in [false, true] {
                    let _ = view(&state, &tokens, &i18n, windowed);
                }
            }
        }
    }
}
