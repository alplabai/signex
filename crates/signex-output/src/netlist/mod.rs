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
    /// When `Some`, the listing is prefixed with an INCOMPLETE-netlist header
    /// comment block — one comment line per string, naming the pages this
    /// export does not cover. Set by the app when the user chooses "Export
    /// anyway (incomplete)" (#431) so the omission is recorded *in the file*,
    /// not only in a dismissible dialog: a downstream PCB import can then see
    /// the netlist is partial. `None` (the default) emits the listing
    /// unchanged, byte-for-byte.
    pub incomplete_note: Option<Vec<String>>,
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
        opts: &Self::Options,
    ) -> Result<Self::Output, Self::Error> {
        let netlist = ctx.netlist.as_ref().ok_or(NetlistError::NoNetlist)?;
        Ok(NetlistOutput {
            bytes: render_listing(netlist, opts.incomplete_note.as_deref()),
        })
    }
}

/// Comment-line marker for the `.net` listing. Every line beginning with this
/// prefix is an annotation, not connectivity — the same convention the
/// `# Signex netlist` banner already uses. A downstream importer skips lines
/// that start with it. Kept as a constant so the marker is defined in exactly
/// one place across the banner and the #431 INCOMPLETE header.
const COMMENT_PREFIX: &str = "# ";

/// The `# `-marked INCOMPLETE header prepended when `incomplete_note` is set
/// (#431). Leads with an unmissable warning, then lists one omitted page per
/// line. Deterministic: the caller passes `note` in a stable order, so two
/// exports of the same incomplete project diff cleanly.
fn incomplete_header(note: &[String]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{COMMENT_PREFIX}!!! INCOMPLETE NETLIST — NOT A FULL WIRING REFERENCE !!!"
    );
    let _ = writeln!(
        out,
        "{COMMENT_PREFIX}This export does not cover the whole project. The user chose"
    );
    let _ = writeln!(
        out,
        "{COMMENT_PREFIX}\"Export anyway (incomplete)\". Components and nets from the pages"
    );
    let _ = writeln!(
        out,
        "{COMMENT_PREFIX}listed below are absent, and nets that merge through them may carry"
    );
    let _ = writeln!(
        out,
        "{COMMENT_PREFIX}the wrong name. Do not import as if it were complete."
    );
    let _ = writeln!(out, "{COMMENT_PREFIX}Omitted from this netlist:");
    for line in note {
        let _ = writeln!(out, "{COMMENT_PREFIX}  {line}");
    }
    out
}

/// A plain, deterministic net listing: one `net <id> "<name>"` line per net (in
/// the netlist's stable id order) followed by its `reference.pin` terminals (in
/// the order the derivation already sorted them). Not the Standard `.net`
/// format — that emitter is issue #62 — but a stable, contract-driven dump.
///
/// When `incomplete_note` is `Some`, an INCOMPLETE header comment block (see
/// [`incomplete_header`]) is prepended before the banner (#431); when `None`
/// the output is byte-identical to the plain dump.
fn render_listing(
    netlist: &signex_types::net::Netlist,
    incomplete_note: Option<&[String]>,
) -> Vec<u8> {
    let mut out = String::new();
    if let Some(note) = incomplete_note {
        out.push_str(&incomplete_header(note));
    }
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

    /// The `netlist` used by both #431 header tests below.
    fn one_net() -> Netlist {
        Netlist {
            nets: vec![Net {
                id: NetId(1),
                name: "GND".to_string(),
                class: None,
                wires: Vec::new(),
                junctions: Vec::new(),
                terminals: vec![Terminal {
                    symbol: Uuid::nil(),
                    reference: "R1".to_string(),
                    pin: "1".to_string(),
                }],
            }],
        }
    }

    #[test]
    fn incomplete_note_prepends_a_comment_header_naming_omitted_pages() {
        // #431: with `incomplete_note = Some([...])` the output must begin with
        // the comment-marked INCOMPLETE header naming the omitted pages, then
        // carry the normal deterministic listing unchanged after it.
        let opts = NetlistOptions {
            incomplete_note: Some(vec![
                "Netlist: page 'b.snxsch' is not in the netlist".to_string(),
            ]),
            ..NetlistOptions::default()
        };
        let out = NetlistExporter
            .export(&ctx_with(Some(one_net())), &opts)
            .expect("netlist present");
        let text = String::from_utf8(out.bytes).unwrap();

        assert!(
            text.starts_with("# "),
            "the INCOMPLETE header must be comment-marked: {text}"
        );
        assert!(
            text.contains("INCOMPLETE NETLIST"),
            "the header must announce incompleteness: {text}"
        );
        assert!(
            text.contains("b.snxsch"),
            "the header must name the omitted page: {text}"
        );
        // The normal listing follows the header, unchanged.
        assert!(
            text.contains("# Signex netlist\n# 1 nets\nnet 1 \"GND\"\n  R1.1\n"),
            "the deterministic listing must follow the header verbatim: {text}"
        );
        let header_at = text.find("INCOMPLETE NETLIST").unwrap();
        let banner_at = text.find("# Signex netlist").unwrap();
        assert!(
            header_at < banner_at,
            "the INCOMPLETE header must precede the listing banner"
        );
    }

    #[test]
    fn without_a_note_the_output_is_byte_identical_to_the_plain_dump() {
        // #431: the header is strictly opt-in. `None` must not change a byte.
        let with_none = NetlistExporter
            .export(&ctx_with(Some(one_net())), &NetlistOptions::default())
            .expect("netlist present");
        assert_eq!(
            String::from_utf8(with_none.bytes).unwrap(),
            "# Signex netlist\n# 1 nets\nnet 1 \"GND\"\n  R1.1\n"
        );
    }
}
