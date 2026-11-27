//! #1116: the asset report behind the Projects tree, the status bar count,
//! the missing-image ERC rule and `list_image_assets`.

use std::path::PathBuf;

use super::*;
use crate::image_asset::{ImageFacts, ImageFormat};

fn facts(hash: &str) -> ImageFacts {
    ImageFacts {
        format: ImageFormat::Svg,
        width_px: 1200,
        height_px: 584,
        bytes: 46_000,
        hash: hash.into(),
    }
}

fn scanned(file: &str, hash: Option<&str>) -> ScannedAsset {
    ScannedAsset {
        file: file.into(),
        facts: hash.map(facts),
    }
}

fn used(sheet: &str, file: &str, hash: &str, n: u128) -> ImageUse {
    ImageUse {
        sheet: PathBuf::from(sheet),
        image: uuid::Uuid::from_u128(n),
        file: file.into(),
        hash: hash.into(),
    }
}

#[test]
fn files_are_listed_with_their_uses_and_facts_in_name_order() {
    let report = asset_report(
        &[
            scanned("assets/images/b.svg", Some("bb")),
            scanned("assets/images/a.svg", Some("aa")),
        ],
        &[
            used("/p/power.snxsch", "assets/images/a.svg", "aa", 1),
            used("/p/power.snxsch", "assets/images/a.svg", "aa", 2),
            used("/p/top.snxsch", "assets/images/a.svg", "aa", 3),
        ],
        &[],
    );
    let files: Vec<&str> = report.entries.iter().map(|e| e.file.as_str()).collect();
    assert_eq!(files, ["assets/images/a.svg", "assets/images/b.svg"]);
    let a = report.entry("assets/images/a.svg").unwrap();
    assert_eq!(a.uses, 3);
    assert_eq!(a.sheets.len(), 2);
    assert_eq!(a.format.as_deref(), Some("svg"));
    assert_eq!(
        (a.width_px, a.height_px, a.bytes),
        (Some(1200), Some(584), Some(46_000))
    );
    assert!(!a.missing);
    assert_eq!(report.entry("assets/images/b.svg").unwrap().uses, 0);
    assert_eq!(report.missing_count(), 0);
}

#[test]
fn a_gone_or_changed_file_an_image_names_is_missing() {
    let report = asset_report(
        &[scanned("assets/images/changed.png", Some("new"))],
        &[
            used("/p/a.snxsch", "assets/images/gone.svg", "aa", 1),
            used("/p/a.snxsch", "assets/images/changed.png", "old", 2),
        ],
        &[],
    );
    assert!(report.is_missing("assets/images/gone.svg"));
    assert!(report.is_missing("assets/images/changed.png"));
    assert_eq!(report.missing_count(), 2);
}

#[test]
fn an_unused_unreadable_file_is_listed_but_not_missing() {
    let report = asset_report(&[scanned("assets/images/broken.png", None)], &[], &[]);
    let broken = report.entry("assets/images/broken.png").unwrap();
    assert_eq!(broken.format, None);
    assert!(!broken.missing);
}

#[test]
fn a_pick_waiting_for_its_save_is_pending_not_missing() {
    let report = asset_report(
        &[],
        &[used("/p/a.snxsch", "assets/images/new.png", "nn", 1)],
        &[("assets/images/new.png".into(), "nn".into())],
    );
    let new = report.entry("assets/images/new.png").unwrap();
    assert!(new.pending);
    assert!(!new.missing);
    assert_eq!(report.missing_count(), 0);
}
