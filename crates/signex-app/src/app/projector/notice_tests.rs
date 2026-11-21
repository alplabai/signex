//! Each tail notice renders byte-identical to the line the tail logged
//! before the seam (#2326 slice 2, PR 2.2, test block B).

use signex_engine::{EngineError, EngineNotice};
use signex_i18n::I18n;

use super::notice::notice_line;
use crate::app::core::event::{Notice, NoticeLevel, locked_skips_line};

fn rendered(notice: &Notice) -> String {
    notice_line(&I18n::en_us(), notice)
}

#[test]
fn each_notice_renders_byte_identical_to_trunk() {
    let skips = locked_skips_line(&["R1".to_string(), "R2".to_string()]).unwrap();
    let refusal = crate::app::read_only_refusal::refusal_message(
        Some(std::path::Path::new("/w/power.snxsch")),
        "snxsch/1.7",
    );
    let error = format!(
        "Engine command failed: {:#}",
        anyhow::Error::new(EngineError::ReadOnly)
    );
    let table = [
        (
            Notice::legacy_english(NoticeLevel::Warning, skips),
            "Skipped 2 locked object(s): R1, R2; unlock in Properties to change them",
        ),
        (
            Notice::legacy_english(NoticeLevel::Warning, refusal),
            "Not changed: power.snxsch is read-only, written by a newer Signex (snxsch/1.7). \
             Edit it in that version.",
        ),
        (
            Notice::legacy_english(NoticeLevel::Error, error.clone()),
            error.as_str(),
        ),
    ];
    for (notice, line) in table {
        assert_eq!(rendered(&notice), line);
    }
    assert_eq!(locked_skips_line(&[]), None);
    let grip = EngineNotice::GripEndLeft {
        at: signex_types::schematic::Point::new(1.5, 2.0),
    };
    assert_eq!(
        rendered(&Notice::engine(grip.clone())),
        crate::app::engine_notices::notice_line(&I18n::en_us(), &grip)
    );
    assert_eq!(Notice::engine(grip).level, NoticeLevel::Warning);
}
