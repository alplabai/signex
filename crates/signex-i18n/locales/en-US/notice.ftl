# Error notice cards (#2058): a heading that names the operation, and a
# detail that says what went wrong. Paths, file names and system error
# lines are passed in as they are and stay untranslated.

## Headings

notice-export-failed = Export Failed
notice-save-failed = Save Failed
notice-version-control-failed = Version Control Setup Failed
notice-version-control-paused = Version Control Paused
notice-link-failed = Could Not Open Link
notice-annotate-refused = Annotate Refused
notice-delete-refused = Delete Refused
notice-exclude-refused = Exclude Refused
# A quit was held because a rename or delete is still running.
notice-quit-held = App Not Closed
notice-replace-refused = Replace Refused
notice-library-change-refused = Library Change Not Applied
notice-recovery-failed = Recovery Failed
notice-bom-exported-with-issues = BOM Exported with Issues
notice-rename-incomplete = Rename Incomplete
notice-sheet-symbol-not-changed = Sheet Symbol Not Changed
notice-rename-failed = Rename Failed
notice-import-failed = Altium Import Failed
notice-imported-with-approximations = Imported with Approximations
notice-open-failed = Open Failed
notice-library-read-only = Library Is Read-Only
notice-restore-failed = Restore Failed
notice-create-failed = Create Failed
notice-not-available = Not Available
notice-paste-failed = Paste Failed
notice-place-failed = Place Failed
notice-disk-changed-title = File Changed on Disk
notice-disk-missing-title = File Missing on Disk
notice-disk-folder-missing-title = Folder Missing on Disk

## Details

# $library is the library's name.
notice-library-read-only-detail = The library "{ $library }" ships with { -brand-name } and is not editable.
# A save found its file changed outside the app. $name is the file name.
notice-disk-changed =
    { $name } changed on disk since it was opened or last saved. Saving now would discard that change.

    Overwrite replaces the file with this document. Reload from disk loads the file and discards the unsaved edits in this document.
# $name is the file name; $path is the full path the save writes to.
notice-disk-missing = { $name } is missing on disk (deleted or moved). Re-create writes this document to { $path }.
# $name is the file name; $folder is the folder that is gone.
notice-disk-folder-missing = { $name } cannot be saved: its folder is gone (moved or deleted). Nothing was written. Put the folder back at { $folder }, then save again.

## Library

# $error is the system's reason, in English.
notice-row-not-saved = The component row could not be saved: { $error }
notice-project-add-library-unavailable = Adding a library to the project from the Components Panel is not yet available.
# $action is an internal tag of the change (such as "add" or "remove"), in
# English; $error is the system's reason, in English.
notice-global-libraries-not-saved = Could not save the global library list ({ $action }): { $error }
# $path is the library file; $error is the system's reason, in English.
notice-library-cannot-open = Cannot open library { $path }: { $error }
# $path is the library file being added; $error is the system's reason, in
# English.
notice-library-cannot-add = Cannot add library { $path }: { $error }
# A library that ships with the app could not be opened. $path is the
# library file; $error is the system's reason, in English.
notice-stock-library-not-opened = The library at { $path } ships with { -brand-name } and could not be opened ({ $error }). It is read-only and cannot be repaired from here; reinstall { -brand-name } to restore it.
# $path is the library file.
notice-stock-library-not-reinitialised = The library at { $path } ships with { -brand-name } and cannot be re-initialised. It is read-only and cannot be repaired from here; reinstall { -brand-name } to restore it.
notice-library-write-while-renaming = A library rename or delete is still running, so this library change was not applied. Try again once it finishes.
# $kind is "symbol" or "footprint"; $library is the library file; $error is
# the system's reason, in English.
notice-stock-copy-failed =
    { $kind ->
        [symbol] The stock symbol could not be copied into { $library }: { $error }
        [footprint] The stock footprint could not be copied into { $library }: { $error }
       *[other] The stock primitive could not be copied into { $library }: { $error }
    }

## Autosave recovery: a Restore that did not happen. $name is the file name.

notice-recovery-needs-open = Restore needs the document open: reopen { $name } to restore. The copy is kept and autosave stays paused for it.
notice-recovery-still-moving = The recovery copy of { $name } is still being moved to the file's new name. It was kept; choose Restore again in a moment.
notice-recovery-renamed-while-reading = { $name } was renamed while its recovery copy was being read. The copy was kept and is offered again.
# $reason is why the copy could not be read, in English.
notice-recovery-copy-gone =
    { $reason }
    The recovery copy is gone, so there is nothing to restore. Autosave resumes for this document.
# $reason is why the copy could not be read, in English.
notice-recovery-copy-unreadable =
    { $reason }
    The copy was kept and is offered again. Autosave stays paused for this document until you answer.
notice-recovery-edited-during-prompt = { $name } was edited while the prompt was open, so its recovery copy was not loaded over those edits. The copy was kept and is offered again. Autosave stays paused for this document until you answer, so neither version is overwritten.
notice-recovery-tab-not-activated = The tab of { $name } could not be activated, so its recovery copy was not restored. The copy was kept and is offered again. Autosave stays paused for this document until you answer.
notice-recovery-target-closed = { $name } was closed, so its recovery copy was not restored. The copy was kept and is offered again the next time the file is opened.

