#!/usr/bin/env python3
"""Prepare or run one demo story (#2437).

  drive_story.py prepare <story.py> --work DIR --cli PATH
      headless set-up; writes DIR/open.json (the paths the window opens)
  drive_story.py run <story.py> --work DIR --cli PATH [--attach] [--log FILE]
      the story's steps: attached to the recorded window, or headless (check)
  drive_story.py title <story.py>      the story's title
  drive_story.py pending <story.py>    why the story waits, or nothing

record_story.sh calls both around the window's start. See docs/DEMO_VIDEOS.md.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import demo_lib  # noqa: E402


def load_story(path: Path):
    spec = importlib.util.spec_from_file_location(path.stem, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("phase", choices=["prepare", "run", "title", "pending"])
    parser.add_argument("story", type=Path)
    parser.add_argument("--work", type=Path)
    parser.add_argument("--cli", type=Path, default=None)
    parser.add_argument("--attach", action="store_true")
    parser.add_argument("--log", type=Path)
    parser.add_argument("--capture-start", type=float, default=None)
    args = parser.parse_args()

    story = load_story(args.story)
    if args.phase == "title":
        print(story.TITLE)
        return 0
    if args.phase == "pending":
        # A story whose commands have not merged yet sets PENDING to why;
        # DEMO_RUN_PENDING=1 runs it anyway (a build that has them).
        if not os.environ.get("DEMO_RUN_PENDING"):
            print(getattr(story, "PENDING", ""))
        return 0

    cli = args.cli or demo_lib.env_cli()
    ctx = demo_lib.Context(args.work, cli, attach=args.attach, log=args.log)
    ctx.capture_start = args.capture_start
    open_file = args.work / "open.json"
    try:
        if args.phase == "prepare":
            story.prepare(ctx)
            open_file.write_text(
                json.dumps({"project": str(ctx.project), "open": [str(p) for p in ctx.open_paths]})
            )
            print("\n".join(str(p) for p in ctx.open_paths))
            return 0
        opened = json.loads(open_file.read_text())
        ctx.show(Path(opened["project"]), *[Path(p) for p in opened["open"][1:]])
        ctx.start()
        story.run(ctx)
    except demo_lib.StoryError as error:
        print(f"story {args.story.name}: {error}", file=sys.stderr)
        return 1
    finally:
        ctx.finish()
    return 0


if __name__ == "__main__":
    sys.exit(main())
