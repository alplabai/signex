//! #2060: the BOM and netlist save dialogs name their file-type filter from
//! the catalog, with the English they showed before.

use signex_i18n::{I18n, LanguagePref};
use signex_output::{BomFormat, NetlistFormat};

use super::bom::bom_format_filter;
use super::pdf_netlist::netlist_filter;

fn xa() -> I18n {
    I18n::new(&LanguagePref::Locale("en-XA".to_owned()))
}

#[test]
fn bom_filters_keep_their_english() {
    let en = I18n::en_us();
    for (format, ext, english, exts) in [
        (BomFormat::Csv, "csv", "CSV (.csv)", &["csv"][..]),
        (BomFormat::Xlsx, "xlsx", "Excel (.xlsx)", &["xlsx"][..]),
        (
            BomFormat::Html,
            "html",
            "HTML (.html)",
            &["html", "htm"][..],
        ),
    ] {
        let (shown_ext, (id, shown_exts)) = bom_format_filter(format);
        assert_eq!(shown_ext, ext);
        assert_eq!(shown_exts, exts);
        assert_eq!(en.message_plain(id, &[]), english);
        assert_ne!(xa().message_plain(id, &[]), english);
    }
}

/// Each netlist filter reads as `NetlistFormat::label`, which the file
/// writer keeps.
#[test]
fn netlist_filters_keep_their_english() {
    let en = I18n::en_us();
    for format in [NetlistFormat::Listing, NetlistFormat::NetText] {
        let id = netlist_filter(format);
        assert_eq!(en.message_plain(id, &[]), format.label());
        assert_ne!(xa().message_plain(id, &[]), format.label());
    }
}
