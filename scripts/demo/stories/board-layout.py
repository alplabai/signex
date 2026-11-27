"""Story: board layout. A routed two-layer microcontroller breakout from the
test corpus: layer views, teardrops, a design rules check that ends clean,
net status and the bill of materials, each a catalog command."""

from demo_lib import REPO, count_findings

TITLE = "Board layout"
SOURCE = REPO / "crates/signex-types/tests/corpus/pcb/real/mcu-breakout-tqfp32.snxprj"


def prepare(ctx):
    project = ctx.copy_project(SOURCE, ctx.work / "mcu-breakout")
    ctx.show(project, project.with_suffix(".snxpcb"))


def routed(result):
    nets = result["data"]["nets"]
    open_links = sum(net["unrouted"] for net in nets)
    status = "every one routed" if open_links == 0 else f"{open_links} connections to route"
    return f"{len(nets)} nets, {status}"


def bom(result):
    rows = result["data"]["rows"]
    parts = sum(row[1] for row in rows)
    return f"Bill of materials from the board: {len(rows)} lines, {parts} parts"


def run(ctx):
    board = str(ctx.project.with_suffix(".snxpcb"))
    ctx.step("A routed two-layer microcontroller breakout", hold=2.0)
    ctx.step("Frame the whole board", lambda: ctx.window("zoom_to_fit"), hold=1.5)
    ctx.step(
        "Highlight the top copper",
        lambda: ctx.ask("highlight_layer", document=board, layer="TopCopper"),
    )
    ctx.step(
        "Then the bottom copper",
        lambda: ctx.ask("highlight_layer", document=board, layer="BottomCopper"),
    )
    ctx.step(
        "Show every layer again",
        lambda: ctx.ask("show_layer_group", document=board, group="all"),
    )
    ctx.step(
        lambda r: f"Add teardrops where tracks enter pads and vias: {len(r['data']['added'])} added",
        lambda: ctx.ask("add_teardrops", document=board),
    )
    ctx.step(
        lambda r: f"Design rules check: {count_findings(r)}",
        lambda: ctx.ask("check_drc", document=board),
        hold=3.0,
        visual=False,
    )
    ctx.step(routed, lambda: ctx.ask("query_board_nets", document=board), visual=False)
    ctx.step(bom, lambda: ctx.ask("board_report_bom", document=board), hold=3.0, visual=False)
    ctx.step("Save", lambda: ctx.ask("save_document", document=board), visual=False)
