//! The ERC and Messages panels' view events (#85, #86), and the user
//! identity a waiver's Author starts as (AV-13). The identity means reading
//! git config, so it is read once, off the update loop, the first time a
//! waiver form opens; every later form reuses the cached answer.

use std::path::PathBuf;

use iced::Task;
use signex_types::schematic::SelectedItem;

use crate::app::handlers::canvas::{AfterOpen, SheetOpen};
use crate::app::{Message, Signex};
use crate::dock::DockMessage;
use crate::panels::{ErcPanelMsg, MessagesPanelMsg, PanelMsg, UserIdentity, ValidationPanels};

impl Signex {
    /// The panels' view state as the panel context shows it, with the Run
    /// ERC key of the active keymap.
    pub(crate) fn validation_panels_view(&self) -> ValidationPanels {
        let mut view = (*self.ui_state.validation_panels).clone();
        view.run_erc_key = self.ui_state.active_keymap.shortcut_label_for_id("run_erc");
        view
    }

    /// Copy only the panels' view state into the panel context: a tile,
    /// group or hover change touches nothing else the context holds.
    fn sync_validation_panels(&mut self) {
        self.document_state.panel_ctx.validation = self.validation_panels_view();
        self.refresh_window_panel_ctxs(None);
    }

    /// A tile, group heading or row hover in the ERC panel.
    pub(crate) fn handle_erc_panel_view(&mut self, msg: ErcPanelMsg) {
        let has_findings = self
            .ui_state
            .erc_violations_by_path
            .values()
            .any(|list| !list.is_empty());
        self.ui_state.validation_panels.erc.apply(msg, has_findings);
        self.sync_validation_panels();
    }

    /// A chip, dropdown or filter edit in the Messages panel; Copy writes
    /// the rows the panel shows to the clipboard; a row's Show opens the
    /// sheet it names and selects its object (mockup row 86).
    pub(crate) fn handle_messages_panel(&mut self, msg: MessagesPanelMsg) -> Task<Message> {
        if let MessagesPanelMsg::ShowPressed { document, object } = msg {
            return self.show_message_target(document, object);
        }
        let ctx = &self.document_state.panel_ctx;
        let task = match msg {
            MessagesPanelMsg::Copied => {
                let verbose = crate::panels::is_verbose(&ctx.diagnostics_level);
                let rows = crate::panels::messages_shown(
                    &ctx.diagnostics,
                    &self.ui_state.validation_panels.messages,
                    verbose,
                    &self.ui_state.i18n,
                );
                let text = crate::panels::messages_copy_text(&rows, &self.ui_state.i18n);
                iced::clipboard::write(text).discard()
            }
            _ => Task::none(),
        };
        let newest = ctx.diagnostics.last().map(|entry| entry.id);
        self.ui_state.validation_panels.messages.apply(msg, newest);
        self.sync_validation_panels();
        task
    }

    /// Open `document` and select `object` where it is now. A sheet nobody
    /// holds is read on a worker; the object is selected when it arrives. A
    /// window cannot switch sheets inside its swap, so from a panel window
    /// the open is queued to the main window.
    fn show_message_target(
        &mut self,
        document: PathBuf,
        object: Option<SelectedItem>,
    ) -> Task<Message> {
        let open = SheetOpen::plain().then(AfterOpen::Reveal { item: object });
        if self.interaction_state.acting_window.is_some() {
            return Task::done(Message::Project(
                crate::app::ProjectMsg::SheetOpenRequested {
                    path: document,
                    open: Box::new(open),
                },
            ));
        }
        self.open_sheet(document, open)
    }

    /// Ask who the user is, once: the answer arrives as
    /// [`PanelMsg::WaiverIdentityResolved`] and is cached for every form.
    pub(crate) fn ask_user_identity(&mut self) -> Task<Message> {
        if !self.ui_state.validation_panels.identity.needs_asking() {
            return Task::none();
        }
        self.ui_state.validation_panels.identity = UserIdentity::Asking;
        Task::future(async {
            let name = tokio::task::spawn_blocking(user_identity_name)
                .await
                .ok()
                .flatten();
            Message::Dock(DockMessage::Panel(PanelMsg::WaiverIdentityResolved(name)))
        })
    }

