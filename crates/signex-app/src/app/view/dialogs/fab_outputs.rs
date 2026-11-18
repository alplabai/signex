//! The Fabrication Outputs dialog (#2388): which files, their name, the
//! output folder, Generate and its outcome. State and rules:
//! `app/fab_outputs/`.

use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{CheckboxLabel as _, checkbox, text, text_input};

use super::sizes;
use crate::app::fab_outputs::{
    FabOutput, FabOutputsMsg, FabOutputsState, board_note_text, note_text, stale_text,
};
use crate::app::{Message, Signex};
use crate::styles::{
    DialogButtonKind, Spacing, TypeScale, dialog_button, dialog_footer_split, ti, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn fo(msg: FabOutputsMsg) -> Message {
    Message::from(msg)
}

fn caption<'a>(i18n: &I18n, id: MessageId, tokens: &ThemeTokens) -> Element<'a, Message> {
    text(t!(i18n, id))
        .size(TS.body)
        .color(ti(tokens.text_secondary))
        .boxed()
}

fn kind_label(kind: FabOutput) -> MessageId {
    match kind {
        FabOutput::Gerber => ids::FAB_OUTPUTS_GERBER,
        FabOutput::Drill => ids::FAB_OUTPUTS_DRILL,
        FabOutput::Centroid => ids::FAB_OUTPUTS_CENTROID,
        FabOutput::TestNetlist => ids::FAB_OUTPUTS_TEST_NETLIST,
        FabOutput::Ipc2581 => ids::FAB_OUTPUTS_IPC2581,
        FabOutput::Odb => ids::FAB_OUTPUTS_ODB,
        FabOutput::AssemblyPdf => ids::FAB_OUTPUTS_ASSEMBLY_PDF,
        FabOutput::AssemblySvg => ids::FAB_OUTPUTS_ASSEMBLY_SVG,
        FabOutput::Step => ids::FAB_OUTPUTS_STEP,
    }
}

/// One tick box of the dialog.
fn tick<'a>(on: bool, label: signex_i18n::Localized, msg: FabOutputsMsg) -> Element<'a, Message> {
    checkbox(on)
        .style(crate::panels::form_look::checkbox_style)
        .size(TS.title)
        .labelled(label)
        .text_size(TS.label)
        .on_toggle(move |_| fo(msg.clone()))
        .boxed()
}

impl Signex {
    pub(in crate::app::view) fn view_fab_outputs_dialog(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(self.view_fab_outputs_card(), sizes::FAB_OUTPUTS)
    }

    fn view_fab_outputs_card(&self) -> Element<'_, Message> {
        let Some(state) = self.ui_state.fab_outputs.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let cancel = Some(fo(FabOutputsMsg::CancelRequested));
        let header = super::header::graphite_header_closing(
            t!(i18n, ids::FAB_OUTPUTS_TITLE).into_string(),
            Some(
                t!(i18n, ids::FAB_OUTPUTS_SUBTITLE, "board" => state.board_name.as_str())
                    .into_string(),
            ),
            None,
            cancel.clone(),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let body = scrollable(container(body(state, i18n, tokens)).padding([SP.l, SP.xl]))
            .height(Length::Fill);
        // After a Generate the left button closes; before, it cancels.
        let dismiss = if state.outcome.is_some() {
            ids::FAB_OUTPUTS_CLOSE
        } else {
            ids::FAB_OUTPUTS_CANCEL
        };
        let footer = dialog_footer_split(
            Vec::new(),
            vec![
                dialog_button(t!(i18n, dismiss), DialogButtonKind::Cancel, cancel, tokens),
                dialog_button(
                    t!(i18n, ids::FAB_OUTPUTS_GENERATE),
                    super::graphite::call_to_action(tokens),
                    state
                        .can_generate()
                        .then(|| fo(FabOutputsMsg::GenerateRequested)),
                    tokens,
                ),
            ],
        );
        container(column![header, body, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, sizes::FAB_OUTPUTS.0),
        )))
        .height(Length::Fixed(sizes::FAB_OUTPUTS.1))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}

