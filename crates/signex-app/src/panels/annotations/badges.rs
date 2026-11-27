//! The Notebook badges of a scan (mockup row 197): the blocks of every
//! notebook file per part and net, in file order, each badge coloured by
//! the worst status of its blocks (`signex_notebook::badges`).

use super::{Badge, NotebookScan};

/// The badges of every block the scan read. A block's file is named as a
/// path inside `notebook/`.
pub(crate) fn notebook_badges(scan: &NotebookScan) -> Vec<Badge> {
    let names: Vec<String> = scan
        .files
        .iter()
        .map(|file| {
            file.path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
        .collect();
    signex_notebook::badges(
        scan.files
            .iter()
            .zip(&names)
            .map(|(file, name)| (name.as_str(), file.annotations.as_slice())),
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use signex_notebook::{BadgeTarget, BadgeTone, badge_of};

    use super::*;
    use crate::panels::annotations::NotebookFile;

    fn scan_of(text: &str) -> NotebookScan {
        let (annotations, problems) = signex_notebook::scan(text);
        assert!(problems.is_empty(), "{problems:?}");
        NotebookScan {
            folder_found: true,
            files: vec![NotebookFile {
                path: PathBuf::from("/p/notebook/bringup.snxnbk"),
                label: "notebook/bringup.snxnbk".into(),
                annotations,
                problems,
                read_error: None,
            }],
            ..NotebookScan::default()
        }
    }

    #[test]
    fn a_part_gets_one_badge_counting_the_blocks_that_name_it() {
        let scan = scan_of(
            "#annotation(kind: \"issue\", status: \"open\")[#component(\"U3\") runs warm.]\n\
             #annotation(kind: \"decision\")[Use the LDO for #component(\"U3\").]\n\
             #annotation(kind: \"issue\", status: \"resolved\")[#component(\"C6\") is missing near #component(\"U3\").]\n\
             #annotation(kind: \"note\")[#component(\"U1\") pin #pin(\"U1\", \"3\") on #net(\"VBUS\").]\n",
        );
        let badges = notebook_badges(&scan);
        let u3 = badge_of(&badges, &BadgeTarget::Part("U3".into())).expect("U3");
        assert_eq!(u3.count(), 3);
        assert_eq!(u3.tone, BadgeTone::OpenIssue, "the worst status");
        let c6 = badge_of(&badges, &BadgeTarget::Part("C6".into())).expect("C6");
        assert_eq!(c6.tone, BadgeTone::Notes, "a resolved issue is not amber");
        let u1 = badge_of(&badges, &BadgeTarget::Part("U1".into())).expect("U1");
        assert_eq!(u1.count(), 1, "a part and its pin in one block count once");
        assert!(
            badge_of(&badges, &BadgeTarget::Net("VBUS".into())).is_some(),
            "a net carries a badge"
        );
        assert_eq!(badges.len(), 4);
        assert_eq!(u3.blocks[0].file, "bringup.snxnbk");
    }

    #[test]
    fn no_annotations_no_badges() {
        assert!(notebook_badges(&NotebookScan::default()).is_empty());
    }
}
