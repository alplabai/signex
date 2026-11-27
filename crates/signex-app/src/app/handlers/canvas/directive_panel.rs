//! The Properties snapshot of a selected Net Parameters directive or Net
//! Blanket (#1281, #1489), and its events: an edit keeps the typed text as a draft
//! while it is typed, a net row's hover lights that net on the sheet, and a
//! hover on the illustration or a linked row lights the other side.

use std::sync::Arc;

use signex_types::schematic::{SchDirective, SelectedKind};

use super::super::super::*;
use super::place_directive::{outline_value, parse_value};
use crate::panels::{
    BlanketEdit, DirectivePanel, DirectivePanelMsg, DraftField, NetParametersEdit, NetRule,
    OutlineField,
};

/// The draft a typed edit leaves, `None` for a pick or a checkbox.
fn blanket_draft(edit: &BlanketEdit) -> Option<(DraftField, String)> {
    match edit {
        BlanketEdit::TrackWidth(text) => Some((DraftField::TrackWidth, text.clone())),
        BlanketEdit::Outline(field, text) => Some((DraftField::Outline(*field), text.clone())),
        _ => None,
    }
}

/// Whether `field` refuses typed `text`: nothing was applied, and the box
/// keeps the text and shows it is invalid (#2767).
fn draft_refused(field: DraftField, text: &str) -> bool {
    match field {
        DraftField::Rule(_) | DraftField::TrackWidth => parse_value(text).is_err(),
        DraftField::Outline(side) => outline_value(side, text).is_none(),
    }
}

/// Whether the draft still stands: it is refused text, or it reads as what is
/// stored. A value changed by anything else (an undo, a command) drops it.
fn draft_still_true(
    directive: &SchDirective,
    page: (signex_types::schematic::PageOrigin, f64),
    draft: &(DraftField, String),
) -> bool {
    let (field, text) = draft;
    let stored = match (field, directive) {
        (DraftField::Rule(rule), SchDirective::NetParameters { rules, .. }) => match rule {
            NetRule::Width => rules.width_mm,
            NetRule::Clearance => rules.clearance_mm,
            NetRule::ViaDiameter => rules.via_diameter_mm,
            NetRule::ViaHole => rules.via_hole_mm,
            NetRule::Current => rules.current_a,
        },
        (DraftField::TrackWidth, SchDirective::Blanket { track_width, .. }) => *track_width,
        (DraftField::Outline(f), SchDirective::Blanket { start, end, .. }) => Some(match f {
            OutlineField::X => start.x,
            OutlineField::Y => page.0.map_y(start.y, page.1),
            OutlineField::Width => (end.x - start.x).abs(),
            OutlineField::Height => (end.y - start.y).abs(),
        }),
        _ => return false,
    };
    if draft_refused(*field, text) {
        return true;
    }
    let typed = match field {
        DraftField::Outline(_) => text.trim().parse::<f64>().ok(),
        DraftField::Rule(_) | DraftField::TrackWidth => parse_value(text).ok().flatten(),
    };
    typed == stored
}

impl Signex {
    /// Rebuild the directive Properties snapshot from the selection: the one
    /// selected Net Parameters directive or Net Blanket of the sheet in view,
    /// keeping its hover and a draft that still stands.
    pub(crate) fn refresh_directive_panel(&mut self) {
        let previous = self.document_view.panel_ctx.directive_panel.take();
        let selected = &self.interaction_state.active_canvas().selected;
        let [item] = selected.as_slice() else {
            return;
        };
        if item.kind != SelectedKind::Directive {
            return;
        }
        let id = item.uuid;
        let Some(path) = self.document_state.active_path.clone() else {
            return;
        };
        let Some(engine) = self.document_state.engines.get(&path) else {
            return;
        };
        let (net_classes, diff_pair_classes) = self
            .document_state
            .document_project(&path)
            .map(|project| {
                (
                    project
                        .data
                        .net_classes
                        .iter()
                        .map(|c| c.name.clone())
                        .collect(),
                    project
                        .data
                        .schematic_design
                        .diff_pair_classes
                        .iter()
                        .map(|c| c.name.clone())
                        .collect(),
                )
            })
            .unwrap_or_default();
        let sheet_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(mut panel) = DirectivePanel::build(
            engine.document(),
            id,
            &sheet_name,
            net_classes,
            diff_pair_classes,
        ) else {
            return;
        };
        if let Some(old) = previous.filter(|old| old.directive.uuid() == id) {
            panel.hover = old.hover;
            panel.hovered_net = old.hovered_net;
            panel.draft = old
                .draft
                .filter(|draft| draft_still_true(&panel.directive, panel.page, draft));
            panel.draft_refused = panel
                .draft
                .as_ref()
                .is_some_and(|(field, text)| draft_refused(*field, text));
        }
        self.document_view.panel_ctx.directive_panel = Some(Box::new(panel));
    }

