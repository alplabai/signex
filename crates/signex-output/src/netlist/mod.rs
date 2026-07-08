//! Netlist export.
//!
//! The Standard-format `.net` S-expression emitter that previously lived
//! here was split out as part of the issue #62 Apache-clean cutover.
//! It moves to the optional `signex-standard-import` GPL-3.0 companion
//! repository alongside the rest of the Standard I/O codepaths.
//!
//! What stays here is the *input side* (ADR-0002 D7): the exporter reads the
//! authoritative [`Netlist`](signex_types::net::Netlist) off [`ExportContext`]
//! — derived once by the app through `signex_net::build_project_netlist` — so
//! future Signex-native emitters (XML, Spice, …) land against the contract
//! instead of re-deriving connectivity. The interim emitter writes a plain,
//! deterministic net listing; the Standard `.net` format is issue #62.

use std::fmt::Write as _;

use thiserror::Error;

use crate::{ExportContext, Exporter};

pub struct NetlistExporter;

#[derive(Debug, Clone, Default)]
pub struct NetlistOptions {
    pub include_timestamps: bool,
}

#[derive(Debug, Clone)]
pub struct NetlistOutput {
    pub bytes: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum NetlistError {
    #[error(
        "no netlist was derived for this export — the app must attach `ExportContext.netlist` (via signex_net::build_project_netlist) before exporting"
    )]
    NoNetlist,
}

impl Exporter for NetlistExporter {
    type Options = NetlistOptions;
    type Output = NetlistOutput;
    type Error = NetlistError;

    fn export(
        &self,
        ctx: &ExportContext,
        _opts: &Self::Options,
    ) -> Result<Self::Output, Self::Error> {
        let netlist = ctx.netlist.as_ref().ok_or(NetlistError::NoNetlist)?;
        Ok(NetlistOutput {
            bytes: render_listing(netlist),
        })
    }
}

/// A plain, deterministic net listing: one `net <id> "<name>"` line per net (in
/// the netlist's stable id order) followed by its `reference.pin` terminals (in
/// the order the derivation already sorted them). Not the Standard `.net`
/// format — that emitter is issue #62 — but a stable, contract-driven dump.
fn render_listing(netlist: &signex_types::net::Netlist) -> Vec<u8> {
    let mut out = String::new();
    let _ = writeln!(out, "# Signex netlist");
    let _ = writeln!(out, "# {} nets", netlist.nets.len());
    for net in &netlist.nets {
        let _ = writeln!(out, "net {} \"{}\"", net.id.0, net.name);
        for t in &net.terminals {
            let _ = writeln!(out, "  {}.{}", t.reference, t.pin);
        }
    }
    out.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::net::{Net, NetId, Netlist, Terminal};
    use uuid::Uuid;

    fn ctx_with(netlist: Option<Netlist>) -> ExportContext {
        ExportContext {
            sheets: Vec::new(),
            metadata: Default::default(),
            netlist,
        }
    }

    #[test]
    fn export_errors_without_a_netlist() {
        let result = NetlistExporter.export(&ctx_with(None), &NetlistOptions::default());
        assert!(matches!(result, Err(NetlistError::NoNetlist)));
    }

    #[test]
    fn export_renders_the_netlist_deterministically() {
        let netlist = Netlist {
            nets: vec![Net {
                id: NetId(1),
                name: "GND".to_string(),
                class: None,
                wires: Vec::new(),
                junctions: Vec::new(),
                terminals: vec![
                    Terminal {
                        symbol: Uuid::nil(),
                        reference: "R1".to_string(),
                        pin: "1".to_string(),
                    },
                    Terminal {
                        symbol: Uuid::nil(),
                        reference: "U2".to_string(),
                        pin: "3".to_string(),
                    },
                ],
            }],
        };
        let out = NetlistExporter
            .export(&ctx_with(Some(netlist)), &NetlistOptions::default())
            .expect("netlist present");
        let text = String::from_utf8(out.bytes).unwrap();
        assert_eq!(
            text,
            "# Signex netlist\n# 1 nets\nnet 1 \"GND\"\n  R1.1\n  U2.3\n"
        );
    }
}
