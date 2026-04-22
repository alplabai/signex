"""Pure-Python fallback for installer/build-icons.sh.

Renders signex-mark.svg to platform icon bitmaps using resvg_py (no native
Cairo/ImageMagick/Inkscape dependency). Produces the same outputs as the
bash script:
    installer/windows/signex.ico
    installer/macos/Signex.icns
    installer/linux/signex-{128,256,512}.png
    crates/signex-app/assets/brand/generated/signex-{256,512}.png
    crates/signex-app/assets/brand/generated/signex.ico

Usage:
    py tools/build_icons.py

Requires: pip install resvg-py pillow
"""
from __future__ import annotations

import shutil
import struct
import sys
from io import BytesIO
from pathlib import Path

from PIL import Image
from resvg_py import svg_to_bytes

REPO_ROOT = Path(__file__).resolve().parent.parent
SRC = REPO_ROOT / "crates" / "signex-app" / "assets" / "brand" / "signex-mark.svg"
WIN_DIR = REPO_ROOT / "installer" / "windows"
MAC_DIR = REPO_ROOT / "installer" / "macos"
LIN_DIR = REPO_ROOT / "installer" / "linux"
GEN_DIR = REPO_ROOT / "crates" / "signex-app" / "assets" / "brand" / "generated"

SIZES = [16, 32, 48, 64, 128, 256, 512, 1024]
ICO_SIZES = [16, 32, 48, 64, 128, 256]


def render(size: int) -> bytes:
    """Rasterize signex-mark.svg to a PNG at size×size. Returns PNG bytes."""
    # resvg_py returns a list of ints (byte values). Convert to bytes.
    data = svg_to_bytes(svg_path=str(SRC), width=size, height=size)
    return bytes(data)


def build_ico(pngs: dict[int, bytes], out: Path) -> None:
    """Bundle multiple native-rendered PNGs into a multi-size .ico."""
    entries = []
    for sz in ICO_SIZES:
        data = pngs[sz]
        img = Image.open(BytesIO(data))
        w, h = img.size
        entries.append((w, h, data))

    header = struct.pack("<HHH", 0, 1, len(entries))
    dir_entries = b""
    image_blob = b""
    offset = 6 + 16 * len(entries)
    for w, h, data in entries:
        w_b = 0 if w >= 256 else w
        h_b = 0 if h >= 256 else h
        dir_entries += struct.pack(
            "<BBBBHHII",
            w_b, h_b, 0, 0,
            1, 32,
            len(data), offset,
        )
        image_blob += data
        offset += len(data)

    out.write_bytes(header + dir_entries + image_blob)


def build_icns(pngs: dict[int, bytes], out: Path) -> None:
    """Use Pillow to write a .icns containing common Apple sizes."""
    imgs = [Image.open(BytesIO(pngs[sz])).convert("RGBA")
            for sz in [16, 32, 64, 128, 256, 512, 1024]]
    imgs[0].save(out, format="ICNS", append_images=imgs[1:])


def main() -> int:
    if not SRC.is_file():
        print(f"ERROR: source missing: {SRC}", file=sys.stderr)
        return 1
    for d in (WIN_DIR, MAC_DIR, LIN_DIR, GEN_DIR):
        d.mkdir(parents=True, exist_ok=True)

    print(f"rasterizing {SRC.name} via resvg_py")
    pngs: dict[int, bytes] = {}
    for sz in SIZES:
        print(f"  {sz}x{sz}")
        pngs[sz] = render(sz)

    # Canonical PNGs for runtime embedding / Linux desktop files.
    (GEN_DIR / "signex-256.png").write_bytes(pngs[256])
    (GEN_DIR / "signex-512.png").write_bytes(pngs[512])
    (LIN_DIR / "signex-128.png").write_bytes(pngs[128])
    (LIN_DIR / "signex-256.png").write_bytes(pngs[256])
    (LIN_DIR / "signex-512.png").write_bytes(pngs[512])

    # Windows ICO.
    ico_path = WIN_DIR / "signex.ico"
    print(f"writing {ico_path.relative_to(REPO_ROOT)}")
    build_ico(pngs, ico_path)
    shutil.copy2(ico_path, GEN_DIR / "signex.ico")

    # macOS ICNS.
    icns_path = MAC_DIR / "Signex.icns"
    print(f"writing {icns_path.relative_to(REPO_ROOT)}")
    build_icns(pngs, icns_path)

    print("done.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
