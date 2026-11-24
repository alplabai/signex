//! Graphite footer of the component editor (mockup row 128): a quiet
//! "Reload saved version" (the old header Discard, and the stale-row
//! reload) with its confirm, the summary of the fields that do not parse,
//! then the Ctrl+S hint, Release… (disabled), Save & Close, Cancel and the
//! primary Save, which stays disabled while a field does not parse and
//! names it in a tooltip. A read-only library shows its note and a disabled
//! Copy to Library… instead (Caner, 2026-11-20).

use iced::widget::{Space, Widget, container, row};
use iced::{Element, Length, Theme};
use signex_i18n::{I18n, ids, t};
use signex_widgets::{button, text, theme_ext, tooltip};

use super::kit::{self, SP, TS};
use crate::library::messages::{EditorMsg, LibraryMessage};
use crate::library::state::{ComponentPreviewState, EditorAddress};

/// What each footer action does now: `None` leaves the button disabled.
#[derive(Debug, Clone)]
pub(super) struct FooterActions {
    pub(super) reload: Option<LibraryMessage>,
    pub(super) save: Option<LibraryMessage>,
    pub(super) save_and_close: Option<LibraryMessage>,
    pub(super) cancel: Option<LibraryMessage>,
}

fn event(address: &EditorAddress, msg: EditorMsg) -> LibraryMessage {
    LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg,
    }
}

/// The footer's actions for an editable row. Save and Save & Close wait
/// until every field parses; Reload saved version needs something to drop
/// (unsaved edits, or a save refused because the stored row moved on).
pub(super) fn actions(state: &ComponentPreviewState, address: &EditorAddress) -> FooterActions {
    let parses = state.first_unparsed_field().is_none();
    let reloadable = state.has_unsaved_edits() || state.stale_base;
    FooterActions {
        reload: reloadable.then(|| event(address, EditorMsg::ReloadRequested)),
        save: parses.then(|| event(address, EditorMsg::SaveDraft)),
        save_and_close: parses.then(|| event(address, EditorMsg::Commit)),
        cancel: Some(event(address, EditorMsg::CloseEditor)),
    }
}

pub(super) fn view<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a signex_types::theme::ThemeTokens,
    i18n: &I18n,
    address: &EditorAddress,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let line = if read_only {
        read_only_line(tokens, i18n)
    } else {
        editable_line(state, tokens, i18n, address)
    };
    container(line)
        .padding([SP.m, SP.xl])
        .width(Length::Fill)
        .style(crate::styles::modal_footer_strip(tokens))
        .boxed()
}

/// "Read-only library - this component cannot be changed." and Copy to
/// Library…, disabled with a "not available yet" tooltip: no component
/// row copy exists (mockup row 134), and the symbol-only stock copy is another flow.
fn read_only_line<'a>(
    tokens: &'a signex_types::theme::ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let copy = tooltip(
        kit::secondary_action(
            t!(i18n, ids::EDITORS_COMPONENT_COPY_TO_LIBRARY),
            None,
            tokens,
        ),
        container(
            text(t!(i18n, ids::EDITORS_COMPONENT_COPY_TO_LIBRARY_UNAVAILABLE)).size(TS.caption),
        )
        .padding([SP.xs, SP.s])
        .style(crate::styles::context_menu(tokens)),
        iced::widget::tooltip::Position::Top,
    )
    .boxed();
    row![
        text(t!(i18n, ids::EDITORS_COMPONENT_READ_ONLY_FOOTER))
            .size(TS.body)
            .color(theme_ext::text_secondary(tokens))
            .boxed(),
        Space::new().width(Length::Fill).boxed(),
        copy,
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center)
    .boxed()
}

