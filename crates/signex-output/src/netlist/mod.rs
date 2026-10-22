//! Netlist export.
//!
//! The exporter reads the authoritative [`Netlist`](signex_types::net::Netlist)
//! off [`ExportContext`] (ADR-0002 D7) — derived once by the app through
//! `signex_net::build_project_netlist` — so emitters never re-derive
//! connectivity. Two formats: the Protel `.NET` file for downstream PCB tools
//! (default, see [`protel`]) and a plain debug/diff listing.

mod protel;

use std::fmt::Write as _;

use thiserror::Error;

use crate::{ExportContext, Exporter};

pub struct NetlistExporter;

/// Output format of a netlist export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetlistFormat {
    /// Plain `# Signex netlist` text dump — a debug/diff aid, read by no PCB tool.
    Listing,
    /// Protel `.NET`: `[ ... ]` component blocks and `( ... )` net blocks.
    #[default]
    Protel,
}

impl NetlistFormat {
    /// Name shown in the save dialog's file-type filter.
    pub fn label(self) -> &'static str {
        match self {
            Self::Listing => "Signex Netlist Listing",
            Self::Protel => "Protel Netlist",
        }
    }

    /// File extension (without the dot) the format is saved under.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Listing => "txt",
            Self::Protel => "NET",
        }
    }

    /// Default filename offered by the save dialog.
    pub fn default_file_name(self) -> String {
        format!("schematic.{}", self.extension())
    }
}

#[derive(Debug, Clone, Default)]
pub struct NetlistOptions {
    /// Which emitter runs; defaults to [`NetlistFormat::Protel`].
    pub format: NetlistFormat,
    /// When `Some`, the [`NetlistFormat::Listing`] output is prefixed with an
    /// INCOMPLETE-netlist header comment block — one comment line per string,
    /// naming the pages this export does not cover (#431). The Protel format
    /// has no comment syntax, so it ignores this field and always writes a
    /// clean netlist; the app reports the omission through the Messages panel
    /// and the export dialog instead. `None` (the default) emits the listing
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
    #[error(
        "the Protel netlist cannot be written: designator(s) {0} are held by more than one part — run \"Reset Duplicate Schematic Designators...\" and export again"
    )]
    DuplicateDesignators(String),
    #[error(
        "the Protel netlist cannot be written: designator(s) {0} are unannotated — run \"Annotate Schematics...\" and export again"
    )]
    UnannotatedDesignators(String),
    #[error(
        "the Protel netlist cannot be written: {0} part(s) have a blank designator — run \"Annotate Schematics...\" (or give each part a designator) and export again"
    )]
    BlankDesignators(usize),
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
        let note = opts.incomplete_note.as_deref();
        let bytes = match opts.format {
            NetlistFormat::Listing => render_listing(netlist, note),
            NetlistFormat::Protel => protel::emit(ctx, netlist)?,
        };
        Ok(NetlistOutput { bytes })
    }
}

/// Comment-line marker for the listing and the INCOMPLETE header. Every line beginning with this
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
/// the order the derivation already sorted them). A stable, contract-driven
/// debug dump; not read by PCB tools (that is the Protel format).
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

    fn listing_opts() -> NetlistOptions {
        NetlistOptions {
            format: NetlistFormat::Listing,
            ..NetlistOptions::default()
        }
    }

    #[test]
    fn protel_is_the_default_format() {
        assert_eq!(NetlistOptions::default().format, NetlistFormat::Protel);
        assert_eq!(NetlistFormat::Protel.extension(), "NET");
        assert_eq!(NetlistFormat::Protel.default_file_name(), "schematic.NET");
    }

    fn ctx_with(netlist: Option<Netlist>) -> ExportContext {
        ExportContext {
            sheets: Vec::new(),
            metadata: Default::default(),
            netlist,
            netlist_sheets: Vec::new(),
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
            .export(&ctx_with(Some(netlist)), &listing_opts())
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
            ..listing_opts()
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
            .export(&ctx_with(Some(one_net())), &listing_opts())
            .expect("netlist present");
        assert_eq!(
            String::from_utf8(with_none.bytes).unwrap(),
            "# Signex netlist\n# 1 nets\nnet 1 \"GND\"\n  R1.1\n"
        );
    }
}
