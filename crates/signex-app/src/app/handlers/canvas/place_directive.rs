//! The directive tools (#2420, #1489): a Net Blanket or Compile Mask is two
//! corner clicks, a Differential Pair directive one click on a `_P` or `_N`
//! net, a Net Parameters flag one click on a wire. Every placement and edit
//! is one `Command` through the gateway, so one undo step. Design note:
//! `docs/SCHEMATIC_DIRECTIVES.md`.

use signex_types::schematic::{
    BlanketPour, NetRules, PageOrigin, Point, SchDirective, SchematicSheet,
};

use super::super::super::*;
use super::shape_tool::ClickKind;
use crate::canvas::ShapePreviewKind;
use crate::ignore::IgnoreResult;
use crate::panels::{BlanketEdit, NetParametersEdit, NetRule, OutlineField};

/// A Net Blanket over the region `start`..`end`.
pub(crate) fn blanket_directive(
    start: Point,
    end: Point,
    net_class: Option<String>,
    erc_exclusions: Vec<String>,
) -> SchDirective {
    SchDirective::Blanket {
        uuid: uuid::Uuid::now_v7(),
        start,
        end,
        net_class: net_class.filter(|c| !c.trim().is_empty()),
        erc_exclusions,
        track_width: None,
        pour: BlanketPour::Normal,
        include_edge_nets: false,
    }
}

/// A Compile Mask over the region `start`..`end`.
pub(crate) fn compile_mask_directive(start: Point, end: Point) -> SchDirective {
    SchDirective::CompileMask {
        uuid: uuid::Uuid::now_v7(),
        start,
        end,
    }
}

/// A Differential Pair directive at `position`.
pub(crate) fn diff_pair_directive(position: Point) -> SchDirective {
    SchDirective::DiffPair {
        uuid: uuid::Uuid::now_v7(),
        position,
        class: None,
    }
}

/// A Net Parameters directive at `position` with nothing set yet.
pub(crate) fn net_parameters_directive(position: Point) -> SchDirective {
    SchDirective::NetParameters {
        uuid: uuid::Uuid::now_v7(),
        position,
        net_class: None,
        diff_pair_class: None,
        rules: NetRules::default(),
        show_on_flag: false,
    }
}

/// Why a Net Parameters flag was not placed: it sits on a wire.
pub(crate) const NET_PARAMETERS_OFF_WIRE: &str =
    "Net Parameters sit on a wire: click on a wire; nothing was placed";

/// A typed length or current: blank is unset, anything else must be a
/// finite number not below zero.
pub(crate) fn parse_value(text: &str) -> Result<Option<f64>, ()> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    match text.parse::<f64>() {
        Ok(v) if v.is_finite() && v >= 0.0 => Ok(Some(v)),
        _ => Err(()),
    }
}

/// `directive` (a Net Blanket) with `edit` applied; `None` when the edit
/// does not parse or does not apply. `page` maps a typed Y to the stored Y.
pub(crate) fn blanket_edited(
    directive: &SchDirective,
    edit: &BlanketEdit,
    page: (PageOrigin, f64),
) -> Option<SchDirective> {
    let mut out = directive.clone();
    let SchDirective::Blanket {
        start,
        end,
        net_class,
        erc_exclusions,
        track_width,
        pour,
        include_edge_nets,
        ..
    } = &mut out
    else {
        return None;
    };
    match edit {
        // Kept as typed, spaces included, so a typed field does not fight
        // the cursor; readers trim it (`SchDirective::net_class`).
        BlanketEdit::NetClass(value) => {
            *net_class = Some(value.clone()).filter(|c| !c.trim().is_empty());
        }
        BlanketEdit::Rule { rule_id, excluded } => {
            erc_exclusions.retain(|r| r != rule_id);
            if *excluded {
                erc_exclusions.push(rule_id.clone());
            }
        }
        BlanketEdit::TrackWidth(text) => *track_width = parse_value(text).ok()?,
        BlanketEdit::Pour(value) => *pour = *value,
        BlanketEdit::IncludeEdgeNets(on) => *include_edge_nets = *on,
        BlanketEdit::Outline(field, text) => {
            let value = text.trim().parse::<f64>().ok().filter(|v| v.is_finite())?;
            outline_edited(start, end, *field, value, page)?;
        }
    }
    Some(out)
}

