# The sketch info bar and the sketch parameters table (mockup row 189), on the
# footprint sketch and the board sketch alike.

# The info bar: what is being sketched, on which plane, at which grid step.
sketch-info-footprint = Footprint sketch · plane { $plane ->
        [body] Body top
       *[top] Top
    } · grid { $grid } mm
sketch-info-board = Board shape · plane { $plane ->
        [body] Body top
       *[top] Top
    } · grid { $grid } mm
sketch-info-parameters = Parameters

# The table.
sketch-parameters-title = Parameters
sketch-parameters-close = Close
sketch-parameters-col-name = Name
sketch-parameters-col-expression = Expression
sketch-parameters-col-value = Value
sketch-parameters-col-unit = Unit
sketch-parameters-col-used-by = Used by
sketch-parameters-empty = No parameters yet. Add one, or use Make parameter on a dimension.
sketch-parameters-add = Add parameter
sketch-parameters-units-hint = Units: mm, mil, in, um, deg, rad. A number without a unit is a count.
sketch-parameters-expression-placeholder = Expression
sketch-parameters-value = { $value }
sketch-parameters-last-good = { $value } last good
sketch-parameters-no-value = none yet
sketch-parameters-unit-mm = mm
sketch-parameters-unit-deg = deg
sketch-parameters-unit-count = count
sketch-parameters-error = { $reason }
sketch-parameters-error-blocked = Reads { $name }, which has an error.

# What reads a parameter (the Used by column).
sketch-parameters-used-by-none = none
sketch-parameters-used-by-list = { $uses }
sketch-parameters-use-parameter = { $name }
sketch-parameters-use-plane = body plane
sketch-parameters-use-distance = distance dimension
sketch-parameters-use-angle = angle dimension
sketch-parameters-use-linear = linear pattern
sketch-parameters-use-grid = grid pattern
sketch-parameters-use-circular = circular pattern
sketch-parameters-use-pad = pad { $number }
sketch-parameters-use-pour = pour
sketch-parameters-use-cutout = cutout
sketch-parameters-use-hole = mounting hole
sketch-parameters-use-v-score = V-score

# Typing a letter offers the names.
sketch-parameters-autocomplete-title = Insert name

# The row menu: Rename… and Delete, with the in-use prompt.
sketch-parameters-row-menu = ⋯
sketch-parameters-rename = Rename…
sketch-parameters-delete = Delete
sketch-parameters-rename-placeholder = New name
sketch-parameters-rename-apply = Rename
sketch-parameters-cancel = Cancel
sketch-parameters-delete-prompt = { $name } is used by { $uses }. Replace each use with its value and delete { $name }?
sketch-parameters-replace-with-value = Replace with value

# Why a name was refused.
sketch-parameters-name-empty = A parameter needs a name.
sketch-parameters-name-invalid = Use letters, digits and _, not starting with a digit.
sketch-parameters-name-reserved = { $name } is reserved.
sketch-parameters-name-taken = A parameter named { $name } already exists.
sketch-parameters-name-missing = There is no parameter named { $name }.
