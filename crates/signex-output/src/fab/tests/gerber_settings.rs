//! An Export Job's Gerber settings (#1299). The default settings write the
//! golden files byte for byte, so no job written before the settings
//! changes what a board house receives; each other setting changes only
//! what it names.

use signex_types::output_job::{
    GerberApertures, GerberFormat, GerberSettings, GerberUnits, GerberZeros,
};

use super::super::{FabExtras, FabFileKind, FabOptions, FabSet, generate};
use super::fixture::fixture_board;
use super::{file, golden_path};

fn with(settings: GerberSettings) -> FabSet {
    let options = FabOptions {
        gerber_settings: settings,
        ..FabOptions::all("fab")
    };
    generate(&fixture_board(), &FabExtras::default(), &options).expect("generates")
}

fn gerbers(set: &FabSet) -> Vec<(&str, &str)> {
    set.files
        .iter()
        .filter(|f| matches!(f.kind, FabFileKind::Gerber { .. }))
        .map(|f| (f.name.as_str(), f.contents.as_str()))
        .collect()
}

/// Every setting spelled out at its default value.
fn spelled_out_defaults() -> GerberSettings {
    GerberSettings {
        units: GerberUnits::Mm,
        format: GerberFormat::parse("4:6").expect("4:6"),
        zeros: GerberZeros::Leading,
        layers: None,
        apertures: GerberApertures::Standard,
    }
}

/// The hard rule of the settings: the defaults are today's bytes. The
/// golden files are read, never written, here.
#[test]
fn the_default_settings_write_the_golden_gerber_bytes() {
    for settings in [GerberSettings::default(), spelled_out_defaults()] {
        let set = with(settings);
        for name in [
            "fab-copper-l1-top.gbr",
            "fab-soldermask-top.gbr",
            "fab-profile.gbr",
        ] {
            let golden = std::fs::read_to_string(golden_path(name)).expect("golden file");
            assert_eq!(file(&set, name), Some(golden.as_str()), "{name}");
        }
    }
    let plain = generate(
        &fixture_board(),
        &FabExtras::default(),
        &FabOptions::all("fab"),
    )
    .expect("generates");
    assert_eq!(
        with(spelled_out_defaults()),
        plain,
        "every file, the job too"
    );
}

#[test]
fn inch_coordinates_and_apertures_are_converted() {
    let set = with(GerberSettings {
        units: GerberUnits::Inch,
        format: GerberFormat::parse("2:4").expect("2:4"),
        ..GerberSettings::default()
    });
    let copper = file(&set, "fab-copper-l1-top.gbr").expect("copper");
    assert!(copper.contains("%FSLAX24Y24*%\n%MOIN*%\n"), "{copper}");
    // The 1 mm x 1.2 mm pad, in inches to the millionth.
    assert!(copper.contains("%ADD10R,0.03937X0.047244*%"), "{copper}");
    // (4 mm, -5 mm) is (0.1575 in, -0.1969 in) to four decimals.
    assert!(copper.contains("X1575Y-1969D03*"), "{copper}");
    let profile = file(&set, "fab-profile.gbr").expect("profile");
    // 20 mm is 0.7874 in.
    assert!(profile.contains("X7874Y0D01*"), "{profile}");
}

#[test]
fn the_zeros_setting_decides_which_digits_are_written() {
    let trailing = with(GerberSettings {
        zeros: GerberZeros::Trailing,
        ..GerberSettings::default()
    });
    let copper = file(&trailing, "fab-copper-l1-top.gbr").expect("copper");
    assert!(copper.contains("%FSTAX46Y46*%"), "{copper}");
    // 4 mm in 4:6 is 0004000000; its trailing zeros go.
    assert!(copper.contains("X0004Y-0005D03*"), "{copper}");

    let none = with(GerberSettings {
        zeros: GerberZeros::None,
        ..GerberSettings::default()
    });
    let copper = file(&none, "fab-copper-l1-top.gbr").expect("copper");
    assert!(copper.contains("%FSLAX46Y46*%"), "{copper}");
    assert!(copper.contains("X0004000000Y-0005000000D03*"), "{copper}");
    let profile = file(&none, "fab-profile.gbr").expect("profile");
    assert!(profile.contains("X0000000000Y0000000000D02*"), "{profile}");
}

#[test]
fn a_layer_list_writes_only_its_images() {
    let set = with(GerberSettings {
        layers: Some(vec!["copper-top".into(), "profile".into()]),
        ..GerberSettings::default()
    });
    let names: Vec<&str> = gerbers(&set).into_iter().map(|(name, _)| name).collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names.contains(&"fab-copper-l1-top.gbr"), "{names:?}");
    assert!(names.contains(&"fab-profile.gbr"), "{names:?}");
    let job = file(&set, "fab-job.gbrjob").expect("job file");
    assert!(
        !job.contains("soldermask-top"),
        "the job lists what is written"
    );
    assert!(
        file(&set, "fab-pth.drl").is_some(),
        "drill files are not images"
    );

    let copper = with(GerberSettings {
        layers: Some(vec!["copper".into()]),
        ..GerberSettings::default()
    });
    assert!(
        gerbers(&copper)
            .iter()
            .all(|(name, _)| name.starts_with("fab-copper-")),
        "copper names every copper layer and nothing else"
    );
}

#[test]
fn the_macro_form_defines_every_aperture_as_a_macro() {
    let set = with(GerberSettings {
        apertures: GerberApertures::Macros,
        ..GerberSettings::default()
    });
    let copper = file(&set, "fab-copper-l1-top.gbr").expect("copper");
    assert!(
        copper.contains("%AMM10*\n21,1,1,1.2,0,0,0*\n%\n"),
        "the rectangle pad: {copper}"
    );
    assert!(copper.contains("%ADD10M10*%"), "{copper}");
    assert!(
        copper.contains("%AMM11*\n1,1,1.6,0,0,0*\n%\n"),
        "the round pad: {copper}"
    );
    assert!(!copper.contains("%ADD11C,"), "no standard aperture is left");
    // The image itself is the same: only the aperture definitions differ.
    let body = |text: &str| text[text.find("G75*").expect("body")..].to_string();
    let golden = std::fs::read_to_string(golden_path("fab-copper-l1-top.gbr")).expect("golden");
    assert_eq!(body(copper), body(&golden));
}
