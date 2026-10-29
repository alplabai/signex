//! The lines net navigation writes to the Messages panel.
//!
//! The panel keeps about 157 bytes of a line, so each one leads with what to
//! do and puts names after it, and every dynamic part is clipped so no name or
//! path can push the advice out. [`cap`] is the last guard.

use std::path::Path;

/// What the Messages panel shows of a line.
pub(super) const MAX_BYTES: usize = 157;

/// The longest name or sheet label copied into a line.
const NAME_CLIP: usize = 24;

/// `text` cut to `max` characters, with an ellipsis when it was longer.
fn clip(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_string(),
    }
}

/// `line` cut to [`MAX_BYTES`] at a character boundary.
fn cap(line: String) -> String {
    if line.len() <= MAX_BYTES {
        return line;
    }
    let end = (0..=MAX_BYTES)
        .rev()
        .find(|&i| line.is_char_boundary(i))
        .unwrap_or(0);
    line[..end].to_string()
}

/// A file's own name, never its folder.
fn file(path: &Path) -> String {
    clip(&path.file_name().unwrap_or_default().to_string_lossy(), 40)
}

pub(super) fn no_port_selected() -> String {
    "Select a label, sheet entry or power port first, then run Go to net partner.".to_string()
}

pub(super) fn no_project(what: &str) -> String {
    cap(format!(
        "Open the project (.snxprj) that owns this sheet, then use {what}."
    ))
}

pub(super) fn index_unavailable() -> String {
    "Fix the project's root sheet, then retry; net navigation could not read the project."
        .to_string()
}

pub(super) fn index_failed(reason: &str) -> String {
    cap(format!(
        "Fix the project's root sheet to use net navigation: {}",
        clip(reason, 80)
    ))
}

pub(super) fn sheet_not_in_project() -> String {
    "Add this sheet to the project, then retry; it is not in the project's hierarchy.".to_string()
}

pub(super) fn not_a_port(name: &str) -> String {
    cap(format!(
        "Select a global label, hierarchical label, sheet entry or power port; '{}' is not one.",
        clip(name, NAME_CLIP)
    ))
}

pub(super) fn no_partner(name: &str) -> String {
    cap(format!(
        "Add a matching global label or sheet entry; '{}' has no partner on another sheet.",
        clip(name, NAME_CLIP)
    ))
}

pub(super) fn partner_has_no_file(name: &str, sheet: &str) -> String {
    cap(format!(
        "Check the project's sheet list; the partner of '{}' is on '{}', which has no file.",
        clip(name, NAME_CLIP),
        clip(sheet, NAME_CLIP)
    ))
}

pub(super) fn jumped(name: &str, total: usize, sheet: &str) -> String {
    cap(format!(
        "Run Go to net partner again for the next; '{}' has {total} partner(s), now on '{}'.",
        clip(name, NAME_CLIP),
        clip(sheet, NAME_CLIP)
    ))
}

pub(super) fn cannot_open(path: &Path) -> String {
    cap(format!(
        "Check the file exists and is a valid sheet; could not open {}.",
        file(path)
    ))
}

pub(super) fn no_history(forward: bool) -> String {
    let edge = if forward { "later" } else { "earlier" };
    format!("Open or jump to another sheet first; the history has no {edge} sheet.")
}

pub(super) fn history_skipped(path: &Path) -> String {
    cap(format!(
        "Open the sheet from the project tree instead; history skipped {}, it is missing.",
        file(path)
    ))
}

pub(super) fn index_warning(warning: &str) -> String {
    cap(format!(
        "Check the project's sheets: {}",
        clip(warning, 110)
    ))
}

pub(super) fn project_closed() -> String {
    "Highlight a net again in an open project; its project was closed, so the highlight was cleared."
        .to_string()
}

pub(super) fn nothing_pointed_at() -> String {
    "Point at a wire, pin, label or sheet entry first, then highlight its net.".to_string()
}

pub(super) fn on_no_net() -> String {
    "Connect it to a wire or pin, then highlight again; that object is on no net.".to_string()
}

pub(super) fn highlighted(name: &str, sheets: usize) -> String {
    cap(format!(
        "Step through it or run Clear net highlighting; net '{}' is highlighted on {sheets} sheet(s).",
        clip(name, NAME_CLIP)
    ))
}

pub(super) fn nothing_highlighted() -> String {
    "Highlight a net first (net under cursor), then step through it; none is highlighted."
        .to_string()
}

pub(super) fn highlight_gone() -> String {
    "Highlight a net again; the highlighted net no longer exists, so it was cleared.".to_string()
}

pub(super) fn net_has_no_members() -> String {
    "Add a pin or label to the net, then step again; it has nothing to step to.".to_string()
}

pub(super) fn net_sheet_has_no_file(name: &str, sheet: &str) -> String {
    cap(format!(
        "Check the project's sheet list; net '{}' reaches '{}', which has no file.",
        clip(name, NAME_CLIP),
        clip(sheet, NAME_CLIP)
    ))
}

pub(super) fn stepped(
    name: &str,
    kind: &str,
    what: &str,
    sheet: &str,
    at: usize,
    total: usize,
) -> String {
    cap(format!(
        "Net '{}': {kind} {} on '{}', item {at} of {total}.",
        clip(name, NAME_CLIP),
        clip(what, NAME_CLIP),
        clip(sheet, NAME_CLIP)
    ))
}

pub(super) fn nothing_to_clear() -> String {
    "Nothing to clear; no net is highlighted.".to_string()
}

pub(super) fn cleared() -> String {
    "Net highlight cleared.".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A name, sheet and path far longer than the panel keeps.
    fn long() -> String {
        "X".repeat(300)
    }

    #[test]
    fn every_line_fits_the_panel_even_with_huge_names_and_paths() {
        let l = long();
        let path = std::path::PathBuf::from(format!("/a/b/{l}.snxsch"));
        let lines = [
            no_port_selected(),
            no_project("Go to net partner"),
            index_unavailable(),
            index_failed(&l),
            sheet_not_in_project(),
            not_a_port(&l),
            no_partner(&l),
            partner_has_no_file(&l, &l),
            jumped(&l, 99, &l),
            cannot_open(&path),
            no_history(true),
            no_history(false),
            history_skipped(&path),
            index_warning(&l),
            project_closed(),
            nothing_pointed_at(),
            on_no_net(),
            highlighted(&l, 12),
            nothing_highlighted(),
            highlight_gone(),
            net_has_no_members(),
            net_sheet_has_no_file(&l, &l),
            stepped(&l, "label", &l, &l, 10, 99),
            nothing_to_clear(),
            cleared(),
        ];
        for line in &lines {
            assert!(line.len() <= MAX_BYTES, "{} bytes: {line}", line.len());
        }
    }

    #[test]
    fn the_advice_comes_first_and_the_name_after_it() {
        let line = no_partner("VCC");
        assert!(line.starts_with("Add a matching"), "{line}");
        assert!(line.contains("'VCC'"));
    }

    #[test]
    fn cap_cuts_on_a_character_boundary() {
        let line = "é".repeat(200);
        let capped = cap(line);
        assert!(capped.len() <= MAX_BYTES);
        assert!(capped.chars().all(|c| c == 'é'));
    }
}
