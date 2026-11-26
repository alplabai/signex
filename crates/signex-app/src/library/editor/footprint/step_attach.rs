//! STEP file attachment helper.
//!
//! Handles the file-pick → embed-in-the-footprint-file flow for
//! [`signex_library::StepAttachment`] (#3107): the model lands in the
//! `.snxfpt`'s `[[models]]` section, keyed by its SHA-256, so footprints of
//! one file sharing a body store it once. Nothing is written under `step/`.
//! The editor's attach flow (`EditorMsg::StepAttachResult`) is not wired to
//! [`attach_step`] yet.
//! Also exposes the small `view()` widget for the Footprint tab's Body 3D
//! pane.

use iced::widget::{Space, Widget, column, container, row};
use iced::{Element, Length};
use signex_library::{Footprint, FootprintFile, StepAttachment};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text};
use uuid::Uuid;

use crate::styles::{Spacing, TypeScale};
use signex_widgets::theme_ext;

use crate::library::messages::{EditorMsg, LibraryMessage};
use crate::library::state::EditorAddress;

/// Render the STEP attachment row in the Body 3D pane.
pub fn view<'a>(
    fp: &'a Footprint,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let ts = TypeScale::default();
    let sp = Spacing::default();
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let attach_addr = address.clone();
    let attach_btn = button(
        container(
            text(raw("Attach STEP…"))
                .size(ts.body)
                .color(crate::styles::tokens::on_accent(tokens)),
        )
        .padding([sp.s, sp.l]),
    )
    .on_press(LibraryMessage::EditorEvent {
        library_path: attach_addr.library_path,
        table: attach_addr.table.clone(),
        row_id: attach_addr.row_id,
        msg: EditorMsg::StepAttachDialog,
    })
    .style(crate::styles::accent_button(tokens));

    let body: Element<'a, LibraryMessage> = match fp.step_attachment.as_ref() {
        Some(att) => {
            let remove_addr = address.clone();
            let remove_btn = button(
                container(text(raw("Remove")).size(ts.caption).color(text_c))
                    .padding([sp.xs, sp.m]),
            )
            .on_press(LibraryMessage::EditorEvent {
                library_path: remove_addr.library_path,
                table: remove_addr.table.clone(),
                row_id: remove_addr.row_id,
                msg: EditorMsg::StepAttachRemove,
            })
            .style(crate::styles::outline_button(tokens));
            column![
                row![
                    text(raw("STEP attachment")).size(ts.title).color(text_c),
                    Space::new().width(Length::Fill),
                    remove_btn,
                ]
                .align_y(iced::Alignment::Center),
                Space::new().height(sp.s),
                text(raw(format!("File: {}", att.filename)))
                    .size(ts.body)
                    .color(text_c),
                text(raw(format!(
                    "SHA-256: {}",
                    &att.content_hash[..16.min(att.content_hash.len())]
                )))
                .size(ts.caption)
                .color(muted),
            ]
            .spacing(0)
            .boxed()
        }
        None => column![
            row![
                text(raw("STEP attachment")).size(ts.title).color(text_c),
                Space::new().width(Length::Fill),
                attach_btn,
            ]
            .align_y(iced::Alignment::Center),
            Space::new().height(sp.s),
            text(raw(
                "Attach a .step / .stp mech-CAD file. Stored in the footprint file."
            ))
            .size(ts.caption)
            .color(muted),
        ]
        .spacing(0)
        .boxed(),
    };

    container(body)
        .padding(sp.l)
        .style(crate::styles::modal_card(tokens))
        .width(Length::Fill)
        .boxed()
}

/// Embed a freshly picked STEP file in `file` and attach it to the
/// footprint `footprint`, at no offset or turn. `None`, with nothing
/// embedded, when the file holds no such footprint. Pure: the caller saves
/// the file as for any other edit.
pub fn attach_step(
    file: &mut FootprintFile,
    footprint: Uuid,
    bytes: &[u8],
    filename: &str,
) -> Option<StepAttachment> {
    file.attach_step_model(footprint, filename, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_attachment_hash_is_lowercase_sha256_hex() {
        let mut file = FootprintFile::from_footprint(Footprint::empty("F"));
        let uuid = file.footprints[0].uuid;
        let att = attach_step(&mut file, uuid, b"abc", "X.step").expect("attached");
        assert_eq!(
            att.content_hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn attach_step_embeds_once_and_writes_no_step_file() {
        let mut file = FootprintFile::from_footprint(Footprint::empty("F"));
        let uuid = file.footprints[0].uuid;
        let att = attach_step(&mut file, uuid, b"hello", "X.step").expect("first attach");
        // The same bytes again keep the one entry and its first name.
        attach_step(&mut file, uuid, b"hello", "Y.step").expect("dup attach");
        assert_eq!(file.models.len(), 1);
        let model = file.models.get(&att.content_hash).expect("embedded");
        assert_eq!(model.filename, "X.step");
        let bytes = file.step_model(&file.footprints[0]).expect("decodes");
        assert_eq!(bytes.as_deref(), Some(&b"hello"[..]));
        assert!(attach_step(&mut file, Uuid::nil(), b"x", "Z.step").is_none());
        assert_eq!(file.models.len(), 1);
    }
}