/// Move the region `start`..`end` to a typed corner, or size it from
/// `start`. X / Y is the `start` corner in page coordinates.
fn outline_edited(
    start: &mut Point,
    end: &mut Point,
    field: OutlineField,
    value: f64,
    (origin, paper_height): (PageOrigin, f64),
) -> Option<()> {
    let sign = |d: f64| if d < 0.0 { -1.0 } else { 1.0 };
    match field {
        OutlineField::X => {
            let shift = value - start.x;
            start.x += shift;
            end.x += shift;
        }
        OutlineField::Y => {
            let shift = origin.map_y(value, paper_height) - start.y;
            start.y += shift;
            end.y += shift;
        }
        OutlineField::Width if value > 0.0 => end.x = start.x + sign(end.x - start.x) * value,
        OutlineField::Height if value > 0.0 => end.y = start.y + sign(end.y - start.y) * value,
        OutlineField::Width | OutlineField::Height => return None,
    }
    Some(())
}

/// `directive` (a Net Parameters directive) with `edit` applied; `None`
/// when the edit does not parse or does not apply.
pub(crate) fn net_parameters_edited(
    directive: &SchDirective,
    edit: &NetParametersEdit,
) -> Option<SchDirective> {
    let mut out = directive.clone();
    let SchDirective::NetParameters {
        net_class,
        diff_pair_class,
        rules,
        show_on_flag,
        ..
    } = &mut out
    else {
        return None;
    };
    match edit {
        NetParametersEdit::NetClass(class) => *net_class = class.clone(),
        NetParametersEdit::DiffPairClass(class) => *diff_pair_class = class.clone(),
        NetParametersEdit::ShowOnFlag(on) => *show_on_flag = *on,
        NetParametersEdit::Rule(rule, text) => {
            let value = parse_value(text).ok()?;
            let slot = match rule {
                NetRule::Width => &mut rules.width_mm,
                NetRule::Clearance => &mut rules.clearance_mm,
                NetRule::ViaDiameter => &mut rules.via_diameter_mm,
                NetRule::ViaHole => &mut rules.via_hole_mm,
                NetRule::Current => &mut rules.current_a,
            };
            *slot = value;
        }
    }
    Some(out)
}

/// Whether a Net Parameters flag at `at` sits on a wire of `sheet`.
pub(crate) fn is_on_wire(sheet: &SchematicSheet, at: Point) -> bool {
    signex_net::wire_at_point(sheet, &at).is_some()
}

impl Signex {
    /// One snapped point of the Net Blanket or Compile Mask tool: the first
    /// click anchors a corner (the rubber band follows the cursor), the
    /// second places the region and leaves the tool armed for the next one.
    /// A double-click stands for the second click and never seeds a corner.
    pub(super) fn directive_region_point(&mut self, tool: Tool, p: Point, kind: ClickKind) {
        match self.interaction_state.shape_anchor.take() {
            None if kind == ClickKind::Click => {
                self.set_shape_anchor(Some((p, ShapePreviewKind::Rect)));
            }
            None => {}
            Some(start) => {
                self.set_shape_anchor(None);
                let directive = if tool == Tool::Blanket {
                    blanket_directive(start, p, None, Vec::new())
                } else {
                    compile_mask_directive(start, p)
                };
                // The gateway logs a refusal itself.
                self.place_directive(directive)
                    .ignore("the gateway logs a refusal itself");
            }
        }
    }

    /// One snapped click of the Differential Pair tool.
    pub(super) fn diff_pair_click(&mut self, at: Point) {
        // The gateway logs a refusal itself.
        self.place_directive(diff_pair_directive(at))
            .ignore("the gateway logs a refusal itself");
    }

    /// One snapped click of the Net Parameters tool: on a wire it places
    /// the flag there; anywhere else nothing is placed and Messages says so.
    pub(super) fn net_parameters_click(&mut self, at: Point) {
        let on_wire = self
            .document_state
            .edit_engine()
            .is_some_and(|engine| is_on_wire(engine.document(), at));
        if !on_wire {
            let line = signex_i18n::t!(self.ui_state.i18n, signex_i18n::ids::PANELS_NP_OFF_WIRE);
            crate::diagnostics::log_info(line.into_string());
            return;
        }
        // The gateway logs a refusal itself.
        self.place_net_parameters(net_parameters_directive(at))
            .ignore("the gateway logs a refusal itself");
    }

