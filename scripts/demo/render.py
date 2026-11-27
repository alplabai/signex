#!/usr/bin/env python3
"""Turn a raw story capture into the published MP4 and GIF (#2437).

  render.py story --raw raw.mkv --steps steps.json --title T --out DIR --name N
  render.py hero --schematic A.clean.mp4 --board B.clean.mp4 --out DIR

`story`: cuts the idle start, speeds up slow actions, draws a pointer that
moves to where each step changed the picture, a click ripple there, and burns
in the captions (ASS through libass, set in the app's own Inter face). It also
writes N.clean.mp4 next to the raw file (the same cut with nothing drawn on
it), which `hero` uses. See docs/DEMO_VIDEOS.md.
"""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[2]
FONTS = REPO / "crates/signex-app/assets/fonts"

LEAD = 0.6  # s of the previous hold kept before a step, for the pointer to move
MOVE = 0.45  # s the pointer takes to reach a step's spot
SETTLE = 0.8  # s after a step's answer for the window to draw it
MAX_ACTION = 2.0  # s an action may last on screen; longer ones are sped up
INTRO = 2.0  # s of title card before the first step
OUT_FPS = 30


def run(args: list[str], **kw) -> subprocess.CompletedProcess:
    return subprocess.run(args, check=True, **kw)


def probe(path: Path) -> tuple[int, int, float]:
    out = run(
        ["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries",
         "stream=width,height:format=duration", "-of", "json", str(path)],
        capture_output=True, text=True,
    ).stdout
    info = json.loads(out)
    stream = info["streams"][0]
    return stream["width"], stream["height"], float(info["format"]["duration"])


def frame(path: Path, t: float, size: tuple[int, int]) -> np.ndarray:
    """The grey frame of `path` at `t` seconds."""
    width, height = size
    raw = run(
        ["ffmpeg", "-v", "error", "-ss", f"{max(t, 0):.3f}", "-i", str(path), "-frames:v", "1",
         "-f", "rawvideo", "-pix_fmt", "gray", "-"],
        capture_output=True,
    ).stdout
    if len(raw) < width * height:
        return np.zeros((height, width), np.uint8)
    return np.frombuffer(raw[: width * height], np.uint8).reshape(height, width)


def hotspot(before: np.ndarray, after: np.ndarray, block: int = 24):
    """Where the picture changed most: the centre of the densest changed area,
    or None when (almost) nothing changed."""
    changed = np.abs(before.astype(np.int16) - after.astype(np.int16)) > 24
    if changed.sum() < 40:
        return None
    h, w = changed.shape
    hb, wb = h // block, w // block
    counts = changed[: hb * block, : wb * block].reshape(hb, block, wb, block).sum(axis=(1, 3))
    padded = np.pad(counts, 1)
    smooth = sum(padded[dy : dy + hb, dx : dx + wb] for dy in range(3) for dx in range(3))
    by, bx = np.unravel_index(int(np.argmax(smooth)), smooth.shape)
    y0, y1 = max(by - 2, 0) * block, min(by + 3, hb) * block
    x0, x1 = max(bx - 2, 0) * block, min(bx + 3, wb) * block
    ys, xs = np.nonzero(changed[y0:y1, x0:x1])
    return int(x0 + xs.mean()), int(y0 + ys.mean())


def plan(steps: list[dict], start: float, duration: float) -> list[dict]:
    """The parts of the raw video to keep, in order, with their speed.

    Raw time is seconds since the capture started. Everything before the
    intro is dropped (the app starting); each step keeps LEAD seconds before
    it, its action (sped up past MAX_ACTION) and its hold."""
    rel = [dict(s, a=s["t0"] - start, b=s["t1"] - start) for s in steps]
    parts = [{"from": max(rel[0]["a"] - LEAD - INTRO, 0), "to": rel[0]["a"] - LEAD, "speed": 1.0,
              "step": None}]
    for i, step in enumerate(rel):
        begin = step["a"] - LEAD
        end = rel[i + 1]["a"] - LEAD if i + 1 < len(rel) else min(step["b"] + step["hold"], duration)
        action_end = min(step["b"] + (SETTLE if step["visual"] else 0.2), end)
        length = action_end - begin
        speed = max(1.0, length / (MAX_ACTION + LEAD))
        parts.append({"from": begin, "to": action_end, "speed": speed, "step": i})
        if end > action_end:
            parts.append({"from": action_end, "to": end, "speed": 1.0, "step": i})
    out = 0.0
    for part in parts:
        part["out"] = out
        out += (part["to"] - part["from"]) / part["speed"]
    return parts


def ass_time(t: float) -> str:
    cs = int(round(t * 100))
    return f"{cs // 360000}:{cs // 6000 % 60:02d}:{cs // 100 % 60:02d}.{cs % 100:02d}"


