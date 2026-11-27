//! The Fabrication Outputs dialog (#2388; laid out as mockup row 115): the kinds of
//! file in four groups over two columns, the destination, Generate, and the
//! outcome with its notes. State and rules: `app/fab_outputs/`.

mod kinds;
mod outcome;

#[cfg(test)]
mod tests;

use iced::widget::{Column, Space, column, container, scrollable};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::text;

use super::board_output::kit;
use super::sizes;
use crate::app::fab_outputs::{FabOutputKinds, FabOutputsMsg, FabOutputsState, ticked_count};
use crate::app::{Message, Signex};
use crate::styles::{
    DialogButtonKind, PillTone, Spacing, TypeScale, dialog_button, dialog_footer_split, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn fo(msg: FabOutputsMsg) -> Message {
    Message::from(msg)
}

impl Signex {
    pub(in crate::app::view) fn view_fab_outputs_dialog(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(self.view_fab_outputs_card(), sizes::FAB_OUTPUTS)
    }

    fn view_fab_outputs_card(&self) -> Element<'_, Message> {
        let Some(state) = self.ui_state.fab_outputs.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_view.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let header = super::header::graphite_header_closing(
            t!(i18n, ids::FAB_OUTPUTS_TITLE).into_string(),
            Some(
                t!(i18n, ids::FAB_OUTPUTS_SUBTITLE, "board" => state.board_name.as_str())
                    .into_string(),
            ),
            None,
            Some(fo(FabOutputsMsg::CancelRequested)),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let ticked = self.fab_output_kinds(state);
        let theme = self.ui_state.effective_theme_id();
        let body = scrollable(
            container(body(state, &ticked, theme, i18n, tokens)).padding([SP.l, SP.xl + SP.xs]),
        )
        .direction(crate::styles::thin_vertical())
        .height(Length::Fill);
        let footer = footer(state, &ticked, i18n, tokens);
        container(column![header, body, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, sizes::FAB_OUTPUTS.0),
        )))
        .height(Length::Fixed(sizes::FAB_OUTPUTS.1))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}

/// The intro, the panel banner, the kinds of file (`ticked` on), the
/// destination and the outcome.
fn body<'a>(
    state: &'a FabOutputsState,
    ticked: &FabOutputKinds,
    theme: ThemeId,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.l);
    col = col.push(
        text(t!(i18n, ids::FAB_OUTPUTS_INTRO))
            .size(TS.caption)
            .color(tokens::resolve(tokens).ink3)
            .boxed(),
    );
    // #2410: every file is the panel's while this box is ticked, so it
    // heads the dialog.
    if let Some((columns, rows)) = state.panel {
        let label = t!(
            i18n,
            ids::FAB_OUTPUTS_PANEL,
            "columns" => columns,
            "rows" => rows
        );
        let whole = kit::tick(
            state.whole_panel,
            label,
            Some(fo(FabOutputsMsg::WholePanelToggled)),
        );
        col = col.push(kit::banner(PillTone::Info, whole, None, tokens));
    }
    col = col.push(kinds::grid(ticked, i18n, tokens));
    col = col.push(outcome::destination(state, theme, i18n, tokens));
    if let Some(result) = outcome::result(state, i18n, tokens) {
        col = col.push(result);
    }
    col.boxed()
}

/// The words left of the footer buttons: what Generate would write, that
/// it is writing, or when it last wrote.
fn footer_caption(
    state: &FabOutputsState,
    ticked: &FabOutputKinds,
    i18n: &I18n,
) -> Option<signex_i18n::Localized> {
    if state.writing.is_some() {
        return Some(t!(i18n, ids::FAB_OUTPUTS_WRITING));
    }
    match &state.outcome {
        None => Some(t!(i18n, ids::FAB_OUTPUTS_SELECTED, "count" => ticked_count(ticked))),
        Some(Ok(_)) => state.generated_at.map(|at| {
            let time = at.format("%H:%M").to_string();
            t!(i18n, ids::FAB_OUTPUTS_GENERATED, "time" => time.as_str())
        }),
        Some(Err(_)) => None,
    }
}

/// Before a run: Cancel and Generate. After one (the footer rule of the
/// output dialogs): Generate again and Close.
fn footer<'a>(
    state: &FabOutputsState,
    ticked: &FabOutputKinds,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let cancel = Some(fo(FabOutputsMsg::CancelRequested));
    let generate = state
        .can_generate(ticked)
        .then(|| fo(FabOutputsMsg::GenerateRequested));
    let call = super::graphite::call_to_action(tokens);
    let buttons = if state.has_run() {
        vec![
            dialog_button(
                t!(i18n, ids::FAB_OUTPUTS_GENERATE_AGAIN),
                DialogButtonKind::Secondary,
                generate,
                tokens,
            ),
            dialog_button(t!(i18n, ids::FAB_OUTPUTS_CLOSE), call, cancel, tokens),
        ]
    } else {
        vec![
            dialog_button(
                t!(i18n, ids::FAB_OUTPUTS_CANCEL),
                DialogButtonKind::Cancel,
                cancel,
                tokens,
            ),
            dialog_button(t!(i18n, ids::FAB_OUTPUTS_GENERATE), call, generate, tokens),
        ]
    };
    let left = footer_caption(state, ticked, i18n)
        .map(|words| {
            text(words)
                .size(TS.body)
                .color(tokens::resolve(tokens).ink3)
                .boxed()
        })
        .into_iter()
        .collect();
    dialog_footer_split(left, buttons)
}
