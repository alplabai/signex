# New PCB and New Schematic setup dialogs, and the user's saved setup
# profiles (#2232). Engineering numbers are not localised.

# Messages panel; $name is the profile name.
setup-profile-saved = Saved the setup profile "{ $name }". It is offered in every project.

# Messages panel; $name is the profile name.
setup-profile-removed = Removed the setup profile "{ $name }".

# Messages panel; $profile is the name the profile had when it was removed, $project the project name.
setup-profile-missing = Project { $project } names the setup profile "{ $profile }", which was removed from your saved profiles. Its stored values are used.

# Heading over the user's saved profile cards.
setup-profiles-heading = Your profiles

# Shown under the heading while no profile is saved.
setup-profiles-none = Save the values below as a profile to use them in every project.

# Button that opens the profile name field.
setup-profile-save-as = Save as profile…

setup-profile-name-placeholder = Profile name

setup-profile-save = Save

setup-profile-cancel = Cancel

# Button under a saved profile's card.
setup-profile-remove = Remove

setup-profile-err-name-empty = Give the profile a name.

# $max is the longest name, in characters.
setup-profile-err-name-too-long = A profile name can be at most { $max } characters.

setup-sheet-orientation = Orientation

setup-sheet-landscape = Landscape

setup-sheet-portrait = Portrait

# Messages panel, when New Schematic runs with no project open.
setup-sheet-no-project = Open a project to add a schematic to it.

# Dialog title.
setup-sheet-title = New Schematic

# Intro line of the New PCB dialog; $project is the project name.
setup-pcb-intro = A new board for { $project }, starting from the project's setup. Every value stays editable after it is made.

# Intro line of the New Schematic dialog; $project is the project name.
setup-sheet-intro = A new sheet for { $project }, starting from the project's setup. Create asks where to save it; every value stays editable later.

# Primary button of the New PCB dialog.
setup-create-pcb = Create PCB

# Primary button of the New Schematic dialog; it opens the file dialog.
setup-create-sheet = Create schematic…

# Note under the New PCB preview.
setup-pcb-preview-note = The board opens unsaved. Nothing is written until you save it.

# Inline confirm under a saved profile's card; $name is the profile name.
setup-profile-remove-confirm = Remove "{ $name }" from your profiles? Projects that use it keep their own values.

# The confirm's button that keeps the profile.
setup-profile-keep = Keep

# Under the profile name field when a rule value is not valid.
setup-profile-err-rules = Fix the values marked below before saving them as a profile.

# Messages panel; $why is the reason the settings file was not written.
setup-profile-write-failed = The setup profile change was not saved: { $why }

# Messages panel at start-up; $count is how many saved profiles are not offered.
setup-profiles-skipped = { $count ->
    [one] One saved setup profile could not be read and is not offered. It is kept in your settings unchanged.
   *[other] { $count } saved setup profiles could not be read and are not offered. They are kept in your settings unchanged.
}

# Messages panel; $project is the project name.
setup-profile-missing-unknown = Project { $project } names a setup profile that was removed or is not among your saved profiles. Its stored values are used.

# Messages panel; $project is the project name.
setup-project-closed = Project { $project } was closed, so its New PCB or New Schematic dialog was closed and nothing was made.
