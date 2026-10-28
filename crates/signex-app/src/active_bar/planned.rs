//! Active Bar tools that are shown but not built yet (#1249).
//!
//! Each of these used to arm a different tool (a rectangle, a circle, a
//! line, a `DIFF_PAIR` text note) or only log a line, so a user could not
//! tell the feature did not exist. They are now greyed with a "planned"
//! tooltip and refused wherever they can still be reached: the keymap, the
//! command palette, the context-menu Place flyout and a group button's
//! remembered last tool all end in `handle_active_bar_action`.
//!
//! The list only shrinks. When a real tool lands, delete its row here and
//! give it a real arm in `handlers/active_bar/placement_presets.rs`.

use super::ActiveBarAction;

/// Tooltip shown on every planned row, in the bar, the menus and the
/// context menus.
pub const PLANNED_TOOLTIP: &str = "Planned — not available yet";

/// Every tool that is still a placeholder, with the name the Messages line
/// uses when it refuses the click.
pub const PLANNED_TOOLS: &[(ActiveBarAction, &str)] = &[
    (
        ActiveBarAction::PlaceDeviceSheetSymbol,
        "Device Sheet Symbol",
    ),
    (ActiveBarAction::PlaceReuseBlock, "Reuse Block"),
    (ActiveBarAction::DrawRoundRectangle, "Round Rectangle"),
    (ActiveBarAction::PlaceGraphic, "Graphic"),
    (ActiveBarAction::PlaceBlanket, "Blanket"),
    // #1248: a rule directive must never enter the netlist, and no
    // non-electrical directive object exists yet.
    (ActiveBarAction::PlaceParameterSet, "Parameter Set"),
    (ActiveBarAction::PlaceCompileMask, "Compile Mask"),
    (ActiveBarAction::PlaceDiffPair, "Differential Pair"),
    (ActiveBarAction::DrawBezier, "Bezier"),
    (ActiveBarAction::DrawEllipse, "Ellipse"),
    (ActiveBarAction::DrawEllipticalArc, "Elliptical Arc"),
    (ActiveBarAction::PlaceSignalHarness, "Signal Harness"),
    (ActiveBarAction::PlaceHarnessConnector, "Harness Connector"),
    (ActiveBarAction::PlaceHarnessEntry, "Harness Entry"),
];

/// The user-facing name of `action` when it is a planned placeholder.
pub fn planned_name(action: &ActiveBarAction) -> Option<&'static str> {
    PLANNED_TOOLS
        .iter()
        .find(|(candidate, _)| candidate == action)
        .map(|(_, name)| *name)
}

/// Whether `action` is a planned placeholder that must not run.
pub fn is_planned(action: &ActiveBarAction) -> bool {
    planned_name(action).is_some()
}

