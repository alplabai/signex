# The board Graphics tools (#2424): Line, Arc, Full Circle, Rectangle and
# Text Frame on a non-copper layer, and the Properties of a board drawing.

## Rows of Place ▸ Graphics on a board and of the PCB Active Bar's Graphics group.
pcb-tool-line = Line
pcb-tool-graphic-arc = Arc
pcb-tool-full-circle = Full Circle
pcb-tool-rectangle = Rectangle
pcb-tool-text-frame = Text Frame

## Tooltips of the Graphics tools on the PCB Active Bar.
pcb-bar-line-tooltip = Place Line
pcb-bar-graphic-arc-tooltip = Place Arc (centre, start, end)
pcb-bar-circle-tooltip = Place Full Circle
pcb-bar-rectangle-tooltip = Place Rectangle
pcb-bar-text-frame-tooltip = Place Text Frame

## What the Messages panel says when a Graphics tool is armed.
pcb-tool-hint-line = Line: click to start, then click each next point; each line is one step, and on a copper layer a track on the net it starts on. Esc or a right-click ends the run, then the tool
pcb-tool-hint-graphic-arc = Arc: click the centre, then the start (it sets the radius), then the end; the arc runs clockwise, and on a copper layer it is a track arc on the net under its start. Esc or a right-click drops it, then ends the tool
pcb-tool-hint-circle = Full Circle: on a non-copper layer, click the centre, then a point on the circle. Esc or a right-click drops it, then ends the tool
pcb-tool-hint-rectangle = Rectangle: on a non-copper layer, click two opposite corners. Esc or a right-click drops it, then ends the tool
pcb-tool-hint-text-frame = Text Frame: on a non-copper layer, click two opposite corners of the box; Tab pauses to edit its text in Properties. Esc or a right-click drops it, then ends the tool

## Notes in the Messages panel. $tool is the tool's name.
pcb-tool-needs-graphic-layer = { $tool }: make a non-copper layer other than a keep-out active to draw on it
# The text a text frame reads until it is edited.
pcb-tool-text-frame-text = Text

## Properties of the drawings selected on a board, or of the one a Graphics
## tool places next.
pcb-prim-graphic-title = Drawings
pcb-prim-graphic-count = { $count ->
    [one] 1 Drawing
   *[other] { $count } Drawings
}
pcb-prim-graphic-center = Centre (X/Y)
pcb-prim-graphic-radius = Radius
pcb-prim-graphic-text = Text
pcb-prim-graphic-text-height = Text Height
pcb-prim-graphic-text-hint = Type \n for a new line; the text wraps to the box.
