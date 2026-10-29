-- #1613 — unique part number per library, across all of its tables. A blank
-- key (an unfilled draft) is exempt; comparison is case-sensitive. Built
-- only after `AppState::migrate` has refreshed `pn_key` and found no
-- duplicates, so two writers racing the same new PN cannot both win.

CREATE UNIQUE INDEX idx_component_rows_pn_key
    ON component_rows(library_id, pn_key)
    WHERE pn_key <> '';
