//! The destination (file name, output folder led by a folder glyph, how
//! the files are named) and a run's outcome: the success banner with Open
//! folder and the notes, each with its severity, or why nothing was
//! written.

use iced::widget::{Column, Row, column, container, row};
use iced::{Alignment, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::{button, text, text_input};

use super::{SP, TS, fo, kit};
use crate::app::Message;
use crate::app::fab_outputs::{
    FabOutputsMsg, FabOutputsState, FabWritten, NoteSeverity, outcome_notes, problem_text,
    written_text,
};
use crate::styles::{PillTone, Radii, chrome_px, ti, tokens};

/// Width of the destination's label column (96 px).
const LABEL_W: f32 = chrome_px(88.0);

/// A compact outline button: Browse…, Open folder.
fn small_button<'a>(
    label: Localized,
    msg: Option<Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    button(text(label).size(TS.body))
        .padding([SP.s, SP.l])
        .on_press_maybe(msg)
        .style(crate::styles::outline_button(tokens))
        .boxed()
}

/// One label-and-control line of the destination grid.
fn form_line<'a>(
    label: Option<Localized>,
    control: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ink = tokens::resolve(tokens).ink3;
    let label: Element<'a, Message> = match label {
        Some(words) => text(words).size(TS.body).color(ink).boxed(),
        None => iced::widget::Space::new().boxed(),
    };
    row![container(label).width(Length::Fixed(LABEL_W)), control]
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed()
}

/// The file name, the output folder with Browse…, and the naming example.
pub(super) fn destination<'a>(
    state: &'a FabOutputsState,
    theme: ThemeId,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mono = super::super::graphite::mono();
    let name = text_input(t!(i18n, ids::FAB_OUTPUTS_NAME), &state.name)
        .style(crate::panels::form_look::text_input_style)
        .on_input(|v| fo(FabOutputsMsg::NameEdited(v)))
        .font(mono)
        .size(TS.body)
        .padding(SP.s + SP.xs)
        .width(Length::Fill);
    // Mockup row 115: a folder glyph leads the path, inside the field.
    let folder = kit::glyph_field(
        kit::FieldGlyph::Folder,
        text_input(t!(i18n, ids::FAB_OUTPUTS_FOLDER), &state.folder)
            .on_input(|v| fo(FabOutputsMsg::FolderEdited(v)))
            .font(mono)
            .size(TS.body),
        theme,
        tokens,
    );
    let browse = small_button(
        t!(i18n, ids::FAB_OUTPUTS_BROWSE),
        state
            .writing
            .is_none()
            .then(|| fo(FabOutputsMsg::BrowseRequested)),
        tokens,
    );
    let stem = state.name.trim();
    let example = kit::caption(
        t!(i18n, ids::FAB_OUTPUTS_NAME_EXAMPLE, "name" => stem),
        tokens,
    );
    column![
        kit::section_head(t!(i18n, ids::FAB_OUTPUTS_DESTINATION), None, i18n, tokens),
        form_line(Some(t!(i18n, ids::FAB_OUTPUTS_NAME)), name.boxed(), tokens),
        form_line(
            Some(t!(i18n, ids::FAB_OUTPUTS_FOLDER)),
            row![folder, browse]
                .spacing(SP.m)
                .align_y(Alignment::Center)
                .boxed(),
            tokens,
        ),
        form_line(None, example, tokens),
    ]
    .spacing(SP.s + SP.xs)
    .boxed()
}

/// The last run's outcome; `None` before a run.
pub(super) fn result<'a>(
    state: &'a FabOutputsState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Option<Element<'a, Message>> {
    match state.outcome.as_ref()? {
        Ok(written) => Some(written_view(written, i18n, tokens)),
        Err(problem) => Some(kit::banner(
            PillTone::Err,
            text(problem_text(problem, i18n)).size(TS.body).boxed(),
            None,
            tokens,
        )),
    }
}

/// What was written and where, with Open folder, then the notes.
fn written_view<'a>(
    written: &FabWritten,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let open = small_button(
        t!(i18n, ids::FAB_OUTPUTS_OPEN_FOLDER),
        Some(fo(FabOutputsMsg::FolderOpenRequested)),
        tokens,
    );
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.m).push(kit::banner(
        PillTone::Ok,
        text(written_text(written, i18n)).size(TS.body).boxed(),
        Some(open),
        tokens,
    ));
    let notes = outcome_notes(written, i18n);
    if !notes.is_empty() {
        col = col.push(notes_head(&notes, i18n, tokens));
        col = col.push(notes_list(notes, tokens));
    }
    col.boxed()
}

/// The tone a note is drawn in: an older file beside the set is an error,
/// since sent with it it can break a fab order.
pub(super) fn note_tone(severity: NoteSeverity) -> PillTone {
    match severity {
        NoteSeverity::Warning => PillTone::Warn,
        NoteSeverity::Error => PillTone::Err,
    }
}

/// "NOTES" and how many, the pill in the gravest note's tone.
fn notes_head<'a>(
    notes: &[(NoteSeverity, Localized)],
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let gravest = if notes.iter().any(|(s, _)| *s == NoteSeverity::Error) {
        NoteSeverity::Error
    } else {
        NoteSeverity::Warning
    };
    let count = t!(i18n, ids::FAB_OUTPUTS_NOTES_COUNT, "count" => notes.len());
    row![
        kit::section_label(t!(i18n, ids::FAB_OUTPUTS_NOTES), i18n, tokens),
        kit::status_pill(count, note_tone(gravest), tokens)
    ]
    .spacing(SP.s + SP.xs)
    .align_y(Alignment::Center)
    .boxed()
}

/// The notes as a bordered list, a glyph of the note's tone on each row.
fn notes_list<'a>(
    notes: Vec<(NoteSeverity, Localized)>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let count = notes.len();
    let mut list: Column<Element<'a, Message>> = Column::new().width(Length::Fill);
    for (index, (severity, words)) in notes.into_iter().enumerate() {
        let line: Row<Element<'a, Message>> = Row::new()
            .spacing(SP.m)
            .align_y(Alignment::Center)
            .push(kit::status_glyph(note_tone(severity), tokens))
            .push(text(words).size(TS.body).width(Length::Fill).boxed());
        // A one-line row is 34 px tall: the body line and this padding.
        list = list.push(
            container(line)
                .padding([SP.m, SP.l - SP.xs])
                .width(Length::Fill)
                .boxed(),
        );
        if index + 1 < count {
            list = list.push(kit::rule(r.line_soft));
        }
    }
    let border = ti(tokens.border);
    container(list)
        .width(Length::Fill)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: 1.0,
                color: border,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
