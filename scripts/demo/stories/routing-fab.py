"""Story: routing and fabrication outputs. A linear regulator board from the
test corpus with its LED net left unrouted: the net is routed pad to pad
through the interactive router, the pours are filled again, the design
rules check runs, and the fabrication files and a board PDF are written
into the project, each a catalog command."""

from demo_lib import REPO, count_findings

TITLE = "Routing and fabrication outputs"
SOURCE = REPO / "crates/signex-types/tests/corpus/pcb/real/linear-regulator.snxprj"
# The two tracks of the LED_A net (R1 pad 2 to D1 pad 1) in the corpus board.
LED_TRACKS = "5ca1ab1e-0001-4000-8000-000000000023,5ca1ab1e-0001-4000-8000-000000000024"


def prepare(ctx):
    project = ctx.copy_project(SOURCE, ctx.work / "linear-regulator")
    board = project.with_suffix(".snxpcb")

    def unroute(c):
        c.ask("delete_selection", document=str(board), ids=LED_TRACKS)
        c.ask("save_document", document=str(board))

    ctx.headless(project, [project, board], unroute)
    ctx.show(project, board)


def to_route(result):
    nets = result["data"]["nets"]
    open_links = sum(net["unrouted"] for net in nets)
    if open_links == 0:
        return f"{len(nets)} nets, every one routed"
    return f"{len(nets)} nets, {open_links} connection{'s' if open_links != 1 else ''} to route"


def routed(result):
    tracks = len(result["data"].get("tracks", []))
    return f"Route the LED net from R1 to D1: {tracks} tracks around the other copper"


def written(result):
    files = result["data"].get("files", [])
    return f"Fabrication outputs: {len(files)} files written, each with its SHA-256"


def printed(result):
    pages = result["data"].get("pages", [])
    return f"A board PDF for review: {len(pages)} page{'s' if len(pages) != 1 else ''}"


def run(ctx):
    board = str(ctx.project.with_suffix(".snxpcb"))
    folder = ctx.project.parent
    ctx.step("A regulator board with its LED net still to route", hold=2.0)
    ctx.step(
        "Frame the whole board",
        lambda: ctx.window("zoom_to_fit"),
    )
    ctx.step(to_route, lambda: ctx.ask("query_board_nets", document=board), visual=False)
    ctx.step(
        routed,
        lambda: ctx.ask("place_board_route", document=board, start="R1.2", end="D1.1"),
    )
    ctx.step(to_route, lambda: ctx.ask("query_board_nets", document=board), visual=False)
    ctx.step("Fill the pours again", lambda: ctx.ask("repour_all", document=board))
    ctx.step(
        lambda r: f"Design rules check: {count_findings(r)}",
        lambda: ctx.ask("check_drc", document=board),
        hold=3.0,
        visual=False,
    )
    ctx.step(
        written,
        lambda: ctx.ask(
            "fabrication_outputs",
            document=board,
            path=str(folder / "fab"),
            kinds="gerber,drill,centroid",
        ),
        hold=3.0,
        visual=False,
    )
    ctx.step(
        printed,
        lambda: ctx.ask("export_board_pdf", document=board, path=str(folder / "board.pdf")),
        visual=False,
    )
    ctx.step("Save", lambda: ctx.ask("save_document", document=board), visual=False)