/// The intro, the kinds of file, the name and folder, and the outcome.
fn body<'a>(state: &'a FabOutputsState, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let muted = tokens::muted_text(tokens);
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.l);
    col = col.push(
        text(t!(i18n, ids::FAB_OUTPUTS_INTRO))
            .size(TS.body)
            .color(muted)
            .boxed(),
    );
    let mut kinds: Column<Element<'a, Message>> =
        Column::new()
            .spacing(SP.s)
            .push(caption(i18n, ids::FAB_OUTPUTS_FILES, tokens));
    for kind in FabOutput::ALL {
        kinds = kinds.push(tick(
            kind.is_on(&state.kinds),
            t!(i18n, kind_label(kind)),
            FabOutputsMsg::OutputToggled(kind),
        ));
        // The ODB++ form, under its row while it is ticked.
        if kind == FabOutput::Odb && state.kinds.odb {
            let folder = tick(
                state.kinds.odb_folder,
                t!(i18n, ids::FAB_OUTPUTS_ODB_FOLDER),
                FabOutputsMsg::OdbFolderToggled,
            );
            kinds = kinds.push(
                container(folder)
                    .padding(iced::Padding {
                        left: SP.xl,
                        ..iced::Padding::ZERO
                    })
                    .boxed(),
            );
        }
    }
    // #2410: a board with a panel writes the whole panel unless unticked.
    if let Some((columns, rows)) = state.panel {
        kinds = kinds.push(
            checkbox(state.whole_panel)
                .style(crate::panels::form_look::checkbox_style)
                .size(TS.title)
                .labelled(t!(
                    i18n,
                    ids::FAB_OUTPUTS_PANEL,
                    "columns" => columns,
                    "rows" => rows
                ))
                .text_size(TS.label)
                .on_toggle(|_| fo(FabOutputsMsg::WholePanelToggled))
                .boxed(),
        );
    }
    col = col.push(kinds.boxed());
    col = col.push(fields(state, i18n, tokens));
    col = col.push(outcome(state, i18n, tokens));
    col.boxed()
}

/// The file name and the output folder with Browse.
fn fields<'a>(
    state: &'a FabOutputsState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let name = text_input(t!(i18n, ids::FAB_OUTPUTS_NAME), &state.name)
        .style(crate::panels::form_look::text_input_style)
        .on_input(|v| fo(FabOutputsMsg::NameEdited(v)))
        .size(TS.label)
        .padding(SP.m)
        .width(Length::Fill);
    let folder = text_input(t!(i18n, ids::FAB_OUTPUTS_FOLDER), &state.folder)
        .style(crate::panels::form_look::text_input_style)
        .on_input(|v| fo(FabOutputsMsg::FolderEdited(v)))
        .size(TS.label)
        .padding(SP.m)
        .width(Length::Fill);
    let browse = signex_widgets::button(text(t!(i18n, ids::FAB_OUTPUTS_BROWSE)).size(TS.label))
        .padding([SP.m, SP.l])
        .on_press_maybe(
            state
                .writing
                .is_none()
                .then(|| fo(FabOutputsMsg::BrowseRequested)),
        )
        .style(crate::styles::outline_button(tokens));
    column![
        column![caption(i18n, ids::FAB_OUTPUTS_NAME, tokens), name].spacing(SP.m),
        column![
            caption(i18n, ids::FAB_OUTPUTS_FOLDER, tokens),
            row![folder, browse].spacing(SP.m)
        ]
        .spacing(SP.m),
    ]
    .spacing(SP.l)
    .boxed()
}

/// While writing, a line that says so; after, what was written, what the
/// files leave out and the older files beside them, or why nothing was
/// written.
fn outcome<'a>(
    state: &'a FabOutputsState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let muted = tokens::muted_text(tokens);
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.s);
    if state.writing.is_some() {
        col = col.push(
            text(t!(i18n, ids::FAB_OUTPUTS_WRITING))
                .size(TS.body)
                .color(muted)
                .boxed(),
        );
    }
    match &state.outcome {
        Some(Ok(written)) => {
            col = col.push(
                text(crate::app::fab_outputs::written_text(written, i18n))
                    .size(TS.body)
                    .color(ti(tokens.text))
                    .boxed(),
            );
        }
        Some(Err(problem)) => {
            col = col.push(
                text(crate::app::fab_outputs::problem_text(problem, i18n))
                    .size(TS.body)
                    .color(tokens::danger(tokens))
                    .boxed(),
            );
        }
        None => {}
    }
    // What the set leaves out, and older files beside it, in the warning
    // colour: the set is not complete as it stands.
    let warning = ti(tokens.warning);
    for note in state.notes() {
        col = col.push(
            text(note_text(*note, i18n))
                .size(TS.body)
                .color(warning)
                .boxed(),
        );
    }
    for note in state.board_notes() {
        col = col.push(
            text(board_note_text(note, i18n))
                .size(TS.body)
                .color(warning)
                .boxed(),
        );
    }
    if let Some(Ok(written)) = &state.outcome
        && let Some(line) = stale_text(written, i18n)
    {
        col = col.push(text(line).size(TS.body).color(warning).boxed());
    }
    col.boxed()
}
