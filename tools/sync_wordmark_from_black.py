"""Mirror the <path id="text11"> 'd' attribute from signex-logo-black.svg into
signex-logo.svg and signex-logo-white.svg, preserving each file's own fill.

The black variant is the user's manually-tuned reference (mark-to-wordmark gap).
Run this after adjusting the black SVG to keep all three logos in sync.

Usage:
    py tools/sync_wordmark_from_black.py
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BRAND_DIR = REPO_ROOT / "crates" / "signex-app" / "assets" / "brand"
WEBSITE_DIR = REPO_ROOT.parent / "signex-website"
REFERENCE = BRAND_DIR / "signex-logo-black.svg"
TARGETS = [
    BRAND_DIR / "signex-logo.svg",
    BRAND_DIR / "signex-logo-white.svg",
    WEBSITE_DIR / "signex-logo.svg",
]

TEXT11_BLOCK = re.compile(
    r'(<path\b(?:(?!</?path\b).)*?\bid="text11"(?:(?!</?path\b).)*?/?>)',
    re.DOTALL,
)
D_ATTR = re.compile(r'\bd="([^"]*)"')


def extract_text11_d(svg_text: str) -> str:
    m = TEXT11_BLOCK.search(svg_text)
    if not m:
        raise RuntimeError('text11 path not found')
    md = D_ATTR.search(m.group(1))
    if not md:
        raise RuntimeError('d= not found in text11 path')
    return md.group(1)


def replace_text11_d(svg_text: str, new_d: str) -> str:
    m = TEXT11_BLOCK.search(svg_text)
    if not m:
        raise RuntimeError('text11 path not found in target')
    old_block = m.group(1)
    if not D_ATTR.search(old_block):
        raise RuntimeError('d= not found in target text11 path')
    # lambda avoids re.sub's backreference interpretation of \g<...>, \1 etc.
    new_block = D_ATTR.sub(lambda _m: f'd="{new_d}"', old_block, count=1)
    return svg_text[: m.start()] + new_block + svg_text[m.end() :]


def main() -> int:
    if not REFERENCE.is_file():
        print(f'ERROR: reference missing: {REFERENCE}', file=sys.stderr)
        return 1
    new_d = extract_text11_d(REFERENCE.read_text(encoding='utf-8'))
    print(f'reference d: {len(new_d)} chars from {REFERENCE.name}')

    for target in TARGETS:
        if not target.is_file():
            print(f'skip (missing): {target}')
            continue
        text = target.read_text(encoding='utf-8')
        updated = replace_text11_d(text, new_d)
        target.write_text(updated, encoding='utf-8')
        try:
            rel = target.relative_to(REPO_ROOT)
        except ValueError:
            rel = target
        print(f'updated: {rel}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
