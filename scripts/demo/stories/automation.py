"""Story: automation. A script of the user's own drives the running app over
the agent socket (signex-cli, attached): it reads the sheet, colours nets,
names a net, runs ERC and takes its own step back."""

from demo_lib import count_findings
from led_circuit import NET_POINTS, STEPS

TITLE = "Automation over the agent socket"
COLOURS = [
    ("the supply net VIN", "VIN", "#E5A50A"),
    ("the LED anode net", "LED_A", "#3584E4"),
    ("the ground net", "GND", "#2EC27E"),
]


def prepare(ctx):
    project = ctx.new_project(ctx.work / "led-indicator", "led-indicator")
    sheet = project.with_suffix(".snxsch")

    def draw(c):
        for _caption, edits in STEPS:
            c.edit(sheet, *edits)
        c.ask("save_document", document=str(sheet))

    ctx.headless(project, [project, sheet], draw)
    ctx.show(project, sheet)


def pin_summary(answers):
    return "It reads the parts: " + ", ".join(
        f"{a['designator']} with {a['count']} pins" for a in answers
    )


def run(ctx):
    sheet = ctx.project.with_suffix(".snxsch")
    doc = str(sheet)
    ctx.step(
        lambda r: f"A script connects to the running app: {r['data']['count']} documents open",
        lambda: ctx.ask("list_documents"),
        visual=False,
    )
    ctx.step(
        pin_summary,
        lambda: [ctx.ask("query_pins", document=doc, designator=d)["data"] for d in ("J1", "R1", "D1")],
        visual=False,
    )
    for name, net, colour in COLOURS:
        ctx.step(
            f"It colours {name}",
            lambda net=net, colour=colour: ctx.ask(
                "color_net", document=doc, at=NET_POINTS[net], color=colour
            ),
        )
    added = ctx.step(
        "It names the LED anode net",
        lambda: ctx.edit(
            sheet,
            ("place_net_label", {"at": NET_POINTS["LED_A"], "text": "LED_A"}),
            summary="name the LED anode net",
        ),
    )
    ctx.step(
        lambda r: f"It runs ERC: {count_findings(r)}",
        lambda: ctx.ask("check_erc", document=doc),
        hold=3.0,
        visual=False,
    )
    ctx.step(
        "It takes its own step back; the person's undo history is untouched",
        lambda: ctx.ask("revert", document=doc, group=added["data"]["group"]),
    )
    ctx.step("It clears the net colours", lambda: ctx.ask("clear_all_net_colors", document=doc))
