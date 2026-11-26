//! Each tail notice renders byte-identical to the line the tail logged
//! before the seam (#2326 slice 2, PR 2.2, test block B), from a catalog id
//! (#2644): another language words it differently.

use std::path::Path;

use signex_engine::{EngineError, EngineNotice};
use signex_i18n::{I18n, LanguagePref, ids};
use signex_i18n_ids::Translatable;

use super::notice::notice_line;
use crate::app::core::event::{
    Notice, NoticeBody, NoticeLevel, locked_skips, read_only_refusal, read_only_refusal_message,
};

fn rendered(notice: &Notice) -> String {
    notice_line(&I18n::en_us(), notice)
}

fn pseudo() -> I18n {
    I18n::new(&LanguagePref::Locale("en-XA".into()))
}

/// `notice` has a catalog id, and the pseudo-locale renders it differently.
fn assert_translated(notice: &Notice) {
    if let NoticeBody::Text(text) = &notice.body {
        assert!(text.id().is_some(), "a catalog line: {text:?}");
    }
    assert_ne!(
        notice_line(&pseudo(), notice),
        rendered(notice),
        "{notice:?}"
    );
}

fn warning(text: Translatable) -> Notice {
    Notice::new(NoticeLevel::Warning, text)
}

#[test]
fn each_notice_renders_byte_identical_to_trunk() {
    let skips = locked_skips(&["R1".to_string(), "R2".to_string()]).unwrap();
    let refusal = read_only_refusal(Some(Path::new("/w/power.snxsch")), "snxsch/1.7");
    let error = format!("{:#}", anyhow::Error::new(EngineError::ReadOnly));
    let failed = Translatable::new(ids::NOTICE_EDIT_COMMAND_FAILED).arg("error", error.clone());
    let table = [
        (
            warning(skips),
            "Skipped 2 locked object(s): R1, R2; unlock in Properties to change them".to_string(),
        ),
        (
            warning(refusal),
            "Not changed: power.snxsch is read-only, written by a newer Signex (snxsch/1.7). \
             Edit it in that version."
                .to_string(),
        ),
        (
            Notice::new(NoticeLevel::Error, failed),
            format!("Engine command failed: {error}"),
        ),
    ];
    for (notice, line) in table {
        assert_eq!(rendered(&notice), line);
        assert_translated(&notice);
    }
    assert_eq!(locked_skips(&[]), None);
    let grip = EngineNotice::GripEndLeft {
        at: signex_types::schematic::Point::new(1.5, 2.0),
    };
    assert_eq!(
        rendered(&Notice::engine(grip.clone())),
        crate::app::engine_notices::notice_line(&I18n::en_us(), &grip)
    );
    assert_eq!(Notice::engine(grip).level, NoticeLevel::Warning);
}

/// The refusal notice and the refusal reason an edit returns read the same,
/// with a file name and without one.
#[test]
fn the_read_only_notice_reads_as_the_refusal_reason() {
    for path in [Some(Path::new("/w/power.snxsch")), None] {
        let notice = warning(read_only_refusal(path, "snxsch/1.7"));
        assert_eq!(
            rendered(&notice),
            read_only_refusal_message(path, "snxsch/1.7")
        );
        assert_translated(&notice);
    }
    assert_eq!(
        read_only_refusal_message(None, "snxsch/1.7"),
        "Not changed: This sheet is read-only, written by a newer Signex (snxsch/1.7). \
         Edit it in that version."
    );
}

/// The pins a "keep wires" move left as a plain move, worded as the engine
/// wrote them before they became a typed notice.
#[test]
fn the_pins_not_kept_line_reads_as_the_engine_wrote_it() {
    let pins = |n: usize| EngineNotice::PinsNotKept {
        pins: (1..=n).map(|i| format!("R{i} pin 1")).collect(),
    };
    let table = [
        (
            1,
            "could not keep R1 pin 1 connected without crossing another net",
        ),
        (
            3,
            "could not keep 3 pins connected without crossing another net: \
             R1 pin 1, R2 pin 1, R3 pin 1",
        ),
        (
            5,
            "could not keep 5 pins connected without crossing another net: \
             R1 pin 1, R2 pin 1, R3 pin 1 and 2 more",
        ),
    ];
    for (n, line) in table {
        let notice = Notice::engine(pins(n));
        assert_eq!(rendered(&notice), line);
        assert_translated(&notice);
    }
    // Counts are text, never grouped: the line the engine wrote.
    let many = rendered(&Notice::engine(pins(1_004)));
    assert!(many.starts_with("could not keep 1004 pins"), "{many}");
    assert!(many.ends_with("and 1001 more"), "{many}");
}