fn editable_line<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a signex_types::theme::ThemeTokens,
    i18n: &I18n,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let acts = actions(state, address);
    let muted = crate::styles::tokens::resolve(tokens).ink4;
    let mut line: iced::widget::Row<Element<'a, LibraryMessage>> =
        row![].spacing(SP.m).align_y(iced::Alignment::Center);
    if state.reload_confirming {
        line = line
            .push(
                text(t!(i18n, ids::EDITORS_COMPONENT_RELOAD_CONFIRM))
                    .size(TS.body)
                    .color(theme_ext::text_secondary(tokens))
                    .boxed(),
            )
            .push(kit::primary_action(
                t!(i18n, ids::EDITORS_COMPONENT_RELOAD),
                Some(event(address, EditorMsg::ReloadConfirmed)),
                tokens,
            ))
            .push(kit::secondary_action(
                t!(i18n, ids::EDITORS_COMPONENT_KEEP_EDITING),
                Some(event(address, EditorMsg::ReloadCancelled)),
                tokens,
            ));
    } else {
        line = line.push(reload_button(acts.reload.clone(), tokens, i18n));
    }
    // The inline error sits under the field; this is the one-line
    // summary.
    if let Some((field, why)) = state.first_unparsed_field() {
        // Clipped to the room left, so a long reason never pushes Save out.
        let summary = text(t!(
            i18n,
            ids::EDITORS_COMPONENT_FOOTER_UNPARSED,
            "count" => state.params_error.len(),
            "field" => field,
            "reason" => why
        ))
        .size(TS.caption)
        .color(theme_ext::error_color(tokens))
        .wrapping(iced::widget::text::Wrapping::None);
        line = line.push(container(summary).width(Length::Fill).clip(true).boxed());
    }
    line = line
        .push(Space::new().width(Length::Fill).boxed())
        .push(
            text(t!(i18n, ids::EDITORS_COMPONENT_SAVE_SHORTCUT))
                .size(TS.caption)
                .color(muted)
                .boxed(),
        )
        // Release has no backend yet: a disabled stub, as it was.
        .push(kit::secondary_action(
            t!(i18n, ids::EDITORS_COMPONENT_RELEASE),
            None,
            tokens,
        ))
        .push(kit::secondary_action(
            t!(i18n, ids::EDITORS_COMPONENT_SAVE_AND_CLOSE),
            acts.save_and_close,
            tokens,
        ))
        .push(kit::secondary_action(
            t!(i18n, ids::EDITORS_COMPONENT_CANCEL),
            acts.cancel,
            tokens,
        ));
    let save = kit::primary_action(t!(i18n, ids::EDITORS_COMPONENT_SAVE), acts.save, tokens);
    let save = match state.first_unparsed_field() {
        Some((field, _)) => tooltip(
            save,
            container(
                text(t!(i18n, ids::EDITORS_COMPONENT_SAVE_BLOCKED, "field" => field))
                    .size(TS.caption),
            )
            .padding([SP.xs, SP.s])
            .style(crate::styles::context_menu(tokens)),
            iced::widget::tooltip::Position::Top,
        )
        .boxed(),
        None => save,
    };
    line.push(save).boxed()
}