    /// A Net Blanket edit from its Properties.
    pub(crate) fn blanket_panel_edited(&mut self, id: uuid::Uuid, edit: &BlanketEdit) {
        if let Some(panel) = self.document_view.panel_ctx.directive_panel.as_mut() {
            panel.draft = blanket_draft(edit);
        }
        self.edit_blanket_field(id, edit);
        self.refresh_directive_panel();
    }

    /// An event of a directive's Properties.
    pub(crate) fn directive_panel_event(&mut self, event: &DirectivePanelMsg) {
        match event {
            DirectivePanelMsg::NetParametersEdited(id, edit) => {
                if let Some(panel) = self.document_view.panel_ctx.directive_panel.as_mut() {
                    panel.draft = match edit {
                        NetParametersEdit::Rule(rule, text) => {
                            Some((DraftField::Rule(*rule), text.clone()))
                        }
                        _ => None,
                    };
                }
                self.edit_net_parameters_field(*id, edit);
                self.refresh_directive_panel();
            }
            DirectivePanelMsg::NetRowHovered(name) => self.directive_net_row_hovered(name.clone()),
            DirectivePanelMsg::Illustration(m) => {
                if let Some(panel) = self.document_view.panel_ctx.directive_panel.as_mut() {
                    panel.hover.update(m.clone());
                }
            }
        }
    }

    /// Light the net of a hovered Net Blanket row on the sheet in view, or
    /// put back the net highlight the Net Inspector owns when it is left.
    fn directive_net_row_hovered(&mut self, name: Option<String>) {
        let elements = self
            .document_view
            .panel_ctx
            .directive_panel
            .as_mut()
            .and_then(|panel| {
                panel.hovered_net = name.clone();
                let name = name.as_deref()?;
                panel.nets.iter().find(|n| n.name == name).map(|n| {
                    n.elements
                        .iter()
                        .copied()
                        .collect::<std::collections::HashSet<_>>()
                })
            });
        match elements {
            Some(set) => self
                .interaction_state
                .canvas
                .set_net_highlight(Some(Arc::new(set))),
            None => self.sync_net_highlight_to_canvases(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{BlanketPour, PageOrigin, Point};

    fn blanket(track_width: Option<f64>) -> SchDirective {
        SchDirective::Blanket {
            uuid: uuid::Uuid::from_u128(1),
            start: Point::new(10.0, 20.0),
            end: Point::new(60.0, 50.0),
            net_class: None,
            erc_exclusions: Vec::new(),
            track_width,
            pour: BlanketPour::Normal,
            include_edge_nets: false,
        }
    }

    #[test]
    fn a_draft_stands_while_it_reads_as_the_stored_value() {
        let page = (PageOrigin::UpperLeft, 297.0);
        let typed = |text: &str| (DraftField::TrackWidth, text.to_string());
        assert!(draft_still_true(&blanket(Some(1.0)), page, &typed("1.")));
        assert!(draft_still_true(&blanket(None), page, &typed("")));
        assert!(!draft_still_true(&blanket(Some(2.0)), page, &typed("1.")));
        let width = (DraftField::Outline(OutlineField::Width), "50.0".to_string());
        assert!(draft_still_true(&blanket(None), page, &width));
        let half = (DraftField::Outline(OutlineField::X), "-".to_string());
        assert!(draft_still_true(&blanket(None), page, &half));
    }

    #[test]
    fn refused_text_stands_as_typed_and_a_stored_value_does_not_hold_it() {
        let page = (PageOrigin::UpperLeft, 297.0);
        for (field, text) in [
            (DraftField::TrackWidth, "-1"),
            (DraftField::TrackWidth, "wide"),
            (DraftField::Outline(OutlineField::Width), "0"),
            (DraftField::Outline(OutlineField::Height), "-4"),
            (DraftField::Outline(OutlineField::X), "inf"),
            (DraftField::Outline(OutlineField::Y), ""),
        ] {
            assert!(draft_refused(field, text), "{field:?} {text:?}");
            let draft = (field, text.to_string());
            assert!(draft_still_true(&blanket(Some(1.0)), page, &draft));
        }
    }

    #[test]
    fn a_value_the_field_can_store_is_not_refused() {
        for (field, text) in [
            (DraftField::TrackWidth, ""),
            (DraftField::TrackWidth, "0"),
            (DraftField::Rule(NetRule::Current), "3"),
            (DraftField::Outline(OutlineField::X), "-5"),
            (DraftField::Outline(OutlineField::Width), "0.1"),
        ] {
            assert!(!draft_refused(field, text), "{field:?} {text:?}");
        }
    }
}
