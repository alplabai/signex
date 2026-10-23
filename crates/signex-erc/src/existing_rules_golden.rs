//! Regression pin: the rules that predate `PinTypeConflict` must keep
//! producing byte-identical output on the shared fixtures. The golden file was
//! captured from the engine before the pin-matrix rule existed; `PinTypeConflict`
//! findings are filtered out because that rule is new, not because it is exempt
//! from the contract of the others.
//!
//! Regenerate deliberately with `SIGNEX_ERC_PRINT_GOLDEN=1 cargo test -p
//! signex-erc existing_rules_golden -- --nocapture` and review the diff.

use crate::fixtures::context_for;
use crate::{RuleKind, engine};

const NEW_RULE_ID: &str = "builtin::pin_type_conflict";

fn render_existing_rules() -> String {
    let mut lines: Vec<String> = Vec::new();
    for kind in RuleKind::ALL {
        for d in engine::run_all(&context_for(kind)) {
            if d.rule_id.as_str() == NEW_RULE_ID {
                continue;
            }
            // `bus_bit_width_mismatch` picks which of its two labels leads the
            // message and the location from hash-map order, so those two
            // fields differ run to run on the base commit too; pin the rest.
            let (message, x, y) = if d.rule_kind == RuleKind::BusBitWidthMismatch {
                ("<hash-order dependent>", f64::NAN, f64::NAN)
            } else {
                (d.message.as_str(), d.location.x, d.location.y)
            };
            lines.push(format!(
                "{kind:?}|{}|{:?}|{}|{},{}|{:?}|{:?}|{:?}\n",
                d.rule_id,
                d.severity,
                message,
                x,
                y,
                d.location_space,
                d.primary.as_ref().map(|p| p.kind),
                d.peer.as_ref().map(|p| p.kind),
            ));
        }
    }
    // Several rules iterate hash maps, so emission order within a rule is not
    // stable across runs; the contract pinned here is the set of findings.
    lines.sort();
    lines.concat()
}

#[test]
fn existing_rules_output_is_byte_identical_to_the_pre_matrix_golden() {
    let actual = render_existing_rules();
    if std::env::var_os("SIGNEX_ERC_PRINT_GOLDEN").is_some() {
        print!("<<<GOLDEN\n{actual}GOLDEN>>>\n");
    }
    assert_eq!(actual, include_str!("golden/existing_rules.txt"));
}
