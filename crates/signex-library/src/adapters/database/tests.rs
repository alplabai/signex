use super::*;

#[test]
fn token_over_cleartext_non_loopback_is_refused() {
    assert!(DatabaseAdapter::with_token("http://lib.example.com", "t", "h").is_err());
    assert!(DatabaseAdapter::with_token("http://10.0.0.5:3535", "t", "h").is_err());
    assert!(DatabaseAdapter::with_token("https://lib.example.com", "t", "h").is_ok());
    assert!(DatabaseAdapter::with_token("http://127.0.0.1:3535", "t", "h").is_ok());
    assert!(DatabaseAdapter::with_token("http://localhost:3535", "t", "h").is_ok());
    assert!(DatabaseAdapter::with_token("http://[::1]:3535", "t", "h").is_ok());
    // No credential, nothing to leak.
    assert!(DatabaseAdapter::with_token("http://lib.example.com", "", "h").is_ok());
}

#[test]
fn keychain_sigil_is_rejected_with_the_sigil_error() {
    let manifest = Manifest {
        library: LibraryMeta {
            name: "remote".into(),
            library_id: Uuid::nil(),
            description: None,
        },
        mode: LibraryMode::Database {
            url: "https://lib.example.com".into(),
            auth: "@signex-keychain:x".into(),
        },
        workflow: Default::default(),
        users: Default::default(),
        tables: Vec::new(),
    };
    let err = DatabaseAdapter::new(manifest)
        .err()
        .expect("sigil must be refused");
    assert!(err.to_string().contains("not resolved"), "{err}");
}

#[test]
fn redirect_downgrade_to_cleartext_is_refused() {
    let u = |s: &str| url::Url::parse(s).unwrap();
    assert!(!redirect_hop_allowed(
        &u("http://lib.example.com:8443/x"),
        true
    ));
    assert!(redirect_hop_allowed(
        &u("https://lib.example.com:8443/x"),
        true
    ));
    assert!(redirect_hop_allowed(&u("http://127.0.0.1:3535/x"), true));
}

#[test]
fn tokenless_client_may_follow_same_origin_cleartext_redirect() {
    let u = url::Url::parse("http://lib.example.com/tables/").unwrap();
    assert!(redirect_hop_allowed(&u, false));
    assert!(!redirect_hop_allowed(&u, true));
}

#[test]
fn refusal_error_does_not_leak_userinfo() {
    let err = DatabaseAdapter::with_token("http://alice:hunter2@lib.example.com:8080", "t", "h")
        .err()
        .expect("must refuse");
    let msg = err.to_string();
    assert!(!msg.contains("hunter2") && !msg.contains("alice"), "{msg}");
    assert!(msg.contains("http://lib.example.com:8080"), "{msg}");
}

#[test]
fn keychain_sigil_reported_before_transport_error() {
    let manifest = Manifest {
        library: LibraryMeta {
            name: "remote".into(),
            library_id: Uuid::nil(),
            description: None,
        },
        mode: LibraryMode::Database {
            url: "http://lib.example.com".into(),
            auth: "@signex-keychain:x".into(),
        },
        workflow: Default::default(),
        users: Default::default(),
        tables: Vec::new(),
    };
    let err = DatabaseAdapter::new(manifest).err().expect("refused");
    assert!(err.to_string().contains("not resolved"), "{err}");
}

use crate::library_file::{FORMAT_TOKEN, LibrarySection};
use crate::manifest::{UsersConfig, WorkflowConfig};

/// `from_snxlib` mirrors `LocalGitAdapter::init`'s manifest API
/// shape. The DB adapter requires `LibraryMode::Database`; passing
/// the default `LocalGit` mode must fail loudly so a misconfigured
/// project doesn't silently bring up a remote-shaped adapter
/// pointed at nothing.
#[test]
fn from_snxlib_round_trips_database_mode() {
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::nil(),
        library: LibrarySection {
            name: "Remote".into(),
            description: None,
        },
        mode: LibraryMode::Database {
            url: "https://example.com/api".into(),
            auth: "remote-token".into(),
        },
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let adapter = DatabaseAdapter::from_snxlib(manifest).unwrap();
    assert_eq!(adapter.base_url(), "https://example.com/api");
    assert_eq!(adapter.manifest().library.name, "Remote");
}

#[test]
fn from_snxlib_rejects_non_database_mode() {
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::nil(),
        library: LibrarySection {
            name: "OopsLocalGit".into(),
            description: None,
        },
        mode: LibraryMode::default(), // LocalGit — wrong for DB adapter
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    match DatabaseAdapter::from_snxlib(manifest) {
        Err(LibraryError::Backend(_)) => {}
        Err(other) => panic!("expected Backend error, got {other:?}"),
        Ok(_) => panic!("non-database mode should have been rejected"),
    }
}

#[test]
fn with_token_round_trips_url() {
    let adapter =
        DatabaseAdapter::with_token("https://example.com/api/", "secret-bearer", "alice@example")
            .unwrap();
    assert_eq!(adapter.base_url(), "https://example.com/api");
}

#[test]
fn encode_segment_passes_through_unreserved() {
    assert_eq!(
        DatabaseAdapter::encode_segment("Discrete-Passives_v2.0~final"),
        "Discrete-Passives_v2.0~final"
    );
}

#[test]
fn encode_segment_escapes_path_breakers() {
    // Spaces, slashes, query separators, anchors — every byte that
    // could reshape the URL has to come back percent-encoded.
    assert_eq!(
        DatabaseAdapter::encode_segment("Discrete Passives"),
        "Discrete%20Passives"
    );
    assert_eq!(
        DatabaseAdapter::encode_segment("resistors?evil"),
        "resistors%3Fevil"
    );
    assert_eq!(DatabaseAdapter::encode_segment("a/b#c"), "a%2Fb%23c");
}

#[test]
fn encode_segment_handles_utf8() {
    // Multi-byte UTF-8 must escape every byte.
    assert_eq!(DatabaseAdapter::encode_segment("π"), "%CF%80");
}

#[test]
fn pn_uniqueness_is_trimmed_case_sensitive_and_blank_exempt() {
    use super::pn_unique::ensure_pn_free;
    let row = |pn: &str| {
        let mut r = crate::component::tests::fixture_row();
        r.internal_pn = InternalPn::new(pn);
        r
    };
    let a = row("PN-A");
    let rows = vec![("t".to_string(), a.clone())];
    assert!(ensure_pn_free(&rows, "PN-A", None).is_err());
    assert!(ensure_pn_free(&rows, " PN-A ", None).is_err());
    assert!(ensure_pn_free(&rows, "pn-a", None).is_ok());
    assert!(ensure_pn_free(&rows, "  ", None).is_ok());
    assert!(ensure_pn_free(&rows, "PN-A", Some(a.row_id)).is_ok());
}
