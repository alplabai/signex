"""Story runner for the scripted demo videos (#2437).

A story is a Python module in `stories/` with `TITLE`, `prepare(ctx)` and
`run(ctx)`. Every action goes to Signex as a catalog command through one
`signex-cli run` process: attached to the window being recorded, or headless
with `--check` (no display needed). Each `ctx.step` is timed and captioned;
`render.py` turns the timings into the cut, the pointer and the captions.
See docs/DEMO_VIDEOS.md.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

STOCK_LIBRARY = "019df45d-af5e-7a00-8c3b-dd3bf89b40ae"
REPO = Path(__file__).resolve().parents[2]
SEED_PROJECT = REPO / "crates/signex-types/tests/corpus/project/minimal-empty.snxprj"


class StoryError(RuntimeError):
    """A command a story needs was refused or failed."""


class Cli:
    """One `signex-cli run` process: one JSON line in, one result line out."""

    def __init__(self, cli: Path, project: Path, *, attach: bool, extra: list[str] | None = None):
        mode = "--attach" if attach else "--headless"
        args = [str(cli), "run", str(project), mode, "--save", "--allow", "export"]
        args += extra or []
        self.proc = subprocess.Popen(
            args,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            bufsize=1,
        )

    def ask(self, command: str, **args) -> dict:
        line = json.dumps({"id": command, "args": args})
        assert self.proc.stdin and self.proc.stdout
        self.proc.stdin.write(line + "\n")
        self.proc.stdin.flush()
        reply = self.proc.stdout.readline()
        if not reply:
            raise StoryError(f"signex-cli ended before answering `{command}`")
        result = json.loads(reply)
        if result.get("status") not in ("ran", "answered"):
            raise StoryError(f"`{command}` {result.get('status')}: {result.get('reason')}")
        return result

    def close(self) -> None:
        if self.proc.stdin:
            self.proc.stdin.close()
        try:
            self.proc.wait(timeout=60)
        except subprocess.TimeoutExpired:
            self.proc.kill()


class Context:
    """What a story sees: its work folder, the CLI and the step log."""

    def __init__(self, work: Path, cli: Path, *, attach: bool, log: Path | None):
        self.work = work
        self.cli_path = cli
        self.attach = attach
        self.log_path = log
        self.steps: list[dict] = []
        self.cli: Cli | None = None
        self.project: Path | None = None
        self.open_paths: list[Path] = []
        self.capture_start: float | None = None

    # -- preparation (headless, before the window starts) -----------------

    def headless(self, project: Path, open_paths: list[Path], steps) -> None:
        """Open `open_paths` in a headless run of `project` and call
        `steps(ctx)` there, outside the recording (a story's set-up)."""
        saved = self.cli
        self.cli = Cli(self.cli_path, project, attach=False)
        try:
            for path in open_paths:
                self.cli.ask("open_document", path=str(path))
            steps(self)
        finally:
            self.cli.close()
            self.cli = saved

    def new_project(self, folder: Path, name: str) -> Path:
        """Create an empty project with one sheet, as File > New Project does."""
        folder.mkdir(parents=True, exist_ok=True)
        seed_dir = self.work / "seed"
        seed_dir.mkdir(exist_ok=True)
        seed = seed_dir / "seed.snxprj"
        shutil.copyfile(SEED_PROJECT, seed)
        project = folder / f"{name}.snxprj"
        cli = Cli(self.cli_path, seed, attach=False, extra=["--allow-path", str(folder)])
        try:
            cli.ask("new_document", path=str(project))
        finally:
            cli.close()
        shutil.rmtree(seed_dir)
        return project

    def copy_project(self, source: Path, folder: Path) -> Path:
        """Copy a repository project and the files named like it (its board,
        its sheet) into the work folder; the repository copy is never edited."""
        folder.mkdir(parents=True, exist_ok=True)
        for path in source.parent.glob(source.stem + ".*"):
            shutil.copyfile(path, folder / path.name)
        return folder / source.name

    def copy_folder(self, source: Path, folder: Path) -> None:
        """Copy a repository folder (a notebook and its attachments) into the
        work folder; the repository copy is never edited."""
        shutil.copytree(source, folder, dirs_exist_ok=True)

    def copy_file(self, source: Path, target: Path) -> None:
        """Copy one repository file (a sheet of a corpus project) into the
        work folder; the repository copy is never edited."""
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)

    def show(self, project: Path, *documents: Path) -> None:
        """The project and documents the window opens at start."""
        self.project = project
        self.open_paths = [project, *documents]

    # -- the recorded part -------------------------------------------------

    def start(self) -> None:
        assert self.project, "prepare() must call ctx.show()"
        self.cli = Cli(self.cli_path, self.project, attach=self.attach)
        if not self.attach:
            # The window opens these from its command line; a headless run
            # opens them itself.
            for path in self.open_paths:
                self.cli.ask("open_document", path=str(path))

    def ask(self, command: str, **args) -> dict:
        assert self.cli
        return self.cli.ask(command, **args)

    def window(self, command: str, **args) -> dict | None:
        """A window-layout command (`zoom_to_fit`, `set_dock_panel`, the
        tiling): sent to the recorded window, skipped in a headless check,
        which has no window to lay out."""
        if not self.attach:
            print(f"  (window only, skipped headless) {command}", file=sys.stderr)
            return None
        return self.ask(command, **args)

    def revision(self, document: Path) -> int:
        return self.ask("read_document", document=str(document))["data"]["revision"]

    def edit(self, document: Path, *edits: tuple[str, dict], summary: str = "") -> dict:
        """One `apply_edits` call: the edits as one undo step on `document`."""
        args = {
            "document": str(document),
            "expect_revision": self.revision(document),
            "edits": [{"id": cmd, "args": a} for cmd, a in edits],
        }
        if summary:
            args["summary"] = summary
        return self.ask("apply_edits", **args)

    def step(self, caption: str, action=None, *, hold: float = 2.5, visual: bool = True):
        """Run `action()` (catalog commands) under `caption` and time it.

        `visual` says the step changes the canvas, so the video draws the
        pointer and a click where it changed. The caption may be a callable
        that turns the action's result into text (a check's counts)."""
        t0 = time.time()
        result = action() if action else None
        t1 = time.time()
        text = caption(result) if callable(caption) else caption
        self.steps.append(
            {"caption": text, "t0": t0, "t1": t1, "hold": hold, "visual": visual and action is not None}
        )
        print(f"  [{t1 - t0:5.2f}s] {text}", file=sys.stderr)
        if os.environ.get("DEMO_DEBUG") and result:
            print(json.dumps(result.get("data"))[:1500], file=sys.stderr)
        if self.attach:
            time.sleep(hold)
        return result

    def finish(self) -> None:
        if self.cli:
            self.cli.close()
        if self.log_path:
            log = {"capture_start": self.capture_start, "steps": self.steps}
            self.log_path.write_text(json.dumps(log, indent=1))


def count_findings(report: dict) -> str:
    """'no errors, no warnings' style summary of a check report."""
    findings = report["data"]["findings"]
    errors = sum(1 for f in findings if f["severity"] == "error")
    warnings = sum(1 for f in findings if f["severity"] == "warning")

    def n(count: int, word: str) -> str:
        return f"no {word}s" if count == 0 else f"{count} {word}{'s' if count != 1 else ''}"

    return f"{n(errors, 'error')}, {n(warnings, 'warning')}"


def env_cli() -> Path:
    return Path(os.environ.get("SIGNEX_CLI", REPO / "target/release/signex-cli"))
