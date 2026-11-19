//! The shared format-version policy (#2207).

use super::*;

const PLAIN: FormatVersion = FormatVersion {
    current: "snxdoc/1.2",
    readable: &["snxdoc/1", "snxdoc/1.1", "snxdoc/1.2"],
    newer_minor: NewerMinor::Refused,
    missing: MissingToken::Refused,
};

const VIEWABLE: FormatVersion = FormatVersion {
    newer_minor: NewerMinor::ReadOnly,
    missing: MissingToken::ReadAs("snxdoc/1"),
    ..PLAIN
};

const V_SPELLED: FormatVersion = FormatVersion {
    current: "snxdoc/v3",
    readable: &["snxdoc/v1", "snxdoc/v3"],
    newer_minor: NewerMinor::Refused,
    missing: MissingToken::Refused,
};

#[test]
fn every_listed_token_reads_as_itself() {
    for token in PLAIN.readable {
        assert_eq!(PLAIN.read(Some(*token)).as_deref(), Ok(*token));
    }
    // `1.0` is the same version as `1`.
    assert_eq!(PLAIN.read(Some("snxdoc/1.0")).as_deref(), Ok("snxdoc/1.0"));
    assert_eq!(
        V_SPELLED.read(Some("snxdoc/v1")).as_deref(),
        Ok("snxdoc/v1")
    );
}

#[test]
fn a_newer_token_is_refused_by_name() {
    for newer in ["snxdoc/1.3", "snxdoc/2", "snxdoc/2.0"] {
        let err = PLAIN.read(Some(newer)).unwrap_err();
        assert!(
            matches!(err, FormatVersionError::Newer { .. }),
            "{newer}: {err:?}"
        );
        let message = err.to_string();
        assert!(
            message.contains(newer) && message.contains("newer Signex"),
            "{message}"
        );
        assert!(message.contains("snxdoc/1.2"), "{message}");
        assert_eq!(err.found(), Some(newer));
    }
    let err = V_SPELLED.read(Some("snxdoc/v4")).unwrap_err();
    assert!(matches!(err, FormatVersionError::Newer { .. }), "{err:?}");
}

#[test]
fn a_newer_minor_opens_read_only_only_where_the_format_allows_it() {
    assert_eq!(
        VIEWABLE.read(Some("snxdoc/1.7")).as_deref(),
        Ok("snxdoc/1.7")
    );
    assert!(VIEWABLE.is_newer_minor("snxdoc/1.7"));
    let err = VIEWABLE.refuse_save("snxdoc/1.7").unwrap_err();
    assert!(err.to_string().contains("snxdoc/1.7"), "{err}");
    // A newer major is refused even where a newer minor opens.
    assert!(VIEWABLE.read(Some("snxdoc/2")).is_err());
    // Listed tokens save.
    for token in VIEWABLE.readable {
        assert!(!VIEWABLE.is_newer_minor(token));
        assert!(VIEWABLE.refuse_save(token).is_ok());
    }
}

#[test]
fn an_unlisted_older_or_foreign_or_garbled_token_is_refused() {
    for bad in [
        "snxdoc/0.9",
        "snxother/1",
        "snxdoc/x",
        "snxdoc/01",
        "snxdoc/1.02",
        "snxdoc/1.",
        "snxdoc/+1",
        "snxdoc/v1",
        "",
    ] {
        let err = PLAIN.read(Some(bad)).unwrap_err();
        assert!(
            matches!(err, FormatVersionError::Unsupported { .. }),
            "{bad:?}: {err:?}"
        );
        assert!(err.to_string().contains("snxdoc/1.2"), "{err}");
    }
    // `v2` is between two listed tokens but not one of them.
    assert!(matches!(
        V_SPELLED.read(Some("snxdoc/v2")),
        Err(FormatVersionError::Unsupported { .. })
    ));
}

#[test]
fn a_missing_token_follows_the_format_policy() {
    let err = PLAIN.read(None).unwrap_err();
    assert!(matches!(err, FormatVersionError::Missing { .. }), "{err:?}");
    assert_eq!(err.found(), None);
    assert_eq!(VIEWABLE.read(None).as_deref(), Ok("snxdoc/1"));
}
