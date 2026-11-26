//! The Generate design notebook options (#2945, mockup row 201): variant,
//! template, document number and revision, the ten sections in two columns,
//! whether the checks run now or the last results are used, the note on
//! where the report goes, and Cancel / Generate with the command line that
//! does the same. State and rules: `app/notebook_generate/`.

use iced::widget::{Column, Row, Space, column, container, pick_list, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_notebook::generate::{ChecksSource, Section, Template, generated_path};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{CheckboxLabel as _, checkbox, radio, text, text_input};

use crate::app::notebook_generate::{GENERATE_COMMAND_LINE, GenerateDialog, NotebookGenerateMsg};
use crate::app::{Message, Signex};
use crate::styles::{
    DialogButtonKind, DialogIcon, Spacing, TypeScale, dialog_button, dialog_footer_split, ti,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The dialog, in body-text units.
const SIZE: (f32, f32) = (TS.body * 50.0, TS.body * 54.0);
/// The label column of the option rows.
const LABEL_W: f32 = TS.body * 7.0;
/// The document number field, and the revision field beside it.
const DOCUMENT_W: f32 = TS.body * 10.0;
const REVISION_W: f32 = TS.body * 4.5;

/// The sections as the dialog lays them out: two columns.
pub(crate) const SECTION_COLUMNS: [[Section; 5]; 2] = [
    [
        Section::Summary,
        Section::Requirements,
        Section::Architecture,
        Section::Decisions,
        Section::OpenItems,
    ],
    [
        Section::Bom,
        Section::Checks,
        Section::Simulation,
        Section::Outputs,
        Section::Changelog,
    ],
];

fn msg(event: NotebookGenerateMsg) -> Message {
    Message::from(event)
}

/// The label of `section` in the dialog.
pub(crate) fn section_label(section: Section) -> MessageId {
    match section {
        Section::Summary => ids::DIALOG_NB_GENERATE_SECTION_SUMMARY,
        Section::Requirements => ids::DIALOG_NB_GENERATE_SECTION_REQUIREMENTS,
        Section::Architecture => ids::DIALOG_NB_GENERATE_SECTION_ARCHITECTURE,
        Section::Decisions => ids::DIALOG_NB_GENERATE_SECTION_DECISIONS,
        Section::Bom => ids::DIALOG_NB_GENERATE_SECTION_BOM,
        Section::Checks => ids::DIALOG_NB_GENERATE_SECTION_CHECKS,
        Section::Simulation => ids::DIALOG_NB_GENERATE_SECTION_SIMULATION,
        Section::Outputs => ids::DIALOG_NB_GENERATE_SECTION_OUTPUTS,
        Section::OpenItems => ids::DIALOG_NB_GENERATE_SECTION_OPEN_ITEMS,
        Section::Changelog => ids::DIALOG_NB_GENERATE_SECTION_CHANGELOG,
    }
}

/// The line under a section's label, when it has one.
pub(crate) fn section_hint(
    section: Section,
    dialog: &GenerateDialog,
    i18n: &I18n,
) -> Option<Localized> {
    let id = match section {
        Section::Summary | Section::Checks => return None,
        Section::Requirements => {
            let count = dialog.requirements as u32;
            return Some(
                t!(i18n, ids::DIALOG_NB_GENERATE_SECTION_REQUIREMENTS_HINT, "count" => count),
            );
        }
        Section::Architecture => ids::DIALOG_NB_GENERATE_SECTION_ARCHITECTURE_HINT,
        Section::Decisions => ids::DIALOG_NB_GENERATE_SECTION_DECISIONS_HINT,
        Section::Bom => ids::DIALOG_NB_GENERATE_SECTION_BOM_HINT,
        Section::Simulation => ids::DIALOG_NB_GENERATE_SECTION_SIMULATION_HINT,
        Section::Outputs => ids::DIALOG_NB_GENERATE_SECTION_OUTPUTS_HINT,
        Section::OpenItems => ids::DIALOG_NB_GENERATE_SECTION_OPEN_ITEMS_HINT,
        Section::Changelog => ids::DIALOG_NB_GENERATE_SECTION_CHANGELOG_HINT,
    };
    Some(t!(i18n, id))
}

/// One template of the Template dropdown.
#[derive(Debug, Clone, PartialEq)]
struct TemplateChoice {
    template: Template,
    label: String,
}

impl Signex {
    pub(in crate::app::view) fn view_notebook_generate_dialog<'a>(
        &'a self,
        dialog: &'a GenerateDialog,
    ) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let cancel = Some(msg(NotebookGenerateMsg::DialogCancelled));
        let subtitle = match dialog.revision.as_deref() {
            Some(revision) => t!(
                i18n,
                ids::DIALOG_NB_GENERATE_SUBTITLE,
                "project" => dialog.project_file.as_str(),
                "revision" => revision
            ),
            None => t!(
                i18n,
                ids::DIALOG_NB_GENERATE_SUBTITLE_NO_REVISION,
                "project" => dialog.project_file.as_str()
            ),
        };
        let header = super::header::graphite_header_closing(
            t!(i18n, ids::DIALOG_NB_GENERATE_TITLE).into_string(),
            Some(subtitle.into_string()),
            None,
            cancel.clone(),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let path = generated_path();
        let body = column![
            options_rows(dialog, i18n, tokens),
            sections(dialog, i18n, tokens),
            checks(dialog, i18n),
            super::graphite::callout(
                DialogIcon::Info,
                t!(i18n, ids::DIALOG_NB_GENERATE_NOTE, "path" => path.as_str()),
                tokens,
            ),
        ]
        .spacing(SP.l)
        .padding([SP.l, SP.xl])
        .height(Length::Fill);
        let footer = dialog_footer_split(
            footer_extras(dialog, i18n, tokens),
            vec![
                dialog_button(
                    t!(i18n, ids::DIALOG_NB_GENERATE_CANCEL),
                    DialogButtonKind::Cancel,
                    cancel,
                    tokens,
                ),
                dialog_button(
                    t!(i18n, run_label(dialog)),
                    super::graphite::call_to_action(tokens),
                    dialog
                        .can_generate()
                        .then(|| msg(NotebookGenerateMsg::GenerateClicked)),
                    tokens,
                ),
            ],
        );
        let width = super::widgets::modal_card_width(self.ui_state.window_size, SIZE.0);
        let card = container(column![header, body, footer].width(Length::Fixed(width)))
            .height(Length::Fixed(SIZE.1))
            .style(crate::styles::modal_card(tokens))
            .clip(true)
            .boxed();
        self.wrap_centred_dialog(card, SIZE)
    }
}

/// Generate, or Generating… while a run is under way.
pub(crate) fn run_label(dialog: &GenerateDialog) -> MessageId {
    if dialog.running {
        ids::DIALOG_NB_GENERATE_RUNNING
    } else {
        ids::DIALOG_NB_GENERATE_RUN
    }
}

/// A label, then its control.
fn option_row<'a>(
    label: Localized,
    control: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    row![
        text(label)
            .size(TS.label)
            .color(ti(tokens.text_secondary))
            .width(Length::Fixed(LABEL_W))
            .boxed(),
        control,
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center)
    .boxed()
}

