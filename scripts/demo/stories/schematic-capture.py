"""Story: schematic capture. A new project, parts from the stock library,
wires, power ports and a net label, then an ERC run that ends clean."""

from demo_lib import count_findings
from led_circuit import STEPS

TITLE = "Schematic capture"


def prepare(ctx):
    project = ctx.new_project(ctx.work / "led-indicator", "led-indicator")
    ctx.show(project, project.with_suffix(".snxsch"))


def run(ctx):
    sheet = ctx.project.with_suffix(".snxsch")
    ctx.step("A new project with one empty sheet", hold=2.0)
    for caption, edits in STEPS:
        ctx.step(caption, lambda edits=edits: ctx.edit(sheet, *edits))
    ctx.step(
        lambda r: f"Electrical rules check: {count_findings(r)}",
        lambda: ctx.ask("check_erc", document=str(sheet)),
        hold=3.0,
        visual=False,
    )
    ctx.step("Save", lambda: ctx.ask("save_document", document=str(sheet)), visual=False)
