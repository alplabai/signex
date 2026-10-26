use super::*;

const BODY: &str = "*.tmp\n.DS_Store\n";
const BLOCK: &str = "# >>> signex (managed) >>>\n*.tmp\n.DS_Store\n# <<< signex (managed) <<<\n";

#[test]
fn no_file_yields_just_the_block() {
    assert_eq!(merge_managed_block(None, BODY).unwrap(), BLOCK);
    assert_eq!(merge_managed_block(Some(""), BODY).unwrap(), BLOCK);
}

#[test]
fn an_old_block_is_replaced_and_user_lines_stay_byte_identical() {
    let before = "head\r\nline\n";
    let after = "\ntail without newline";
    let old = "# >>> signex (managed) >>>\nold-pattern\n# <<< signex (managed) <<<\n";
    let existing = format!("{before}{old}{after}");
    let once = merge_managed_block(Some(&existing), BODY).unwrap();
    assert_eq!(once, format!("{before}{BLOCK}{after}"));
    let twice = merge_managed_block(Some(&once), BODY).unwrap();
    assert_eq!(twice, once, "a second merge must be a no-op");
}

#[test]
fn a_block_at_end_of_file_without_newline_stays_without_one() {
    let existing = "a\n# >>> signex (managed) >>>\nx\n# <<< signex (managed) <<<";
    let out = merge_managed_block(Some(existing), BODY).unwrap();
    assert!(out.ends_with("# <<< signex (managed) <<<"));
    assert_eq!(merge_managed_block(Some(&out), BODY).unwrap(), out);
}

#[test]
fn a_crlf_block_keeps_crlf_and_is_idempotent() {
    let existing = "# >>> signex (managed) >>>\r\nold\r\n# <<< signex (managed) <<<\r\n";
    let out = merge_managed_block(Some(existing), BODY).unwrap();
    assert_eq!(
        out,
        "# >>> signex (managed) >>>\r\n*.tmp\r\n.DS_Store\r\n# <<< signex (managed) <<<\r\n"
    );
    assert_eq!(merge_managed_block(Some(&out), BODY).unwrap(), out);
}

#[test]
fn duplicate_or_unbalanced_markers_are_an_error() {
    let dup = format!("{BLOCK}{BLOCK}");
    assert_eq!(
        merge_managed_block(Some(&dup), BODY),
        Err(ManagedBlockError::DuplicateMarkers { begins: 2, ends: 2 })
    );
    for broken in [
        "# >>> signex (managed) >>>\nx\n",
        "x\n# <<< signex (managed) <<<\n",
        "# <<< signex (managed) <<<\n# >>> signex (managed) >>>\n",
    ] {
        assert_eq!(
            merge_managed_block(Some(broken), BODY),
            Err(ManagedBlockError::Unbalanced),
            "{broken:?}"
        );
    }
}

#[test]
fn append_missing_lines_skips_present_patterns_and_is_idempotent() {
    let rows = "# Generated\nlibs/\nbuild.snxpcb\n";
    let once = append_missing_lines("keep\nlibs/\n", rows);
    assert_eq!(once, "keep\nlibs/\n# Generated\nbuild.snxpcb\n");
    assert_eq!(append_missing_lines(&once, rows), once);
}

#[test]
fn a_leading_bom_is_ignored_when_matching_and_kept_in_the_output() {
    let existing = "\u{feff}# >>> signex (managed) >>>\nold\n# <<< signex (managed) <<<\nuser\n";
    let out = merge_managed_block(Some(existing), BODY).unwrap();
    assert_eq!(out, format!("\u{feff}{BLOCK}user\n"));
    assert_eq!(merge_managed_block(Some(&out), BODY).unwrap(), out);

    // A BOM on a user line is just user bytes: the block goes in after it.
    let out = merge_managed_block(Some("\u{feff}target/\n"), BODY).unwrap();
    assert_eq!(out, format!("\u{feff}{BLOCK}\ntarget/\n"));
}

#[test]
fn appending_rows_to_a_crlf_file_uses_crlf() {
    let out = append_missing_lines("keep\r\n", "# Generated\nlibs/\n");
    assert_eq!(out, "keep\r\n# Generated\r\nlibs/\r\n");
}

#[test]
fn the_block_goes_first_then_one_blank_line_then_every_user_byte() {
    let out = merge_managed_block(Some("*.pdf diff=pdf\n"), BODY).unwrap();
    assert_eq!(out, format!("{BLOCK}\n*.pdf diff=pdf\n"));
}

#[test]
fn the_bom_stays_first_and_the_files_crlf_style_is_kept() {
    let out = merge_managed_block(Some("\u{feff}a\r\nb"), BODY).unwrap();
    let block = BLOCK.replace('\n', "\r\n");
    assert_eq!(out, format!("\u{feff}{block}\r\na\r\nb"));
    assert_eq!(merge_managed_block(Some(&out), BODY).unwrap(), out);
}

#[test]
fn empty_text_yields_just_the_block() {
    assert_eq!(merge_managed_block(None, BODY).unwrap(), BLOCK);
    assert_eq!(
        merge_managed_block(Some("\u{feff}"), BODY).unwrap(),
        format!("\u{feff}{BLOCK}")
    );
}

#[test]
fn an_existing_block_is_replaced_in_place() {
    let existing = format!("a\n{BLOCK}b\n");
    let out = merge_managed_block(Some(&existing), "*.new\n").unwrap();
    assert_eq!(
        out,
        "a\n# >>> signex (managed) >>>\n*.new\n# <<< signex (managed) <<<\nb\n"
    );
}

#[test]
fn a_marker_with_trailing_whitespace_is_still_the_marker() {
    let existing = format!("mine\n{BLOCK_WITH_STALE_BODY_AND_SPACES}tail\n");
    let out = merge_managed_block(Some(&existing), BODY).unwrap();
    assert_eq!(
        out.matches("# >>> signex (managed) >>>").count(),
        1,
        "{out}"
    );
    assert_eq!(out, format!("mine\n{BLOCK}tail\n"));
}

const BLOCK_WITH_STALE_BODY_AND_SPACES: &str =
    "# >>> signex (managed) >>> \t\nstale\n# <<< signex (managed) <<< \n";
