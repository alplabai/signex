# Document ids (#2172)

A document is one file a person or an agent can name. Its id is made with the
document, stored in the document's own file and read back every time the file
is opened, so it outlives tabs, project closes and restarts. The text form is
the hyphenated uuid, as in every file. `signex_types::document_id::DocumentId`
is the newtype over `uuid::Uuid`; the nil uuid is never an id.

## Where each kind stores it

| Kind | Stored field | Format change |
|---|---|---|
| sheet (`.snxsch`) | `schematic_id`, the sheet's own uuid | none |
| board (`.snxpcb`) | `pcb_id`, the board's own uuid | none |
| symbol container (`.snxsym`) | `file_uuid` | none |
| footprint container (`.snxfpt`) | `file_uuid` | none |
| component row | the row id | none |
| project (`.snxprj`) | `document_id` (new) | optional key, token stays `snxprj/1` |

`document_id` in a `.snxprj` is an additive optional key, like `description`:
skipped when absent, read as absent when it is not an id, kept by older builds
in the file's unknown keys. No token was reused or changed.

## Rules

- **Minted on open.** A file with no id (a project an older build wrote, a sheet
  or board holding the nil uuid) gets one when it is opened. That is not an edit:
  the document is not dirty, and the next save writes the id. A project that is
  never edited and saved keeps getting a new id each session.
- **Rename and move keep the id**; the id is in the file, not in the path.
- **Save As is a new document.** The copy is encoded with a new id and the open
  document takes it when the write lands; the original file keeps its id. Sheets
  only so far.
- **A copy made outside Signex** repeats an id. The document seen first keeps it;
  a later one is given a new id in memory, Messages says so, and its next save
  writes it. All repairs happen in `Signex::reconcile_document_ids`, the one place
  an id is assigned to an existing document. "Seen first" counts among the
  documents loaded now: a closed one has no claim, so a copy that is opened
  again later yields to the document that holds the id at that time. A file that
  is only listed or loaded for one query is never given an id.
- **Undo and a recovery restore keep the open document's id** (a sheet, symbol
  or footprint): they bring back content, not identity.
- **Every document an open project references is listed with its id**, opened in
  a tab or not. The id of a file that is not loaded is read from the head of the
  file on a worker when the project opens (`ProjectMsg::DocumentHeadersRead`);
  nothing is parsed in full and no tab is made.

## What reads them

`list_documents`, `document_info`, `resolve`, `find` and `read_document` answer
through `CommandOutcome::Answered`; the commands that act on a document name it
with the `document` operand (an id or an absolute path, resolved by the volume's
path key). See `docs/COMMANDS.md`.
