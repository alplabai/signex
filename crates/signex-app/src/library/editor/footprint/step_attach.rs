//! STEP file attachment helper.
//!
//! Handles the file-pick → SHA-256 → copy-into-`step/<hash>.step` flow
//! for [`signex_library::StepAttachment`]. Also exposes the small
//! `view()` widget for the Footprint tab's Body 3D pane.
//!
//! Per `v0.9-library-refactor-plan.md` §11 step F5: the attachment is
//! content-hashed so two MPNs sharing identical STEP geometry
//! de-duplicate to one file in `mylib.snxlib/step/`.

use std::path::Path;

use iced::widget::{Space, button, column, container, row, text};
use iced::{Border, Element, Length, Theme};
use sha2::{Digest, Sha256};
use signex_library::{Footprint, StepAttachment};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::library::messages::{EditorMsg, LibraryMessage};

/// Render the STEP attachment row in the Body 3D pane.
pub fn view<'a>(
    fp: &'a Footprint,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);

    let attach_btn = button(
        container(text("Attach STEP…").size(11).color(iced::Color::WHITE)).padding([4, 12]),
    )
    .on_press(LibraryMessage::EditorEvent {
        window_id,
        msg: EditorMsg::StepAttachDialog,
    })
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgb(
            0.00, 0.47, 0.84,
        ))),
        text_color: iced::Color::WHITE,
        border: Border {
            width: 0.0,
            radius: 3.0.into(),
            ..Border::default()
        },
        ..iced::widget::button::Style::default()
    });

    let body: Element<'a, LibraryMessage> = match fp.step_attachment.as_ref() {
        Some(att) => {
            let remove_btn =
                button(container(text("Remove").size(10).color(text_c)).padding([3, 8]))
                    .on_press(LibraryMessage::EditorEvent {
                        window_id,
                        msg: EditorMsg::StepAttachRemove,
                    })
                    .style(move |_: &Theme, _| iced::widget::button::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgba(
                            1.0, 1.0, 1.0, 0.04,
                        ))),
                        text_color: text_c,
                        border: Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: border,
                        },
                        ..iced::widget::button::Style::default()
                    });
            column![
                row![
                    text("STEP attachment").size(13).color(text_c),
                    Space::new().width(Length::Fill),
                    remove_btn,
                ]
                .align_y(iced::Alignment::Center),
                Space::new().height(4),
                text(format!("File: {}", att.filename)).size(11).color(text_c),
                text(format!(
                    "SHA-256: {}",
                    &att.content_hash[..16.min(att.content_hash.len())]
                ))
                .size(10)
                .color(muted),
            ]
            .spacing(0)
            .into()
        }
        None => column![
            row![
                text("STEP attachment").size(13).color(text_c),
                Space::new().width(Length::Fill),
                attach_btn,
            ]
            .align_y(iced::Alignment::Center),
            Space::new().height(4),
            text("Attach a .step / .stp mech-CAD file. Stored content-hashed in step/.")
                .size(10)
                .color(muted),
        ]
        .spacing(0)
        .into(),
    };

    container(body)
        .padding(12)
        .style(crate::styles::modal_card(tokens))
        .width(Length::Fill)
        .into()
}

/// SHA-256 hex of `bytes` — same routine the 3D upload flow uses, but
/// kept private here so this module is self-contained.
fn hash_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in result.iter() {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

/// Stash a freshly-uploaded STEP file under `<lib_root>/step/<hash>.step`
/// and return the [`StepAttachment`] record to drop on the
/// `Footprint::step_attachment` field. Returns `None` when the IO
/// path fails (existing file collisions are NOT errors — content-hash
/// makes them no-ops by design).
pub fn stash_step(lib_root: &Path, bytes: &[u8], filename: &str) -> Option<StepAttachment> {
    let hash = hash_hex(bytes);
    let step_dir = lib_root.join("step");
    if let Err(e) = std::fs::create_dir_all(&step_dir) {
        tracing::warn!(
            target: "signex::library",
            error = %e,
            path = %step_dir.display(),
            "failed to create step dir; STEP attach will be in-memory only"
        );
        return None;
    }
    let target = step_dir.join(format!("{hash}.step"));
    if !target.exists()
        && let Err(e) = std::fs::write(&target, bytes)
    {
        tracing::warn!(
            target: "signex::library",
            error = %e,
            path = %target.display(),
            "failed to write STEP file"
        );
        return None;
    }
    Some(StepAttachment {
        content_hash: hash,
        filename: filename.to_string(),
        offset_xyz: [0.0; 3],
        rotation_xyz: [0.0; 3],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_hex_is_lowercase() {
        let h = hash_hex(b"abc");
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn stash_step_writes_file_then_skips_on_dup() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let lib_root = tmp.path();
        let att = stash_step(lib_root, b"hello", "X.step").expect("first stash");
        let target = lib_root.join("step").join(format!("{}.step", att.content_hash));
        assert!(target.exists());
        // Second call with same bytes should be a no-op (file
        // already exists; we don't overwrite).
        let _ = stash_step(lib_root, b"hello", "Y.step").expect("dup stash");
        // File still has the original content.
        let bytes = std::fs::read(&target).expect("read");
        assert_eq!(bytes, b"hello");
    }
}
