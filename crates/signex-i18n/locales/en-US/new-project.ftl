# New Project dialog (#2232). Engineering numbers are not localised.

# Dialog title.
dialog-new-project-title = New Project

dialog-new-project-name-label = Project name

dialog-new-project-location-label = Location

# Button beside the Location field.
dialog-new-project-browse = Browse…

# Checkbox label; $name is the project name as typed.
dialog-new-project-make-folder = Create "{ $name }" folder

dialog-new-project-profile-heading = Start from a profile

dialog-new-project-profile-hint = Sets board rules and sheet defaults. You can change everything later.

dialog-new-project-profile-hobby-name = Hobby 2-layer

dialog-new-project-profile-hobby-blurb = Cheap prototypes from any fab.

dialog-new-project-profile-standard-name = Standard 4-layer

dialog-new-project-profile-standard-blurb = Mixed-signal boards, solid ground plane.

dialog-new-project-profile-hdi-name = HDI 6-layer

dialog-new-project-profile-hdi-blurb = Fine-pitch BGAs, tight routing.

dialog-new-project-profile-custom-name = Custom

dialog-new-project-profile-custom-blurb = Any layer count and your own rules.

# First line of a profile card; values in mm.
dialog-new-project-summary-clearance = { $clearance } mm clearance

# Second line of a profile card.
dialog-new-project-summary-tracks = { $track } mm tracks · { $via }/{ $drill } vias

dialog-new-project-summary-custom-layers = { $layers } copper layers

dialog-new-project-summary-custom-note = set every value yourself

dialog-new-project-customize = Customize settings

dialog-new-project-customize-optional = (optional)

# Section label.
dialog-new-project-pcb-heading = PCB RULES

# Section label.
dialog-new-project-sheet-heading = SCHEMATIC

dialog-new-project-fab-label = Manufacturer rule template

dialog-new-project-fab-standard = Standard fab

dialog-new-project-fab-advanced = Advanced fab

dialog-new-project-fab-hdi = HDI fab

dialog-new-project-fab-import = Import rules…

dialog-new-project-fab-note = Generic capability class. Values below are its minimums; any of them can be tightened.

dialog-new-project-fab-note-imported = Rule file loaded. It filled the values below.

# $why is the reason from the rule file parser.
dialog-new-project-import-failed = Rule file not used: { $why }

dialog-new-project-layers-label = Copper layers

dialog-new-project-field-clearance = Clearance

dialog-new-project-field-track = Track width

dialog-new-project-field-via = Via size

dialog-new-project-field-drill = Via drill

dialog-new-project-unit-mm = mm

dialog-new-project-sch-size = Sheet size

dialog-new-project-sch-title-block = Title block

dialog-new-project-sch-title-block-default = Signex default

dialog-new-project-sch-title-block-none = None

dialog-new-project-sch-designators = Designators

# A sheet size in the picker.
dialog-new-project-sheet-landscape = { $size } landscape

dialog-new-project-preview-heading = PREVIEW

# Bold start of the board line.
dialog-new-project-preview-layers = { $count }-layer

# Rest of the board line; $thickness such as 1.6 mm.
dialog-new-project-preview-board = board, { $thickness } thick

# Sheet line; $paper such as A3 landscape.
dialog-new-project-preview-sheet = { $paper } sheet

dialog-new-project-preview-note = New schematics and PCBs in this project start from these settings. Edit them any time in Project Options.

dialog-new-project-stack-silk-top = Top Overlay

dialog-new-project-stack-mask-top = Top Solder

dialog-new-project-stack-copper-top = Top Layer

dialog-new-project-stack-copper-inner = Mid-Layer { $n }

dialog-new-project-stack-copper-bottom = Bottom Layer

dialog-new-project-stack-prepreg = Prepreg

dialog-new-project-stack-core = Core

dialog-new-project-stack-mask-bottom = Bottom Solder

dialog-new-project-footer-hint = Enter to create · Esc to cancel

dialog-new-project-cancel = Cancel

dialog-new-project-create = Create project

dialog-new-project-err-name-empty = Enter a project name.

dialog-new-project-err-name-invalid = A project name cannot contain \ / : * ? " < > | or end with a dot or space.

dialog-new-project-err-location-empty = Choose where to save the project.

dialog-new-project-err-location-missing = The location folder does not exist.

dialog-new-project-err-exists = { $path } already exists. Choose another name or location.

dialog-new-project-err-folder-not-empty = The folder { $path } already exists and is not empty. Choose another name or location.

dialog-new-project-err-not-number = Enter a number in mm.

dialog-new-project-err-range = Use a value from { $min } to { $max } mm.

dialog-new-project-err-drill = Must be smaller than the via size.

# OS folder picker title, from Browse...
dialog-new-project-pick-location-title = Choose the project location

# OS file picker title and filter name, from Import rules...
dialog-new-project-pick-rules-title = Import rules
dialog-new-project-pick-rules-filter = Rule file

# Projects panel, empty-area context menu row that opens this dialog.
dialog-new-project-entry-row = New Project...

dialog-new-project-field-thickness = Board thickness

# Create failed; $why is the reason the file writer gave.
dialog-new-project-err-create-failed = The project was not created: { $why }

# Rule-file problems, shown after "Rule file not used:".
dialog-new-project-err-rules-read = { $path } could not be read ({ $detail }).
dialog-new-project-err-rules-syntax = it is not valid TOML ({ $detail }).
dialog-new-project-err-rules-no-pcb = it has no [pcb] table.
dialog-new-project-err-rules-missing = { $key } is missing from [pcb].
dialog-new-project-err-rules-value = { $key }: { $why }
dialog-new-project-err-rules-too-large = it is larger than { $max } bytes.

# Create button label while the project is being written.
dialog-new-project-creating = Creating…

# Create refused because the same project is already being created.
dialog-new-project-err-create-pending = This project is already being created.

# Project Options, Project setup section: the stored section is not usable.
dialog-new-project-setup-unreadable = The project's setup defaults are not readable by this version of Signex; they are kept unchanged and not applied. Editing a value here replaces that section when you save.

# Placeholders of the name and location fields, shown while they are empty.
dialog-new-project-name-placeholder = Project name
dialog-new-project-location-placeholder = Folder the project goes in
