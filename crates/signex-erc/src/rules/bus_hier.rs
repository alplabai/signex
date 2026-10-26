//! Buses across sheets (#1049): a bus-typed sheet pin must carry what the
//! child sheet's matching port carries.
//!
//! A pin and a port are the same bus when their names share a
//! [`join_key`](signex_types::schematic::BusName::join_key): `D[0..7]` on the
//! parent and `D[0..3]` on the child are one bus at two widths, and that is
//! the finding. A pin with no such counterpart is the plain "no matching
//! hierarchical label" case `bad_hier_sheet_pin` already reports.

use std::collections::HashSet;

use signex_types::schematic::{SelectedKind, normalize_port_name, parse_bus_name};

use crate::context::ErcContext;
use crate::diagnostic::Diagnostic;
use crate::{RuleKind, sel};

/// True when `name` is a bus and one of `others` is the same bus, at whatever
/// width. Lets the pin/label match rule leave a width disagreement to
/// [`bus_port_width_mismatch`] instead of also calling it an unmatched name.
pub(super) fn has_bus_counterpart<'a>(
    name: &str,
    mut others: impl Iterator<Item = &'a str>,
) -> bool {
    let Ok(this) = parse_bus_name(name) else {
        return false;
    };
    let key = this.join_key();
    others.any(|o| parse_bus_name(o).is_ok_and(|b| b.join_key() == key))
}

/// A bus-typed sheet pin whose child port names the same bus with different
/// members. Names both sides and the width each one carries.
pub(crate) fn bus_port_width_mismatch(ctx: &ErcContext, out: &mut Vec<Diagnostic>) {
    for child in &ctx.child_sheets {
        let Some(child_ctx) = ctx.children.get(child.filename.as_str()) else {
            continue;
        };
        for pin in &child.pins {
            let Some(pin_bus) = normalize_port_name(&pin.name).and_then(|n| parse_bus_name(n).ok())
            else {
                continue;
            };
            let key = pin_bus.join_key();
            let pin_members: HashSet<String> = pin_bus.members().into_iter().collect();
            for port in &child_ctx.labels {
                let Some(port_bus) = port.port_name().and_then(|n| parse_bus_name(n).ok()) else {
                    continue;
                };
                let port_members: HashSet<String> = port_bus.members().into_iter().collect();
                if port_bus.join_key() != key || port_members == pin_members {
                    continue;
                }
                out.push(
                    Diagnostic::new(
                        RuleKind::BusBitWidthMismatch,
                        format!(
                            "Bus width mismatch across sheets: sheet pin '{}' on '{}' carries {} \
                             signals, port '{}' in '{}' carries {}",
                            pin.name,
                            child.name,
                            pin_bus.width(),
                            port.text,
                            child.filename,
                            port_bus.width(),
                        ),
                        pin.position,
                    )
                    .with_primary(sel(pin.uuid, SelectedKind::SheetPin)),
                );
            }
        }
    }
}
