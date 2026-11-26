"""Story: Design Notebook. A linear regulator board from the test corpus with
a bring-up notebook beside it: the notebook compiles, its measurements are
checked against their limits, its attachments are found in the project,
and the design report is generated from the project, each a catalog
command."""

from demo_lib import REPO

TITLE = "Design Notebook"
SOURCE = REPO / "crates/signex-types/tests/corpus/pcb/real/linear-regulator.snxprj"
NOTEBOOK = REPO / "crates/signex-types/tests/corpus/notebook/with-measurements"


def prepare(ctx):
    project = ctx.copy_project(SOURCE, ctx.work / "linear-regulator")
    ctx.copy_folder(NOTEBOOK, project.parent / "notebook")
    ctx.show(project, project.with_suffix(".snxpcb"))


def plural(count, word):
    return f"{count} {word}{'' if count == 1 else 's'}"


def compiled(result):
    data = result["data"]
    pages = data.get("pages")
    count = len(pages) if isinstance(pages, list) else pages
    errors = data.get("errors", 0)
    state = "no errors" if not errors else plural(errors, "error")
    return f"The notebook compiles: {plural(count or 0, 'page')}, {state}"


def measured(result):
    summary = result["data"].get("summary", {})
    readings = summary.get("readings", 0)
    passed = summary.get("pass", 0)
    return f"Bring-up readings checked against their limits: {passed} of {readings} pass"


def attached(result):
    attachments = result["data"].get("attachments", [])
    present = sum(1 for a in attachments if isinstance(a, dict) and a.get("present"))
    return f"Scope captures kept in the project: {present} of {plural(len(attachments), 'attachment')}"


def reported(result):
    data = result["data"]
    sections = data.get("sections", [])
    return f"Design report generated from the project: {plural(len(sections), 'section')}"


def run(ctx):
    project = str(ctx.project)
    ctx.step("A regulator board and its bring-up notebook", hold=2.0)
    ctx.step(
        "Show the notebook's annotations beside the board",
        lambda: ctx.window("set_dock_panel", panel="annotations", state="open"),
    )
    ctx.step(compiled, lambda: ctx.ask("notebook_compile", document=project), visual=False)
    ctx.step(
        measured,
        lambda: ctx.ask("notebook_measurements", document=project),
        hold=3.0,
        visual=False,
    )
    ctx.step(attached, lambda: ctx.ask("notebook_attachments", document=project), visual=False)
    ctx.step(
        reported,
        lambda: ctx.ask("generate_design_notebook", document=project, save=True, update=True),
        hold=3.0,
        visual=False,
    )