def ass_document(width: int, height: int, events: list[str]) -> str:
    scale = height / 1080
    caption = round(38 * scale)
    title = round(64 * scale)
    return "\n".join(
        [
            "[Script Info]", "ScriptType: v4.00+", f"PlayResX: {width}", f"PlayResY: {height}",
            "WrapStyle: 0", "", "[V4+ Styles]",
            "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, "
            "BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, "
            "BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding",
            f"Style: Caption,Inter Medium,{caption},&H00FFFFFF,&H00FFFFFF,&H30141414,&H00000000,"
            f"0,0,0,0,100,100,0,0,3,{round(16 * scale)},0,2,40,40,{round(56 * scale)},1",
            f"Style: Title,Inter SemiBold,{title},&H00FFFFFF,&H00FFFFFF,&H30141414,&H00000000,"
            f"0,0,0,0,100,100,0,0,3,{round(28 * scale)},0,5,40,40,40,1",
            f"Style: Mark,Inter,10,&H00FFFFFF,&H00FFFFFF,&H00202020,&H00000000,"
            f"0,0,0,0,100,100,0,0,1,{max(1, round(2 * scale))},0,7,0,0,0,1",
            "", "[Events]",
            "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text",
            *events, "",
        ]
    )


def pointer_shape(scale: float) -> str:
    pts = [(0, 0), (0, 26), (7, 20), (12, 31), (16, 29), (11, 18), (19, 18)]
    p = [(round(x * scale * 1.3), round(y * scale * 1.3)) for x, y in pts]
    return "m {} {} ".format(*p[0]) + " ".join(f"l {x} {y}" for x, y in p[1:])


def ripple_shape(r: int) -> str:
    k = round(r * 0.5523)
    return (f"m 0 {-r} b {k} {-r} {r} {-k} {r} 0 b {r} {k} {k} {r} 0 {r} "
            f"b {-k} {r} {-r} {k} {-r} 0 b {-r} {-k} {-k} {-r} 0 {-r}")


