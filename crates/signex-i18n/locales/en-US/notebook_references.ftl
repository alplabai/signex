# Design Notebook references (mockup rows 196 and 197): the hover card of a
# reference, the stale causes and their fixes, the @ picker, the schematic's
# Notebook badges and their card, and Properties > Design notes.

## Hover card facts
notebook-card-footprint = Footprint
notebook-card-sheet = Sheet
notebook-card-sheets = Sheets
notebook-card-nets = Nets
notebook-card-net = Net
notebook-card-parts = Parts
notebook-card-connected = Connected
notebook-card-variants = Variants
notebook-card-used-in = Used in
notebook-card-last-measured = Last measured
notebook-card-fitted-all = Fitted in all { $count }
notebook-card-fitted-some = Fitted in { $count } of { $total }
notebook-card-uses = { $count ->
        [one] 1 use
       *[other] { $count } uses
    }
notebook-card-pin-of = Pin { $pin } of { $part }
notebook-card-net-summary = Net · { $pins ->
        [one] 1 pin
       *[other] { $pins } pins
    } on { $sheets ->
        [one] 1 sheet
       *[other] { $sheets } sheets
    }

## A stale reference: amber, a warning glyph and a wavy underline.
notebook-card-stale-title = Reference out of date
notebook-card-stale-renamed = { $old } was renamed to { $new } in the schematic
notebook-card-stale-deleted = { $name } is not in the design
notebook-card-stale-missing-pin = No pin { $pin } on { $part }
notebook-card-same-part = Same part, { $value }

## Card actions and fixes
notebook-action-cross-probe = Cross-probe
notebook-action-open-sheet = Open sheet
notebook-action-timeline = Timeline
notebook-action-highlight-net = Highlight net
notebook-action-copy-reference = Copy reference
notebook-action-show-in-source = Show in source
notebook-action-update-all = Update all { $count } stale…
notebook-action-update-to = Update to { $name }
notebook-action-remove-reference = Remove reference
notebook-action-keep-as-text = Keep as plain text
notebook-action-pick-pin = Pick another pin

## The status bar of the notebook
notebook-ref-summary-ok = { $count ->
        [one] 1 reference · all resolve
       *[other] { $count } references · all resolve
    }
notebook-ref-summary-stale = { $count ->
        [one] 1 reference
       *[other] { $count } references
    } · { $stale } stale

## The @ picker
notebook-picker-empty = No part, pin or net matches

## Notebook badges on the schematic and their hover card
notebook-badge-card-title = { $name } · { $count ->
        [one] 1 block
       *[other] { $count } blocks
    }
notebook-badge-tone-notes = Notes and calculations only
notebook-badge-tone-pass = All measurements pass
notebook-badge-tone-open-issue = An issue is open
notebook-badge-tone-failing = A measurement fails
notebook-badge-card-hint = Click opens the notebook at the first block

## Properties > Design notes
properties-section-design-notes = Design notes
notebook-design-notes-open = Open in notebook

## Messages lines
notebook-ref-copied = Copied { $reference }
notebook-ref-needs-tab = This opens in the Design Notebook tab.
notebook-ref-open-first = Open { $file } in the Design Notebook first: a fix edits the notebook, which is written on Save.
notebook-ref-open-at = Design Notebook: { $file }, line { $line }
