"""Regenerate the 'signex' wordmark path in brand SVGs from Panton-Bold.ttf.

The three logo SVGs (signex-logo.svg, signex-logo-white.svg, signex-logo-black.svg)
embed a <path id="text11"> whose 'd' attribute is the outlined 'signex' wordmark.
When that path was originally exported from Inkscape, Panton was not installed
on the conversion machine, so the outlines came from a fallback font. This script
rebuilds the 'd' attribute by reading Panton-Bold.ttf directly with fontTools,
guaranteeing correct glyph geometry regardless of installed system fonts.

Usage:
    py tools/regen_wordmark_path.py

Geometry (matches existing wordmark placement):
    font-size    : 300 px
    letter-spacing: -7 px (per existing style)
    baseline y   : 330
    left x       : 496.75 (origin of the first glyph's bbox)
    fill         : preserved from existing style attribute
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.ttLib import TTFont

REPO_ROOT = Path(__file__).resolve().parent.parent
FONT_PATH = REPO_ROOT.parent / "signex-website" / "fonts" / "Panton-Bold.ttf"
BRAND_DIR = REPO_ROOT / "crates" / "signex-app" / "assets" / "brand"

WORDMARK = "signex"
FONT_SIZE = 300.0
LETTER_SPACING = -7.0
BASELINE_Y = 330.0
LEFT_X = 496.75
TARGETS = ["signex-logo.svg", "signex-logo-white.svg", "signex-logo-black.svg"]


def build_wordmark_path_d(font: TTFont) -> str:
    """Trace 'signex' through Panton-Bold at FONT_SIZE and return SVG path 'd'."""
    cmap = font.getBestCmap()
    glyph_set = font.getGlyphSet()
    units_per_em = font["head"].unitsPerEm
    scale = FONT_SIZE / units_per_em

    pen = SVGPathPen(glyph_set)
    advance_x = 0.0

    for ch in WORDMARK:
        if ord(ch) not in cmap:
            raise RuntimeError(f"Glyph for {ch!r} missing from Panton-Bold")
        glyph_name = cmap[ord(ch)]
        glyph = glyph_set[glyph_name]

        # SVG y-axis points down; glyph y-axis points up. Flip with transform.
        # We emit: translate(x, baseline) scale(s, -s) then draw.
        # SVGPathPen doesn't support transforms, so we apply manually via
        # TransformPen. Simpler: use DecomposingRecordingPen is overkill — just
        # wrap with TransformPen.
        from fontTools.pens.transformPen import TransformPen

        tx = LEFT_X + advance_x
        ty = BASELINE_Y
        t = (scale, 0.0, 0.0, -scale, tx, ty)
        tpen = TransformPen(pen, t)
        glyph.draw(tpen)

        advance_x += glyph.width * scale + LETTER_SPACING

    return pen.getCommands()


def replace_path_d(svg_text: str, new_d: str) -> str:
    """Replace the 'd' attribute of <path id="text11"> with new_d."""
    # Match: any attributes, then d="...", then any attributes, then id="text11"
    # or id="text11" first then d="...". We must handle both orderings.
    # Strategy: find the <path ...> block that contains id="text11", then
    # substitute its d="..." inside that block only.
    path_block = re.compile(
        r'(<path\b[^>]*?\bid="text11"[^>]*?/?>)', re.DOTALL
    )
    m = path_block.search(svg_text)
    if not m:
        # Try the other ordering: id may come after d in the element.
        path_block_alt = re.compile(
            r'(<path\b(?:(?!</?path\b).)*?\bid="text11"(?:(?!</?path\b).)*?/?>)',
            re.DOTALL,
        )
        m = path_block_alt.search(svg_text)
    if not m:
        raise RuntimeError('Could not locate <path id="text11"> in SVG')

    old_block = m.group(1)
    new_block = re.sub(
        r'\bd="[^"]*"',
        f'd="{new_d}"',
        old_block,
        count=1,
    )
    if new_block == old_block:
        raise RuntimeError('Failed to substitute d="..." inside text11 path')
    return svg_text[: m.start()] + new_block + svg_text[m.end() :]


def main() -> int:
    if not FONT_PATH.is_file():
        print(f"ERROR: Panton-Bold.ttf not found at {FONT_PATH}", file=sys.stderr)
        return 1

    font = TTFont(str(FONT_PATH))
    new_d = build_wordmark_path_d(font)
    print(f"Generated path d: {len(new_d)} chars")

    for name in TARGETS:
        svg_path = BRAND_DIR / name
        if not svg_path.is_file():
            print(f"skip (missing): {svg_path}")
            continue
        text = svg_path.read_text(encoding="utf-8")
        updated = replace_path_d(text, new_d)
        svg_path.write_text(updated, encoding="utf-8")
        print(f"updated: {svg_path.relative_to(REPO_ROOT)}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