    /// Cache the identity, give it to a form opened while asking, and let
    /// that form's Waive go ahead.
    pub(crate) fn handle_waiver_identity_resolved(&mut self, name: Option<String>) {
        let identity = UserIdentity::resolved(name);
        let author = identity.author();
        self.ui_state.validation_panels.identity = identity;
        if let Some(draft) = self.ui_state.erc_waive_draft.as_mut() {
            draft.awaits_identity = false;
            if draft.author.trim().is_empty() {
                draft.author.clone_from(&author);
            }
        }
        if let Some(draft) = self.interaction_state.pcb_drc.waive_draft.as_mut() {
            draft.awaits_identity = false;
            if draft.author.trim().is_empty() {
                draft.author = author;
            }
        }
        self.refresh_panel_ctx();
    }
}

/// The git author name (environment, then config); `None` when git has no
/// identity, so the waiver's author is stored empty (mockup row 88). Reads
/// git config and the environment: run it off the update loop.
fn user_identity_name() -> Option<String> {
    signex_library::git_identity::identity_for_default_config()
        .ok()
        .map(|(name, _email)| name)
        .filter(|name| !name.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel(app: &mut Signex, msg: PanelMsg) -> Task<Message> {
        app.update(Message::Dock(DockMessage::Panel(msg)))
    }

    /// AV-13: the identity is asked once and cached; the form's Author
    /// starts as it, and no form reads git or the environment itself.
    #[test]
    fn the_identity_is_asked_once_and_cached() {
        let (mut app, _boot) = Signex::new();
        assert_eq!(
            app.ui_state.validation_panels.identity,
            UserIdentity::NotAsked
        );
        let _first = app.ask_user_identity();
        assert_eq!(
            app.ui_state.validation_panels.identity,
            UserIdentity::Asking
        );
        let _ = panel(
            &mut app,
            PanelMsg::WaiverIdentityResolved(Some("C. Alp".into())),
        );
        assert_eq!(
            app.ui_state.validation_panels.identity,
            UserIdentity::Known("C. Alp".into())
        );
        let _again = app.ask_user_identity();
        assert_eq!(
            app.ui_state.validation_panels.identity,
            UserIdentity::Known("C. Alp".into()),
            "a known identity is not asked again"
        );
        assert_eq!(
            app.document_state.panel_ctx.validation.identity,
            UserIdentity::Known("C. Alp".into())
        );
    }

    #[test]
    fn a_messages_chip_and_clear_reach_the_panel_context() {
        let (mut app, _boot) = Signex::new();
        let _ = panel(
            &mut app,
            PanelMsg::MessagesView(MessagesPanelMsg::LevelPicked(
                crate::panels::MessageLevel::Error,
            )),
        );
        assert_eq!(
            app.document_state.panel_ctx.validation.messages.level,
            crate::panels::MessageLevel::Error
        );
        let newest = app
            .document_state
            .panel_ctx
            .diagnostics
            .last()
            .map(|e| e.id);
        let _ = panel(&mut app, PanelMsg::MessagesView(MessagesPanelMsg::Cleared));
        assert_eq!(
            app.document_state
                .panel_ctx
                .validation
                .messages
                .cleared_through,
            newest
        );
    }

    /// Mockup row 86: a row's Show goes to the sheet it names and selects
    /// the object there, whatever its kind.
    #[test]
    fn show_selects_the_object_on_the_named_sheet() {
        use signex_types::schematic::{NoConnect, Point, SchematicSheet, SelectedKind};
        let mark = NoConnect {
            uuid: uuid::Uuid::now_v7(),
            position: Point::new(25.4, 12.7),
        };
        let item = SelectedItem::new(mark.uuid, SelectedKind::NoConnect);
        let sheet = SchematicSheet {
            no_connects: vec![mark],
            ..Default::default()
        };
        let path = PathBuf::from("/w/p/power.snxsch");
        let (mut app, _boot) = Signex::new();
        app.ui_state.first_run_tour_open = false;
        app.document_state.engines.insert(
            path.clone(),
            signex_engine::Engine::new(sheet).expect("engine"),
        );
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "power".to_owned(),
            path: path.clone(),
            project_id: None,
            kind: crate::app::TabKind::Schematic,
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        app.document_state.active_path = Some(path.clone());
        let _ = panel(
            &mut app,
            PanelMsg::MessagesView(MessagesPanelMsg::ShowPressed {
                document: path,
                object: Some(item),
            }),
        );
        assert_eq!(app.interaction_state.active_canvas().selected, vec![item]);
    }
}
