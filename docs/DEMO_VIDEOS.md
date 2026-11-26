# Demo videos (#2437)

The README, the docs and the website show short videos of Signex at work.
They are recorded by scripts, not by hand, so every release records them again
and they never go stale.

## Record

```bash
cargo build --release -p signex-app -p signex-cli
scripts/demo/record_all.sh
```

This records every story into `docs/media/`: `<story>.mp4` and an 800 px,
10 fps `<story>.gif` for the README, then cuts the website hero loop
(`hero.mp4`, `hero.webm` when it is smaller, `hero-poster.webp`). Commit the
new files with the release.

`scripts/demo/record_all.sh --check` runs every story without a display, in a
headless `signex-cli` run, and stops each story at the first command that is
refused or fails; it says `ok` or names the stories that failed, and exits 1
if any did. It needs only Python 3 and `signex-cli` (`SIGNEX_CLI` names
another build, for example a debug one). Run it after changing a story or a
command a story uses. A headless run does not apply the agent allowlist and
has no window, so the check proves each step runs, not that an attached run
accepts it, and it skips the window-layout steps (`ctx.window`).

Recording needs, on Linux (Debian/Ubuntu packages):

| Package | For |
|---|---|
| `xvfb` | the virtual display the app draws on |
| `ffmpeg`, built with libx264, libvpx, libwebp and libass (Debian's is) | the capture, the MP4 and GIF, the WebM hero and its poster, the burnt-in captions |
| `mesa-vulkan-drivers` | lavapipe, Mesa's software Vulkan driver, for wgpu |
| `python3-numpy` | `render.py`'s frame comparison |
| `libxkbcommon-x11-0` (and `libvulkan1`, `libxcursor1`, `libxi6`, `libxrandr2`, usually present) | the libraries the app's X11 windowing loads at start; without `libxkbcommon-x11-0` the app stops before its window opens |

```bash
sudo apt install xvfb ffmpeg mesa-vulkan-drivers python3-numpy libxkbcommon-x11-0
```

No GPU and no desktop are needed. `record_all.sh` and `record_story.sh`
check for these first (`check_tools.sh`) and stop, naming each one that is
missing, before anything starts. Only one recording runs at a time, and the
encodes run under `nice`.

## How a recording is made

`record_story.sh` records one story:

1. The story's set-up runs headless (`drive_story.py prepare`): it makes a new
   project or copies one from the repository into a temporary folder. The
   repository's files are never edited.
2. A virtual display (Xvfb, 1920x1080) starts, then the release app, opening
   the story's project with a private configuration: agent connections on,
   Graphite Dark, the window filling the screen, the Messages panel collapsed.
3. ffmpeg captures the display.
4. The story's steps run (`drive_story.py run`). Every action is a catalog
   command sent through the agent socket by one attached `signex-cli run`
   ([AGENT_SOCKET.md](AGENT_SOCKET.md), [CLI.md](CLI.md)); there is no other
   control channel. Each step has a caption and a hold so it can be read.
5. `render.py` cuts the app's start, speeds up any action that takes longer
   than two seconds, draws a pointer that moves to where each step changed
   the picture and a click ripple there, burns in the captions in Inter, and
   writes the MP4 and the GIF.

The hero loop is the schematic story sped up to six seconds, faded into the
first seconds of the board story and back to the empty sheet, without
captions.

## Stories

`scripts/demo/stories/<name>.py`, one per video. Each has a `TITLE`, a
`prepare(ctx)` that names the project and documents the window opens
(`ctx.show`), and a `run(ctx)` of steps:

```python
ctx.step("Add a green LED", lambda: ctx.edit(sheet, ("place_component", {...})))
ctx.step(lambda r: f"Design rules check: {count_findings(r)}",
         lambda: ctx.ask("check_drc", document=board), visual=False)
```

`ctx.edit` sends `apply_edits` (one undo step), `ctx.ask` any other command
on the agent channel, and `ctx.window` a window-layout command (`zoom_to_fit`,
`set_dock_panel`), which a headless check skips. A caption may be a function
of the command's answer. `visual=False` marks a step that changes nothing on
the canvas (a query), so no pointer is drawn for it. A story whose commands
have not merged sets `PENDING` to the reason, and both `record_all.sh` modes
skip it with that reason.

| Story | Shows |
|---|---|
| `schematic-capture` | a new project; a connector, a resistor and an LED from the stock library; wires, ground ports and a net label; ERC |
| `board-layout` | a routed two-layer microcontroller breakout from the test corpus; layer highlighting, teardrops, DRC, net status, the bill of materials |
| `automation` | a script driving the running app over the agent socket: it reads the parts, colours nets, names a net, runs ERC and reverts its own step |
| `design-notebook` | a regulator board from the test corpus with its bring-up notebook: the Annotations panel, the notebook compiled, its measurements checked against their limits, its attachments, the design report generated |
| `block-diagram` | pending: waits for the Block Diagram batch |
| `routing-fab` | the regulator board with its LED net unrouted: Fit, the net routed pad to pad, the pours filled again, DRC, the fabrication files and a board PDF written into the project |

Captions are plain English about what the product does. They name no other
company or product.

## Limits

- A story can only show what the agent channel accepts
  ([AGENT_SOCKET.md](AGENT_SOCKET.md), "Allowlist"). Opening a document,
  making a new project and making a new board are not on it (each brings a
  tab to the front), so a story's set-up makes the project headless and the
  window opens it at start.
- There is no autorouter: the routing story routes its net pad to pad.
- The GIFs are 800 px wide: canvas text that is small at 1920x1080 is not
  readable in them; the MP4s are.