/// Variant, Template, and Document with its Revision.
fn options_rows<'a>(
    dialog: &'a GenerateDialog,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let variant = pick_list(
        Some(dialog.variant.clone()),
        dialog.variants.clone(),
        |name: &String| name.clone(),
    )
    .graphite()
    .on_select(|name: String| msg(NotebookGenerateMsg::VariantPicked(name)))
    .width(Length::Fill)
    .boxed();
    let templates: Vec<TemplateChoice> = Template::ALL
        .into_iter()
        .map(|template| TemplateChoice {
            template,
            label: t!(i18n, ids::DIALOG_NB_GENERATE_TEMPLATE_A4).into_string(),
        })
        .collect();
    let picked = templates
        .iter()
        .find(|choice| choice.template == dialog.options.template)
        .cloned();
    let template = pick_list(picked, templates, |choice: &TemplateChoice| {
        choice.label.clone()
    })
    .graphite()
    .on_select(|choice: TemplateChoice| msg(NotebookGenerateMsg::TemplatePicked(choice.template)))
    .width(Length::Fill)
    .boxed();
    let field = |value: &str, width: f32, on_input: fn(String) -> NotebookGenerateMsg| {
        text_input(UserText::new(String::new()), value.to_string())
            .on_input(move |typed| msg(on_input(typed)))
            .size(TS.body)
            .padding([SP.xs, SP.s])
            .width(Length::Fixed(width))
            .boxed()
    };
    let document = row![
        field(
            &dialog.document_number,
            DOCUMENT_W,
            NotebookGenerateMsg::DocumentNumberEdited
        ),
        text(t!(i18n, ids::DIALOG_NB_GENERATE_REVISION))
            .size(TS.caption)
            .color(ti(tokens.text_secondary))
            .boxed(),
        field(
            &dialog.document_revision,
            REVISION_W,
            NotebookGenerateMsg::RevisionEdited
        ),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed();
    column![
        option_row(t!(i18n, ids::DIALOG_NB_GENERATE_VARIANT), variant, tokens),
        option_row(t!(i18n, ids::DIALOG_NB_GENERATE_TEMPLATE), template, tokens),
        option_row(t!(i18n, ids::DIALOG_NB_GENERATE_DOCUMENT), document, tokens),
    ]
    .spacing(SP.s)
    .boxed()
}