    /// Place a Net Parameters directive, refused off a wire.
    pub(crate) fn place_net_parameters(&mut self, directive: SchDirective) -> Result<(), String> {
        let on_wire = self
            .document_state
            .edit_engine()
            .is_some_and(|engine| is_on_wire(engine.document(), directive.anchor()));
        if !on_wire {
            return Err(NET_PARAMETERS_OFF_WIRE.to_string());
        }
        self.place_directive(directive)
    }

    /// Place `directive` on the edit target as one undo step, or say why not.
    pub(crate) fn place_directive(&mut self, directive: SchDirective) -> Result<(), String> {
        self.commit_directive(signex_engine::Command::PlaceDirective { directive })
    }

    /// Replace a placed directive (same uuid and kind). Consecutive edits of
    /// one directive share one undo step, like typing a field; a caller that
    /// is one step on its own ends the run with [`Self::end_directive_edit_run`].
    pub(crate) fn update_directive(&mut self, directive: SchDirective) -> Result<(), String> {
        self.commit_directive(signex_engine::Command::UpdateDirective { directive })
    }

    /// The directive `id` on the edit target, as stored.
    pub(crate) fn placed_directive(&self, id: uuid::Uuid) -> Option<SchDirective> {
        let engine = self.document_state.edit_engine()?;
        engine
            .document()
            .directives
            .iter()
            .find(|d| d.uuid() == id)
            .cloned()
    }

    /// A Properties edit of the Net Blanket `id`. Typed fields are one undo
    /// step per typing run; each checkbox, rule or pick is one step.
    pub(crate) fn edit_blanket_field(&mut self, id: uuid::Uuid, edit: &BlanketEdit) {
        let Some(directive) = self.placed_directive(id) else {
            return;
        };
        let page =
            self.document_state
                .edit_engine()
                .map_or((PageOrigin::UpperLeft, 0.0), |engine| {
                    let doc = engine.document();
                    (
                        doc.page.origin,
                        signex_types::schematic::paper_height_mm(&doc.paper_size),
                    )
                });
        let typing = matches!(
            edit,
            BlanketEdit::NetClass(_) | BlanketEdit::TrackWidth(_) | BlanketEdit::Outline(..)
        );
        let Some(edited) = blanket_edited(&directive, edit, page) else {
            return;
        };
        self.commit_directive_edit(edited, typing);
    }

    /// A Properties edit of the Net Parameters directive `id`. Typed rule
    /// values are one undo step per typing run; a pick or a checkbox one step.
    pub(crate) fn edit_net_parameters_field(&mut self, id: uuid::Uuid, edit: &NetParametersEdit) {
        let Some(directive) = self.placed_directive(id) else {
            return;
        };
        let typing = matches!(edit, NetParametersEdit::Rule(..));
        let Some(edited) = net_parameters_edited(&directive, edit) else {
            return;
        };
        self.commit_directive_edit(edited, typing);
    }

    /// Commit an edited directive: a typed value continues the typing run,
    /// anything else is one step of its own.
    fn commit_directive_edit(&mut self, edited: SchDirective, typing: bool) {
        if !typing {
            self.end_directive_edit_run();
        }
        // The gateway logs a refusal itself; an unchanged value is no edit.
        self.update_directive(edited)
            .ignore("the gateway logs a refusal itself; an unchanged value is no edit");
        if !typing {
            self.end_directive_edit_run();
        }
    }

    /// End the typing run of directive edits on the edit target, so the next
    /// edit is an undo step of its own.
    pub(crate) fn end_directive_edit_run(&mut self) {
        if let Some(engine) = self.document_state.edit_engine_mut() {
            engine.end_field_edit_run();
        }
    }

    fn commit_directive(&mut self, command: signex_engine::Command) -> Result<(), String> {
        let (outcome, refusal) = self.apply_engine_command_reporting(command, true, true);
        match (outcome, refusal) {
            (_, Some(reason)) => Err(reason),
            (crate::app::mutation_gateway::MutationOutcome::Changed, None) => Ok(()),
            _ => Err("the directive is already as given".to_string()),
        }
    }
}
