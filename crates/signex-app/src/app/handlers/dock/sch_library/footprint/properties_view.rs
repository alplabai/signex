//! Footprint-editor handler for the Properties panel's view state: the
//! hover link between fields and the illustration. View state only: the tab
//! is not dirtied, no undo step is recorded and the canvas keeps its cache.

use crate::app::Signex;
use crate::panels::FpPropertiesMsg;

impl Signex {
    pub(in crate::app::handlers::dock::sch_library) fn fp_properties_view_changed(
        &mut self,
        msg: &FpPropertiesMsg,
    ) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            match msg {
                FpPropertiesMsg::Illustration(hover) => {
                    editor.state.properties_hover.update(hover.clone());
                }
            }
        }
        self.refresh_panel_ctx();
        true
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use signex_library::{Footprint, FootprintFile, Pad};
    use signex_widgets::property_illustration::{FieldId, IllustrationMsg};

    use crate::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
    use crate::dock::DockMessage;
    use crate::panels::{FpPropertiesMsg, PanelMsg};

    const PATH: &str = "properties-view.snxfpt";

    fn app_with_a_footprint() -> Signex {
        let path = PathBuf::from(PATH);
        let mut footprint = Footprint::empty("VIEW");
        footprint.pads.push(Pad {
            number: "1".into(),
            size: [1.0, 1.0],
            ..Pad::default()
        });
        let editor = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(footprint));
        let (mut app, _task) = Signex::new();
        app.document_state
            .footprint_editors
            .insert(path.clone(), editor);
        app.document_state.tabs.push(TabInfo {
            title: "VIEW".into(),
            path: path.clone(),
            project_id: None,
            kind: TabKind::FootprintEditor(path),
        });
        app.document_state.active_tab = 0;
        app
    }

    fn send(app: &mut Signex, msg: FpPropertiesMsg) {
        let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::FpProperties(
            msg,
        ))));
    }

    fn editor(app: &Signex) -> &FootprintDocument {
        &app.document_state.footprint_editors[&PathBuf::from(PATH)]
    }

    /// Hovering a field reaches the panel, and leaves the tab clean with
    /// nothing to undo.
    #[test]
    fn hover_messages_change_the_view_and_leave_the_tab_clean() {
        let mut app = app_with_a_footprint();
        let field = FieldId::new("pad-size-x");
        send(
            &mut app,
            FpPropertiesMsg::Illustration(IllustrationMsg::FieldHovered(field.clone())),
        );

        let ed = editor(&app);
        assert!(ed.state.properties_hover.is_field_highlighted(&field));
        assert!(!ed.is_dirty(), "view state must not dirty the tab");
        assert!(
            !ed.history.can_undo(),
            "view state must not record an undo step"
        );

        let ctx = app
            .document_view
            .panel_ctx
            .footprint_editor
            .as_ref()
            .expect("the footprint panel context is built");
        assert!(ctx.illustration_hover.is_field_highlighted(&field));

        send(
            &mut app,
            FpPropertiesMsg::Illustration(IllustrationMsg::FieldLeft(field.clone())),
        );
        assert!(
            !editor(&app)
                .state
                .properties_hover
                .is_field_highlighted(&field)
        );
        assert!(!editor(&app).is_dirty());
    }
}
