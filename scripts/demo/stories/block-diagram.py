"""Story: Block Diagram. A three-sheet hierarchy from the test corpus gets a
block diagram file generated for it; the story then draws the system on it:
blocks for the input, the regulator and the MCU, their ports, the power and
signal links between them, and a group, each a catalog command on the named
diagram.

Pending until the Block Diagram batch merges (after #3126): set
`BLOCK_DIAGRAM_MERGED = True` then. Until then `record_all.sh` skips the
story with `PENDING`; `DEMO_RUN_PENDING=1` runs it anyway, against a
`signex-cli` built from the batch (docs/DEMO_VIDEOS.md, "Stories")."""

from demo_lib import REPO

TITLE = "Block Diagram"
BLOCK_DIAGRAM_MERGED = False
PENDING = (
    ""
    if BLOCK_DIAGRAM_MERGED
    else "the Block Diagram batch has not merged yet; its commands are not in the catalog"
)

CORPUS = REPO / "crates/signex-types/tests/corpus"
SOURCE = CORPUS / "project/hierarchical-three-sheets.snxprj"
SHEETS = [
    "hierarchy-root-two-children.snxsch",
    "hierarchy-nested-middle.snxsch",
    "hierarchy-child-leaf.snxsch",
]
# The blocks drawn: name, function, top-left corner and rails.
BLOCKS = [
    ("USB-C Input", "5 V in, ESD", [10, 40], "VBUS"),
    ("LDO 3.3 V", "Linear regulator", [70, 40], "VBUS,+3V3"),
    ("MCU", "Microcontroller", [130, 40], "+3V3"),
]


def prepare(ctx):
    project = ctx.copy_project(SOURCE, ctx.work / "hierarchy")
    for sheet in SHEETS:
        ctx.copy_file(CORPUS / "schematic" / sheet, project.parent / sheet)

    def generate(c):
        c.ask("generate_block_diagram", document=str(project))

    ctx.headless(project, [project], generate)
    ctx.show(project, project.with_suffix(".snxblk"))


def counted(result):
    diagram = result["data"]["diagram"]
    blocks = len(diagram.get("blocks", []))
    links = len(diagram.get("connections", []))
    return f"{blocks} blocks and {links} connections"


def run(ctx):
    doc = str(ctx.project.with_suffix(".snxblk"))
    ctx.step("The project's block diagram, ready to draw on", hold=2.0)
    ids = {}
    for name, function, at, rails in BLOCKS:
        added = ctx.step(
            f"Add a block: {name}",
            lambda name=name, function=function, at=at, rails=rails: ctx.ask(
                "block_add",
                document=doc,
                name=name,
                function=function,
                at=at,
                size=[40, 25],
                rails=rails,
            ),
        )
        block = added["data"]["block"]
        ids[name] = block["id"] if isinstance(block, dict) else block
    ctx.step(
        "Frame the diagram",
        lambda: ctx.window("zoom_to_fit"),
        hold=1.5,
    )
    for block, side, port, direction in [
        ("USB-C Input", "right", "VBUS", "output"),
        ("LDO 3.3 V", "left", "VIN", "input"),
        ("LDO 3.3 V", "right", "VOUT", "output"),
        ("MCU", "left", "VDD", "input"),
    ]:
        ctx.step(
            f"Give {block} a {port} port",
            lambda block=block, side=side, port=port, direction=direction: ctx.ask(
                "block_port_add",
                document=doc,
                block=ids[block],
                name=port,
                side=side,
                direction=direction,
            ),
            hold=1.5,
        )
    for source, target, interface, label in [
        ("USB-C Input", "LDO 3.3 V", "power", "VBUS"),
        ("LDO 3.3 V", "MCU", "power", "+3V3"),
        ("USB-C Input", "MCU", "usb", "USB D+/D-"),
    ]:
        ctx.step(
            f"Link {source} to {target}: {label}",
            lambda source=source, target=target, interface=interface, label=label: ctx.ask(
                "block_link",
                document=doc,
                to=ids[target],
                interface=interface,
                label=label,
                **{"from": ids[source]},
            ),
        )
    ctx.step(
        "Group the power path",
        lambda: ctx.ask(
            "block_group",
            document=doc,
            name="Power",
            ids=f"{ids['USB-C Input']},{ids['LDO 3.3 V']}",
            color="#d0453a",
        ),
    )
    ctx.step(
        lambda r: f"The diagram now: {counted(r)}",
        lambda: ctx.ask("query_block_diagram", document=doc),
        hold=3.0,
        visual=False,
    )