## Sheet symbols

# $name is a sheet symbol name.
notice-sheet-name-taken = Another sheet symbol in this project is already named '{ $name }'.
# $file is a sheet file reference as typed.
notice-sheet-file-outside = '{ $file }' points outside the sheet's folder. A sheet file must live beside, or below, the sheet that references it.
# $file is a sheet file reference.
notice-sheet-file-taken = Another sheet symbol in this project already references '{ $file }'.
# $name is the sheet symbol's name.
notice-child-sheet-no-folder = Sheet '{ $name }' has no file yet, and its parent sheet is not saved, so there is no folder to create it in. Save the parent sheet first.
# $path is the sheet file.
notice-paste-read-only = The sheet { $path } was written by a newer { -brand-name } and is read-only in this build; nothing was pasted.

## Project tree: exclude, delete, quit

# $name is the document's name in the project tree; $error is the system's
# reason, in English.
notice-not-excluded = "{ $name }" was not excluded: { $error }
# $file is the file that could not be saved; $name is the document's name in
# the project tree; $error is the system's reason, in English.
notice-not-excluded-unsaved = Could not save "{ $file }", so "{ $name }" was not excluded: { $error }
# $name is the document's name in the project tree.
notice-exclude-not-a-page = "{ $name }" is placed by a sheet symbol and is not a page of this project, so there is nothing to exclude. Remove or change the sheet symbol instead.
# $name is the document's name; $parents lists the sheets that also place it.
notice-delete-placed-elsewhere = "{ $name }" was not deleted: it is also placed by { $parents }. Remove those sheet symbols first.
# $name is the document's name; $reason says why the project could not be read, in English.
notice-delete-placement-unknown = "{ $name }" was not deleted: whether another sheet places it cannot be ruled out ({ $reason }). Try again once the project has been read, or exclude it instead.
notice-project-still-saving = The project file is still being saved; try again in a moment.
# $name is the document's name.
notice-delete-outside-project = "{ $name }" is a shared library or lives outside this project's folder, so it was not deleted from disk. It has only been removed from the project.
# $name is the document's name; $error is the system's reason, in English.
notice-delete-failed = Could not delete "{ $name }": { $error }
# $path is the file still opening; $name is the document's name.
notice-delete-still-opening = { $path } is still opening, so { $name } was not deleted. Delete it again once it has opened.
# $path is the file being restored; $name is the document's name.
notice-delete-still-restoring = A History restore of { $path } is still running, so { $name } was not deleted. Delete it again once the restore has finished.
notice-quit-held-rewrite = Annotate or Reset Duplicate Designators is still running, so the app was not closed. Try again once it finishes.
notice-quit-held-path-op = A rename or delete is still running, so the app was not closed. Try again once it finishes.
# $reason is why the rename failed, in English.
notice-rename-failed-why = Rename failed: { $reason }

# $parents lists the read-only sheets that reference the renamed one.
notice-rename-read-only-parents = Not renamed: { $parents } reference this sheet but are read-only (written by a newer { -brand-name }), so the reference cannot be updated. Edit them in that version first.
# A rename that moved the file but could not update every reference.
# $failed_count is a whole number; $failed lists the sheets, one per line.
# The English keeps "reference(s)" for every count.
notice-rename-refs-failed =
    The file was renamed, but { $failed_count } sheet reference(s) could not be updated and still point at the old name:
    {"  "}{ $failed }
# $unchecked_count is a whole number; $unchecked lists the sheets, one per
# line. The English keeps "sheet(s)" for every count.
notice-rename-refs-unchecked =
    The file was renamed, but { $unchecked_count } sheet(s) could not be read, so it is unknown whether they reference the old name:
    {"  "}{ $unchecked }
# Both of the two messages above, one paragraph each.
notice-rename-refs-failed-and-unchecked =
    The file was renamed, but { $failed_count } sheet reference(s) could not be updated and still point at the old name:
    {"  "}{ $failed }

    The file was renamed, but { $unchecked_count } sheet(s) could not be read, so it is unknown whether they reference the old name:
    {"  "}{ $unchecked }
# $new is where the library file is now; $old is the path the project still
# lists; $error is the system's reason, in English.
notice-library-rename-stranded = The library is now at { $new } but the project still lists { $old }: { $error }. Move it back by hand.
# $path is the library file; $reason is why, in English.
notice-library-rename-failed = Could not rename library { $path }: { $reason }
# Save All left some documents unsaved. $count is a whole number; $listing
# lists them, one per line. The English keeps "item(s)" for every count.
notice-save-all-project-kept =
    Could not save { $count } item(s) — project not closed:
    {"  "}{ $listing }
# Closing a library: $count is how many documents were not saved (a whole
# number); $listing names each one with its reason, one per line.
notice-save-all-library-kept =
    Could not save { $count } item(s) — library not closed:
    {"  "}{ $listing }
# Save All: $count is how many documents were not saved (a whole number);
# $listing names each one with its reason, one per line.
notice-save-all-failed =
    Save All: could not save { $count } item(s):
    {"  "}{ $listing }
