"""Build vendor/iced_aw/font.ttf, the six icon glyphs iced_aw draws.

The glyphs are drawn here from straight strokes, so the font is original
work of the Signex project (Apache-2.0, like the rest of the repository).
It replaces the Fontello build the crate shipped, whose glyph source and
licence could not be confirmed (#2031).

The codepoints (U+E800..U+E805), the family name "iced_aw" and the advance
widths match the old font, so `iced_aw::iced_aw_font` and the widgets are
unchanged.

    python vendor/iced_aw/tools/build_font.py

Needs fontTools (`pip install fonttools`).
"""

import math
import pathlib

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

UPEM = 1000
ASCENT, DESCENT = 850, -150
STROKE = 180  # stroke width in font units
# Fixed head-table timestamps (seconds since 1904-01-01), so a rebuild is
# byte-identical. Built with fontTools 4.67.0.
TIMESTAMP = 3845404800  # a fixed date in late 2025; any fixed value works


def stroke(points, width=STROKE):
    """Outline of a polyline `points` drawn `width` wide, mitred joins."""
    half = width / 2
    n = len(points)

    def normal(a, b):
        dx, dy = b[0] - a[0], b[1] - a[1]
        length = math.hypot(dx, dy)
        return -dy / length, dx / length

    left, right = [], []
    for i, p in enumerate(points):
        if i == 0:
            nx, ny = normal(points[0], points[1])
            scale = 1.0
        elif i == n - 1:
            nx, ny = normal(points[-2], points[-1])
            scale = 1.0
        else:
            n1 = normal(points[i - 1], p)
            n2 = normal(p, points[i + 1])
            mx, my = n1[0] + n2[0], n1[1] + n2[1]
            length = math.hypot(mx, my)
            nx, ny = mx / length, my / length
            scale = 1.0 / max(nx * n1[0] + ny * n1[1], 0.3)
        left.append((p[0] + nx * half * scale, p[1] + ny * half * scale))
        right.append((p[0] - nx * half * scale, p[1] - ny * half * scale))
    return left + right[::-1]


def clockwise(contour):
    area = sum(
        a[0] * b[1] - b[0] * a[1]
        for a, b in zip(contour, contour[1:] + contour[:1])
    )
    return contour if area < 0 else contour[::-1]


def glyph(contours):
    pen = TTGlyphPen(None)
    for contour in contours:
        contour = clockwise([(round(x), round(y)) for x, y in contour])
        pen.moveTo(contour[0])
        for point in contour[1:]:
            pen.lineTo(point)
        pen.closePath()
    return pen.glyph()


# name: (codepoint, advance width, strokes)
GLYPHS = {
    "cancel": (0xE800, 785, [[(130, 50), (655, 575)], [(130, 575), (655, 50)]]),
    "down-open": (0xE801, 1000, [[(110, 500), (500, 60), (890, 500)]]),
    "left-open": (0xE802, 714, [[(560, 780), (170, 386), (560, -8)]]),
    "right-open": (0xE803, 714, [[(154, 780), (544, 386), (154, -8)]]),
    "up-open": (0xE804, 1000, [[(110, 60), (500, 500), (890, 60)]]),
    "ok": (0xE805, 1000, [[(110, 330), (380, 70), (890, 600)]]),
}


def build(out):
    order = [".notdef"] + list(GLYPHS)
    fb = FontBuilder(UPEM, isTTF=True)
    fb.setupGlyphOrder(order)
    fb.setupCharacterMap({cp: name for name, (cp, _, _) in GLYPHS.items()})
    glyphs = {".notdef": TTGlyphPen(None).glyph()}
    metrics = {".notdef": (500, 0)}
    for name, (_, advance, strokes) in GLYPHS.items():
        g = glyph([stroke(s) for s in strokes])
        glyphs[name] = g
        metrics[name] = (advance, 0)
    fb.setupGlyf(glyphs)
    for name, g in glyphs.items():
        g.recalcBounds(fb.font["glyf"])
        metrics[name] = (metrics[name][0], getattr(g, "xMin", 0))
    fb.setupHorizontalMetrics(metrics)
    fb.setupHorizontalHeader(ascent=ASCENT, descent=DESCENT)
    fb.setupNameTable(
        {
            "copyright": "Copyright 2026 Alp Lab AB",
            "familyName": "iced_aw",
            "styleName": "Regular",
            "uniqueFontIdentifier": "iced_aw-signex",
            "fullName": "iced_aw",
            "psName": "iced_aw",
            "version": "Version 2.0",
            "licenseDescription": "Apache-2.0",
            "licenseInfoURL": "https://www.apache.org/licenses/LICENSE-2.0",
        }
    )
    fb.setupOS2(
        sTypoAscender=ASCENT,
        sTypoDescender=DESCENT,
        usWinAscent=ASCENT,
        usWinDescent=-DESCENT,
        fsType=0,  # installable embedding, like the font it replaces
    )
    fb.setupPost()
    fb.updateHead(created=TIMESTAMP, modified=TIMESTAMP)
    fb.font.recalcTimestamp = False
    fb.save(out)


if __name__ == "__main__":
    build(pathlib.Path(__file__).resolve().parent.parent / "font.ttf")