def overlay_events(steps, parts, spots, title, size) -> list[str]:
    width, height = size
    scale = height / 1080
    dialogue = "Dialogue: {layer},{start},{end},{style},,0,0,0,,{text}"
    events = [dialogue.format(layer=2, start=ass_time(0), end=ass_time(parts[0]["out"] + INTRO),
                              style="Title", text=title)]
    first = {}
    last = {}
    for part in parts:
        if part["step"] is None:
            continue
        end = part["out"] + (part["to"] - part["from"]) / part["speed"]
        first.setdefault(part["step"], part)
        last[part["step"]] = end
    shape = pointer_shape(scale)
    pos = (width // 2, height // 2)
    shown = False
    visual_starts = []
    for i, step in enumerate(steps):
        if i not in first:
            continue
        begin, end = first[i]["out"], last[i]
        events.append(dialogue.format(layer=2, start=ass_time(begin), end=ass_time(end),
                                      style="Caption", text=step["caption"].replace("\n", " ")))
        spot = spots.get(i)
        if spot is None:
            continue
        visual_starts.append((begin, pos, spot))
        pos = spot
        shown = True
    if not shown:
        return events
    end_all = parts[-1]["out"] + (parts[-1]["to"] - parts[-1]["from"]) / parts[-1]["speed"]
    for k, (begin, start_pos, spot) in enumerate(visual_starts):
        arrive = begin + MOVE
        hold_end = visual_starts[k + 1][0] if k + 1 < len(visual_starts) else end_all
        (x1, y1), (x2, y2) = start_pos, spot
        events.append(dialogue.format(
            layer=4, start=ass_time(begin), end=ass_time(arrive), style="Mark",
            text=f"{{\\an7\\move({x1},{y1},{x2},{y2},0,{int(MOVE * 1000)})\\p1}}{shape}{{\\p0}}"))
        events.append(dialogue.format(
            layer=4, start=ass_time(arrive), end=ass_time(hold_end), style="Mark",
            text=f"{{\\an7\\pos({x2},{y2})\\p1}}{shape}{{\\p0}}"))
        for delay, size_from in ((0.0, 20), (0.15, 10)):
            r = round(34 * scale)
            t = arrive + delay
            events.append(dialogue.format(
                layer=3, start=ass_time(t), end=ass_time(t + 0.6), style="Mark",
                text=(f"{{\\an5\\pos({x2},{y2})\\1a&HFF&\\3c&HFFA040&\\bord{max(2, round(3 * scale))}"
                      f"\\fscx{size_from}\\fscy{size_from}\\t(0,600,\\fscx160\\fscy160\\3a&HFF&)\\p1}}"
                      f"{ripple_shape(r)}{{\\p0}}")))
    return events


def cut_filter(parts: list[dict]) -> str:
    chains = [
        f"[0:v]trim=start={p['from']:.3f}:end={p['to']:.3f},"
        f"setpts=(PTS-STARTPTS)/{p['speed']:.4f}[p{i}]"
        for i, p in enumerate(parts)
    ]
    joined = "".join(f"[p{i}]" for i in range(len(parts)))
    return ";".join(chains) + f";{joined}concat=n={len(parts)}:v=1:a=0,fps={OUT_FPS}[cut]"


def encode_mp4(args: list[str], out: Path, crf: int = 24) -> None:
    run(["ffmpeg", "-v", "error", "-y", *args, "-c:v", "libx264", "-preset", "slow",
         "-crf", str(crf), "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(out)])


def gif(source: Path, out: Path, width: int = 800, fps: int = 10) -> None:
    graph = (f"fps={fps},scale={width}:-1:flags=lanczos,split[a][b];"
             "[a]palettegen=max_colors=128:stats_mode=diff[p];"
             "[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle")
    run(["ffmpeg", "-v", "error", "-y", "-i", str(source), "-filter_complex", graph, str(out)])


def render_story(args) -> None:
    meta = json.loads(args.steps.read_text())
    steps = meta["steps"]
    start = meta["capture_start"]
    width, height, duration = probe(args.raw)
    parts = plan(steps, start, duration)

    spots = {}
    for i, step in enumerate(steps):
        if not step["visual"]:
            continue
        t0, t1 = step["t0"] - start, step["t1"] - start
        nxt = steps[i + 1]["t0"] - start if i + 1 < len(steps) else t1 + step["hold"]
        before = frame(args.raw, t0 - 0.3, (width, height))
        after = frame(args.raw, min(t1 + SETTLE, nxt - LEAD - 0.1), (width, height))
        spot = hotspot(before, after)
        if spot:
            spots[i] = spot

    work = args.raw.parent
    ass = work / f"{args.name}.ass"
    ass.write_text(ass_document(width, height, overlay_events(steps, parts, spots, args.title,
                                                             (width, height))))
    clean = work / f"{args.name}.clean.mp4"
    encode_mp4(["-i", str(args.raw), "-filter_complex", cut_filter(parts), "-map", "[cut]"],
               clean, crf=18)
    args.out.mkdir(parents=True, exist_ok=True)
    mp4 = args.out / f"{args.name}.mp4"
    encode_mp4(["-i", str(clean), "-vf", f"ass={ass}:fontsdir={FONTS}"], mp4)
    gif(mp4, args.out / f"{args.name}.gif")
    (work / f"{args.name}.spots.json").write_text(json.dumps(spots))


def render_hero(args) -> None:
    """A captionless loop: the schematic drawn fast, then the routed board,
    fading back to the empty sheet so the end meets the start."""
    sch_len = 6.0
    board_len = 7.0
    fade = 0.8
    _, _, sch_duration = probe(args.schematic)
    width = args.width
    graph = (
        f"[0:v]setpts=PTS*{sch_len / sch_duration:.5f},fps=30,scale={width}:-2:flags=lanczos,"
        f"trim=duration={sch_len},setpts=PTS-STARTPTS,split[s][first];"
        f"[first]trim=end_frame=1,loop=loop={int((fade + 0.2) * 30)}:size=1,setpts=N/30/TB[still];"
        f"[1:v]fps=30,scale={width}:-2:flags=lanczos,trim=duration={board_len},"
        f"setpts=PTS-STARTPTS[b];"
        f"[s][b]xfade=transition=fade:duration={fade}:offset={sch_len - fade}[sb];"
        f"[sb][still]xfade=transition=fade:duration={fade}:offset={sch_len + board_len - 2 * fade}[v]"
    )
    args.out.mkdir(parents=True, exist_ok=True)
    mp4 = args.out / "hero.mp4"
    common = ["-i", str(args.schematic), "-i", str(args.board), "-filter_complex", graph,
              "-map", "[v]", "-an"]
    encode_mp4(common, mp4, crf=30)
    webm = args.out / "hero.webm"
    run(["ffmpeg", "-v", "error", "-y", "-i", str(mp4), "-c:v", "libvpx-vp9", "-b:v", "0",
         "-crf", "42", "-row-mt", "1", "-an", str(webm)])
    if webm.stat().st_size >= mp4.stat().st_size:
        webm.unlink()
    run(["ffmpeg", "-v", "error", "-y", "-i", str(mp4), "-frames:v", "1", "-c:v", "libwebp",
         "-quality", "85", str(args.out / "hero-poster.webp")])


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="mode", required=True)
    story = sub.add_parser("story")
    story.add_argument("--raw", type=Path, required=True)
    story.add_argument("--steps", type=Path, required=True)
    story.add_argument("--title", required=True)
    story.add_argument("--out", type=Path, required=True)
    story.add_argument("--name", required=True)
    hero = sub.add_parser("hero")
    hero.add_argument("--schematic", type=Path, required=True)
    hero.add_argument("--board", type=Path, required=True)
    hero.add_argument("--out", type=Path, required=True)
    hero.add_argument("--width", type=int, default=1600)
    args = parser.parse_args()
    render_story(args) if args.mode == "story" else render_hero(args)


if __name__ == "__main__":
    main()