# Quitting: $count is how many documents were not saved (a whole number);
# $listing names each one with its reason, one per line.
notice-save-all-quit-held =
    Could not save { $count } item(s) — { -brand-name } stayed open so nothing is lost. Save them manually, or choose Discard All to exit anyway:
    {"  "}{ $listing }
# $path is the project folder; $reason is why, in English.
notice-vcs-auto-commit-off = The repository at { $path } was created, but auto-commit is not enabled: { $reason }
# $path is the project folder.
notice-vcs-project-closed = The repository at { $path } was created, but its project was closed before auto-commit could be enabled. Delete the project's .git folder, reopen the project, then enable version control again.

## Files

# $path is the input project.
notice-import-running = An Altium import of { $path } is still running; wait for it to finish or cancel it from the status bar.
# $path is the converted file; $error is the system's reason, in English.
notice-import-not-opened = Converted to { $path }, but it could not be opened: { $error }
# $report is the report file name; $warnings lists what the import
# approximated, one per line, in English.
notice-import-review =
    Review before fabrication (also in the Messages panel and { $report }):
    { $warnings }
# $path is the project folder; $reason is why, in English.
notice-vcs-paused = Version control is paused for { $path }: { $reason }
# $error is the system's reason, in English.
notice-new-project-failed = Failed to create new project: { $error }
# $path is the project file that is gone.
notice-recent-gone = { $path } no longer exists. It has been removed from File > Open Recent.

## Annotate, Replace and other project-wide passes. $label is the name of
## the pass (such as "Annotate"), in English.

# $reason is why, in English.
notice-run-refused = { $label } was not run: { $reason }
# $count is a whole number; $sheets lists the files. The English keeps
# "sheet(s)" for every count.
notice-run-write-failed = { $label } could not write { $count } unopened sheet(s) and left them as they were on disk: { $sheets }. The open sheets were still renumbered, so run it again once the cause is fixed.
# $sheets lists the sheets and why each is skipped.
notice-run-would-skip = { $label } was not run: it would skip { $sheets }, and renumbering around a sheet can repeat its designators. Nothing was changed; try again once the sheet is free.
# $summary lists the skipped sheets.
notice-run-skipped-read-only = { $label } skipped read-only sheets. { $summary }
# $path is a sheet in the project being renamed.
notice-run-project-moving = { $label } was not run: { $path } is in a project whose folder is being renamed. Try again once the rename has finished.
# $path is a sheet in the project being renamed.
notice-replace-project-moving = Replace was not run: { $path } is in a project whose folder is being renamed. Try again once it finishes.
# $reason is why, in English.
notice-replace-refused-why = Replace was not run: { $reason }

## Placing a library part

notice-place-no-sheet = Open a schematic sheet before placing a library part.
# $path is the library file.
notice-place-library-closed = Library { $path } is no longer open.
# $part is the part's internal part number.
notice-place-part-gone = { $part } is no longer in the library; refresh and pick it again.

## Export

# $path is the written file; $summary counts the issues, in English; $issues
# lists the first issues, each on its own line starting with "- ".
notice-bom-issues = Wrote { $path } with { $summary }.{ $issues }
# As above, plus how many more issues are not listed ($more, a whole number).
notice-bom-issues-more =
    Wrote { $path } with { $summary }.{ $issues }
    { "..." } and { $more } more
notice-export-page-invalid = Specific page must be a positive page number (1, 2, 3, ...).
notice-export-no-files = Select at least one file in the Settings tab before exporting.
notice-bom-no-sheet = Cannot build BOM: no active schematic.
# $path is the source document.
notice-bom-source-closed = Cannot export BOM: the source document { $path } is no longer open.
# $error is the system's reason, in English.
notice-bom-export-failed = BOM export failed: { $error }
# $menu is the menu path of Run ERC.
notice-erc-report-not-run = Cannot export the ERC report: ERC has not been run. Run ERC ({ $menu }) first, then export.
# A menu row named in a notice: $menu is the menu's title, $row the row's
# label, both as the menu bar shows them.
notice-menu-path = { $menu } ▸ { $row }
# $path is the source document.
notice-pdf-source-closed = PDF export refused: the source document { $path } is no longer open.
notice-pdf-no-files = Cannot export PDF: no files selected in the Settings tab.
# $error is the system's reason, in English.
notice-netlist-export-failed = Netlist export failed: { $error }
notice-preview-no-pages = Preview has no pages for the selected range. Check page range input.
# $reason is why, in English.
notice-preview-render-failed = The preview could not be rendered: { $reason }

## Board sketch

# The board kept its sketch and the last refusal follows on its own line.
# $board is the board file name; $refusal is the already-worded refusal.
notice-board-sketch-held-why =
    { board-sketch-held }
    { $refusal }

## Links

# $url is the web address; $error is the system's reason, in English.
notice-link-not-opened = Could not open { $url } in a browser: { $error }
# The browser was started but the launch did not finish. $url is the web
# address; $error is the system's reason, in English.
notice-link-launch-incomplete = The browser launch for { $url } did not complete: { $error }