/// The Sections group: its count, then the ten checkboxes in two columns.
fn sections<'a>(
    dialog: &'a GenerateDialog,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let count = t!(
        i18n,
        ids::DIALOG_NB_GENERATE_SECTIONS_COUNT,
        "picked" => dialog.options.sections.len() as u32,
        "total" => Section::ALL.len() as u32
    );
    let heading = row![
        text(t!(i18n, ids::DIALOG_NB_GENERATE_SECTIONS))
            .size(TS.label)
            .boxed(),
        Space::new().width(Length::Fill),
        text(count)
            .size(TS.caption)
            .color(ti(tokens.text_secondary))
            .boxed(),
    ]
    .align_y(Alignment::Center);
    let mut columns: Row<Element<'a, Message>> = Row::new().spacing(SP.xl);
    for sections in SECTION_COLUMNS {
        let mut list: Column<Element<'a, Message>> =
            Column::new().spacing(SP.s).width(Length::Fill);
        for section in sections {
            list = list.push(section_box(section, dialog, i18n, tokens));
        }
        columns = columns.push(list.boxed());
    }
    column![heading.boxed(), columns.boxed()]
        .spacing(SP.s)
        .boxed()
}

fn section_box<'a>(
    section: Section,
    dialog: &GenerateDialog,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let toggle = checkbox(dialog.options.has(section))
        .style(crate::panels::form_look::checkbox_style)
        .size(TS.title)
        .labelled(t!(i18n, section_label(section)))
        .text_size(TS.label)
        .on_toggle(move |_| msg(NotebookGenerateMsg::SectionToggled(section)))
        .boxed();
    let Some(hint) = section_hint(section, dialog, i18n) else {
        return toggle;
    };
    column![
        toggle,
        text(hint)
            .size(TS.caption)
            .color(ti(tokens.text_secondary))
            .boxed(),
    ]
    .spacing(SP.xs)
    .boxed()
}

/// Run the checks now, or use the last results.
fn checks<'a>(dialog: &GenerateDialog, i18n: &I18n) -> Element<'a, Message> {
    let option = |label: Localized, value: ChecksSource| -> Element<'a, Message> {
        radio(label, value, Some(dialog.options.checks), |picked| {
            msg(NotebookGenerateMsg::ChecksSourcePicked(picked))
        })
        .size(TS.heading)
        .spacing(SP.s)
        .text_size(TS.body)
        .boxed()
    };
    column![
        text(t!(i18n, ids::DIALOG_NB_GENERATE_CHECKS))
            .size(TS.label)
            .boxed(),
        option(
            t!(i18n, ids::DIALOG_NB_GENERATE_CHECKS_RUN),
            ChecksSource::RunNow
        ),
        option(
            t!(i18n, ids::DIALOG_NB_GENERATE_CHECKS_LAST),
            ChecksSource::LastResults
        ),
    ]
    .spacing(SP.xs)
    .boxed()
}

/// "Same as signex-cli notebook generate", and why the last run made nothing.
fn footer_extras<'a>(
    dialog: &'a GenerateDialog,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Vec<Element<'a, Message>> {
    let same_as = t!(
        i18n,
        ids::DIALOG_NB_GENERATE_SAME_AS,
        "command" => GENERATE_COMMAND_LINE
    );
    let mut extras = vec![
        text(same_as)
            .size(TS.caption)
            .font(super::graphite::mono())
            .color(ti(tokens.text_secondary))
            .boxed(),
    ];
    if let Some(problem) = dialog.problem.as_deref() {
        extras.push(
            text(t!(i18n, ids::DIALOG_NB_GENERATE_FAILED, "why" => problem))
                .size(TS.caption)
                .color(ti(tokens.error))
                .boxed(),
        );
    }
    extras
}

#[cfg(test)]
#[path = "notebook_generate_tests.rs"]
mod tests;
