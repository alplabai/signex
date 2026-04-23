#!/usr/bin/env python
"""
Rasterize the Signex wordmark SVGs into PNGs at 1x / 2x / 3x DPI tiers.

The app menu bar displays the wordmark at 96x31 logical pixels. On a
100%-scale monitor that's 96x31 device pixels; at 200% scale winit
reports a scale factor of 2.0 and the same logical box is 192x62 device
pixels. Rasterizing fresh PNGs for each tier lets us hand iced an asset
that is 1:1 with the target device-pixel count, which sidesteps resvg's
unhinted path-text blur we'd otherwise get when iced rasterizes the SVG
down to ~31 px tall.

Outputs (committed under assets/brand/generated/):
    wordmark-white-1x.png   96x31
    wordmark-white-2x.png  192x62
    wordmark-white-3x.png  288x93
    wordmark-black-1x.png   96x31
    wordmark-black-2x.png  192x62
    wordmark-black-3x.png  288x93

Requires: resvg_py (pip install resvg-py). resvg is the same rasterizer
Firefox / Servo use, so it handles the logo's linear gradients cleanly.
"""
from __future__ import annotations

import sys
from pathlib import Path

try:
    from resvg_py import svg_to_bytes
except ImportError:
    print("error: resvg_py not installed. Run: pip install resvg-py", file=sys.stderr)
    sys.exit(1)


REPO_ROOT = Path(__file__).resolve().parent.parent
BRAND_DIR = REPO_ROOT / "crates" / "signex-app" / "assets" / "brand"
OUT_DIR = BRAND_DIR / "generated"

# The logical display size in menu_bar.rs. Keep in sync if that ever changes.
BASE_W, BASE_H = 96, 31
TIERS = (1, 2, 3)
VARIANTS = (
    ("white", "signex-logo-white.svg"),
    ("black", "signex-logo-black.svg"),
)


def main() -> int:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for color, svg_name in VARIANTS:
        src = BRAND_DIR / svg_name
        if not src.exists():
            print(f"error: source not found: {src}", file=sys.stderr)
            return 1
        svg_text = src.read_text(encoding="utf-8")
        for tier in TIERS:
            w, h = BASE_W * tier, BASE_H * tier
            out_path = OUT_DIR / f"wordmark-{color}-{tier}x.png"
            png = svg_to_bytes(
                svg_string=svg_text,
                width=w,
                height=h,
                # Pin these so output is deterministic across resvg_py
                # versions. Accepted values are the SVG spec strings.
                shape_rendering="geometric_precision",
                text_rendering="geometric_precision",
                image_rendering="optimize_quality",
            )
            out_path.write_bytes(bytes(png))
            print(f"  {out_path.relative_to(REPO_ROOT)}  {w}x{h}  {len(png):>6} bytes")
    print("done.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
