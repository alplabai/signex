"""Story: schematic capture. A new project, parts from the stock library,
wires, power ports and a net label, then an ERC run that ends clean."""

from demo_lib import count_findings
from led_circuit import STEPS

TITLE = "Schematic capture"
# The first steps place the parts (connector, resistor, LED); the view is
# fitted to them before the wiring starts.
PARTS = 3


def prepare(ctx):
    project = ctx.new_project(ctx.work / "led-indicator", "led-indicator")
    ctx.show(project, project.with_suffix(".snxsch"))


def run(ctx):
    sheet = ctx.project.with_suffix(".snxsch")
    ctx.step("A new project with one empty sheet", hold=2.0)
    for index, (caption, edits) in enumerate(STEPS):
        ctx.step(caption, lambda edits=edits: ctx.edit(sheet, *edits))
        if index == PARTS - 1:
            ctx.step("Fit the view to the parts", lambda: ctx.window("zoom_to_fit"), hold=1.5)
    # The wires, ports and label reach past the parts: frame them all.
    ctx.step("Fit the whole circuit", lambda: ctx.window("zoom_to_fit"), hold=1.5)
    ctx.step(
        lambda r: f"Electrical rules check: {count_findings(r)}",
        lambda: ctx.ask("check_erc", document=str(sheet)),
        hold=3.0,
        visual=False,
    )
    ctx.step("Save", lambda: ctx.ask("save_document", document=str(sheet)), visual=False)