/// The quiet "Reload saved version" with its refresh icon.
fn reload_button<'a>(
    on_press: Option<LibraryMessage>,
    tokens: &signex_types::theme::ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let enabled = on_press.is_some();
    let r = crate::styles::tokens::resolve(tokens);
    let ink = if enabled {
        theme_ext::text_secondary(tokens)
    } else {
        r.ink4
    };
    let icon = iced::widget::svg(crate::icons::icon_chrome_refresh(
        signex_types::theme::ThemeId::GraphiteDark,
    ))
    .width(TS.body)
    .height(TS.body)
    .style(move |_: &Theme, _| iced::widget::svg::Style {
        color: Some(ink),
        ..Default::default()
    });
    let content = row![
        icon.boxed(),
        text(t!(i18n, ids::EDITORS_COMPONENT_RELOAD_SAVED))
            .size(TS.body)
            .boxed()
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    button(content)
        .padding([SP.xs, SP.m])
        .on_press_maybe(on_press)
        .style(crate::styles::ghost_button(tokens))
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> (ComponentPreviewState, EditorAddress) {
        let row = crate::library::component_preview::test_row();
        let address = EditorAddress::new(
            std::path::PathBuf::from("/tmp/test.snxlib"),
            "t".into(),
            signex_library::RowId::from(row.row_id),
        );
        let state = ComponentPreviewState::from_row(
            address.library_path.clone(),
            address.table.clone(),
            row,
        );
        (state, address)
    }

    fn is_editor_msg(msg: &Option<LibraryMessage>, want: fn(&EditorMsg) -> bool) -> bool {
        matches!(msg, Some(LibraryMessage::EditorEvent { msg, .. }) if want(msg))
    }

    /// Mockup row 128: Save and Save & Close are disabled while a field does not
    /// parse, and come back once it does.
    #[test]
    fn save_is_disabled_while_a_field_does_not_parse() {
        let (mut state, address) = editor();
        let acts = actions(&state, &address);
        assert!(is_editor_msg(&acts.save, |m| matches!(
            m,
            EditorMsg::SaveDraft
        )));
        assert!(is_editor_msg(&acts.save_and_close, |m| matches!(
            m,
            EditorMsg::Commit
        )));
        state
            .params_error
            .insert("value".into(), "not a number".into());
        let acts = actions(&state, &address);
        assert!(acts.save.is_none());
        assert!(acts.save_and_close.is_none());
        assert_eq!(
            state.first_unparsed_field(),
            Some(("value", "not a number"))
        );
        state.params_error.clear();
        assert!(actions(&state, &address).save.is_some());
    }

    /// The footer's Reload saved version is the old header Discard: it
    /// sends `ReloadRequested`, and is disabled while there is nothing to
    /// drop; a stale row (a refused save) enables it too.
    #[test]
    fn reload_saved_version_is_the_old_discard() {
        let (mut state, address) = editor();
        assert!(actions(&state, &address).reload.is_none());
        state.dirty = true;
        assert!(is_editor_msg(&actions(&state, &address).reload, |m| {
            matches!(m, EditorMsg::ReloadRequested)
        }));
        state.dirty = false;
        state.stale_base = true;
        assert!(actions(&state, &address).reload.is_some());
    }

    /// A read-only library's Copy to Library… is drawn but sends nothing:
    /// no component row copy exists yet, and the symbol-only flow is not it.
    #[test]
    fn copy_to_library_is_disabled_in_a_read_only_library() {
        let (state, address) = editor();
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let i18n = I18n::en_us();
        let label = t!(i18n, ids::EDITORS_COMPONENT_COPY_TO_LIBRARY).to_string();
        let clicked = crate::layout_probe::messages_on_click(
            view(&state, &tokens, &i18n, &address, true),
            iced::Size::new(900.0, 80.0),
            &label,
        );
        assert!(clicked.is_empty(), "{clicked:?}");
    }

    /// Mockup row 128 (decided 2026-11-22): at the narrowest docked width
    /// Cancel and Save stay inside the footer, also with a long parse error.
    #[test]
    fn cancel_and_save_stay_on_screen_at_the_narrowest_width() {
        let (mut state, address) = editor();
        state
            .params_error
            .insert("value".into(), "not a number ".repeat(20));
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let i18n = I18n::en_us();
        let width = super::super::MIN_EDITOR_W;
        let texts = crate::layout_probe::lay_out_texts(
            view(&state, &tokens, &i18n, &address, false),
            iced::Size::new(width, 80.0),
        );
        for id in [ids::EDITORS_COMPONENT_CANCEL, ids::EDITORS_COMPONENT_SAVE] {
            let label = t!(i18n, id).to_string();
            let found = crate::layout_probe::find(&texts, &label);
            assert!(
                found.bounds.x + found.bounds.width <= width,
                "{label:?} at {:?} in {width}",
                found.bounds
            );
        }
    }

    /// Cancel is the editor's close path, which asks about unsaved edits.
    #[test]
    fn cancel_closes_through_the_editor_close_path() {
        let (state, address) = editor();
        assert!(is_editor_msg(&actions(&state, &address).cancel, |m| {
            matches!(m, EditorMsg::CloseEditor)
        }));
    }
}
