//! The field names saved profiles carry for [`Modifiers`] and how the `Mod`
//! token resolves per platform. A renamed field silently drops a modifier
//! from every profile on disk, so the names are written out here.

use crate::{Modifiers, Platform};

const MOD: Modifiers = Modifiers {
    ctrl: false,
    alt: false,
    shift: false,
    command: false,
    primary: true,
};

#[test]
fn modifiers_keep_their_field_names() {
    let all = Modifiers {
        ctrl: true,
        alt: true,
        shift: true,
        command: true,
        primary: true,
    };
    assert_eq!(
        serde_json::to_value(all).unwrap(),
        serde_json::json!({
            "ctrl": true,
            "alt": true,
            "shift": true,
            "command": true,
            "primary": true,
        })
    );
}

#[test]
fn a_profile_without_primary_reads_it_as_false() {
    let back: Modifiers =
        serde_json::from_str(r#"{"ctrl": true, "alt": false, "shift": true, "command": false}"#)
            .unwrap();
    assert_eq!(
        back,
        Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::default()
        }
    );
}

#[test]
fn mod_resolves_to_command_on_macos_and_ctrl_elsewhere() {
    let command = Modifiers {
        command: true,
        ..Modifiers::default()
    };
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::default()
    };
    assert_eq!(MOD.resolved_for(Platform::MacOs), command);
    assert_eq!(MOD.resolved_for(Platform::Windows), ctrl);
    assert_eq!(MOD.resolved_for(Platform::Linux), ctrl);
    for platform in [Platform::MacOs, Platform::Windows, Platform::Linux] {
        let once = MOD.resolved_for(platform);
        assert_eq!(once.resolved_for(platform), once, "idempotent");
        assert_eq!(ctrl.resolved_for(platform), ctrl, "no Mod, no change");
    }
}

#[test]
fn a_command_press_records_as_mod_only_on_macos() {
    let command_shift = Modifiers {
        command: true,
        shift: true,
        ..Modifiers::default()
    };
    let mod_shift = Modifiers { shift: true, ..MOD };
    assert_eq!(command_shift.recorded_for(Platform::MacOs), mod_shift);
    assert_eq!(command_shift.recorded_for(Platform::Windows), command_shift);
    assert_eq!(command_shift.recorded_for(Platform::Linux), command_shift);
    assert_eq!(mod_shift.resolved_for(Platform::MacOs), command_shift);
}

#[test]
fn only_no_modifier_at_all_is_empty() {
    assert!(Modifiers::default().is_empty());
    assert!(!MOD.is_empty());
    for one in [
        Modifiers {
            ctrl: true,
            ..Modifiers::default()
        },
        Modifiers {
            alt: true,
            ..Modifiers::default()
        },
        Modifiers {
            shift: true,
            ..Modifiers::default()
        },
        Modifiers {
            command: true,
            ..Modifiers::default()
        },
    ] {
        assert!(!one.is_empty(), "{one:?}");
    }
}