/// The Messages line for a refused planned tool. The reason comes first so
/// the panel's ~157-byte cut can never drop it.
pub fn refusal_line(name: &str) -> String {
    let line = format!("Planned, not available yet: {name} was not placed");
    crate::text_trunc::prefix_within_bytes(&line, 157).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ratchet (#1249): the placeholder set may only shrink. A new entry
    /// fails here; add a real tool instead, or — if a placeholder really
    /// must ship — add its name to this list in the same change and say
    /// why in the PR.
    const EXPECTED: &[&str] = &[
        "Device Sheet Symbol",
        "Reuse Block",
        "Round Rectangle",
        "Graphic",
        "Blanket",
        "Parameter Set",
        "Compile Mask",
        "Differential Pair",
        "Bezier",
        "Ellipse",
        "Elliptical Arc",
        "Signal Harness",
        "Harness Connector",
        "Harness Entry",
    ];

    #[test]
    fn planned_tool_set_can_only_shrink() {
        let names: Vec<&str> = PLANNED_TOOLS.iter().map(|(_, n)| *n).collect();
        for name in &names {
            assert!(
                EXPECTED.contains(name),
                "`{name}` is a new placeholder Active Bar tool. Add a real tool \
                 instead of a fallback; if it must ship planned, add it to \
                 EXPECTED in active_bar/planned.rs and justify it in the PR."
            );
        }
        assert_eq!(
            names.len(),
            EXPECTED.len(),
            "a tool left PLANNED_TOOLS: drop it from EXPECTED so the ratchet tightens"
        );
    }

    /// The tool each arm may arm through `SelectTool(Tool::<shape>)`: the
    /// shape's own action, never a stand-in for a different feature. This
    /// is what #1249 was: an action named for one thing arming another.
    const HONEST_ARMS: &[(&str, &str)] = &[
        ("DrawLine", "Line"),
        ("DrawRectangle", "Rectangle"),
        ("DrawFullCircle", "Circle"),
        ("PlaceTextString", "Text"),
        // Known limitation, no open issue tracks it yet (searched 2026-10-27):
        // a bounded text box is not built, so these place a plain text note
        // and log that they do. File one before widening this list.
        ("PlaceNote", "Text"),
        ("PlaceTextFrame", "Text"),
    ];

    /// Every `(action, tool)` pair where an `ActiveBarAction` arm in
    /// `source` arms one of the stand-in shape tools. The arm's patterns are
    /// the `ActiveBarAction::` lines that end at the `=>` before the call.
    fn shape_arms(source: &str) -> Vec<(String, String)> {
        let mut found = Vec::new();
        for tool in ["Rectangle", "Circle", "Line", "Text"] {
            let call = format!("SelectTool(Tool::{tool})");
            let mut from = 0;
            while let Some(rel) = source[from..].find(&call) {
                let at = from + rel;
                from = at + call.len();
                let Some(arrow) = source[..at].rfind("=>") else {
                    continue;
                };
                let lines: Vec<&str> = source[..arrow + 2].lines().collect();
                let mut names = Vec::new();
                for line in lines.iter().rev() {
                    let t = line.trim_start();
                    if !(t.starts_with("ActiveBarAction::") || t.starts_with('|')) {
                        break;
                    }
                    for part in t.split("ActiveBarAction::").skip(1) {
                        let end = part
                            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                            .unwrap_or(part.len());
                        names.push(part[..end].to_string());
                    }
                }
                for name in names {
                    found.push((name, tool.to_string()));
                }
            }
        }
        found
    }

    fn dishonest(source: &str) -> Vec<(String, String)> {
        shape_arms(source)
            .into_iter()
            .filter(|(action, tool)| !HONEST_ARMS.contains(&(action.as_str(), tool.as_str())))
            .collect()
    }

    /// Ratchet on real handler code: no Active Bar arm may arm a
    /// rectangle / circle / line / text tool unless that is what the
    /// action is. A new fake fallback fails here.
    #[test]
    fn no_active_bar_arm_arms_a_stand_in_shape_tool() {
        for (file, source) in [
            (
                "placement_presets.rs",
                include_str!("../app/handlers/active_bar/placement_presets.rs"),
            ),
            (
                "action_groups.rs",
                include_str!("../app/handlers/active_bar/action_groups.rs"),
            ),
        ] {
            let bad = dishonest(source);
            assert!(
                bad.is_empty(),
                "{file}: {bad:?} arm a stand-in shape tool. Add a real tool, or \
                 add the action to PLANNED_TOOLS so it is refused instead."
            );
        }
        let honest = shape_arms(include_str!("../app/handlers/active_bar/action_groups.rs"));
        assert!(
            honest.iter().any(|(a, t)| a == "DrawLine" && t == "Line"),
            "the scanner must see the real arms (found {honest:?})"
        );
    }

    #[test]
    fn the_scanner_fails_when_a_fake_fallback_arm_is_added() {
        let fake = "            ActiveBarAction::PlaceDiffPair => {\n                follow = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Rectangle)));\n            }";
        assert_eq!(
            dishonest(fake),
            vec![("PlaceDiffPair".to_string(), "Rectangle".to_string())]
        );
        let multi = "            ActiveBarAction::PlaceBlanket\n            | ActiveBarAction::PlaceCompileMask => {\n                follow = self.update(Message::Tool(ToolMessage::SelectTool(Tool::Rectangle)));";
        assert_eq!(dishonest(multi).len(), 2);
        let honest = "            ActiveBarAction::DrawRectangle => {\n                self.update(Message::Tool(ToolMessage::SelectTool(Tool::Rectangle)))";
        assert!(dishonest(honest).is_empty());
    }

    #[test]
    fn planned_tools_have_no_duplicates() {
        for (i, (a, _)) in PLANNED_TOOLS.iter().enumerate() {
            assert!(
                PLANNED_TOOLS.iter().skip(i + 1).all(|(b, _)| b != a),
                "{a:?} listed twice"
            );
        }
    }

    #[test]
    fn a_bar_button_whose_action_is_planned_says_so() {
        use super::super::{ActiveBarMsg, button_tooltip};
        let harness = ActiveBarMsg::Action(ActiveBarAction::PlaceSignalHarness);
        assert_eq!(
            button_tooltip(&harness, "Harness"),
            format!("Harness — {PLANNED_TOOLTIP}")
        );
        let wire = ActiveBarMsg::Action(ActiveBarAction::DrawWire);
        assert_eq!(button_tooltip(&wire, "Wiring"), "Wiring");
    }

    #[test]
    fn refusal_line_is_reason_first_and_within_the_cut() {
        let line = refusal_line("Differential Pair");
        assert!(line.starts_with("Planned, not available yet"));
        assert!(line.len() <= 157);
        let long = refusal_line(&"x".repeat(400));
        assert!(long.len() <= 157);
    }
}
