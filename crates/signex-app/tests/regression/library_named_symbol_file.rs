//! #1841 — a symbol file is named by its author (`GATEDRV8.snxsym`), not by
//! the symbol's uuid. The picker's Browse and Edit Symbol find it through
//! the library's own listing.

use signex_library::PrimitiveRef;

use crate::library_place_symbol::{app_with_library, fixture};

#[test]
fn the_file_of_a_symbol_in_a_named_container_is_found_by_uuid() {
    let f = fixture();
    let app = app_with_library(&f);
    let named = f
        .snxlib
        .parent()
        .unwrap()
        .join("symbols")
        .join("two-unit.snxsym");

    let found = app.library.primitive_file(
        &PrimitiveRef::new(f.library_id, f.symbol_id),
        "symbols",
        "snxsym",
    );

    assert_eq!(found.as_deref(), Some(named.as_path()));
    assert!(named.is_file());
}

#[test]
fn browse_resolves_a_named_symbol_file_to_its_symbol() {
    let f = fixture();
    let app = app_with_library(&f);
    let named = f
        .snxlib
        .parent()
        .unwrap()
        .join("symbols")
        .join("two-unit.snxsym");

    let picked = app.library.primitive_uuid_in_file(f.library_id, &named);

    assert_eq!(picked, Ok(f.symbol_id));
}

#[test]
fn browse_refuses_a_file_the_library_does_not_list_and_says_why() {
    let f = fixture();
    let app = app_with_library(&f);
    let stray = f
        .snxlib
        .parent()
        .unwrap()
        .join("symbols")
        .join("stray.snxsym");

    let picked = app.library.primitive_uuid_in_file(f.library_id, &stray);

    assert!(picked.expect_err("refused").contains("holds no primitive"));
}
