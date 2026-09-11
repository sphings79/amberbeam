#!/usr/bin/env python3
"""Renders an SVG to a PNG of an exact size, using only what macOS ships with.

GitHub wants PNG for the social preview and Tauri wants PNG for the icon, but
this repository carries no image library and is not going to grow one for two
files. QuickLook can rasterise SVG, with one quirk: it always draws into a
square canvas. So the graphic is placed into a square of its own, rendered at
full size, and cropped back — which stays sharp, unlike scaling a smaller
rendering up.

    dev/render-png.py assets/icon.svg assets/icon.png 1024 1024
"""
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

SVG_TAG = re.compile(r"<svg\b[^>]*>", re.IGNORECASE)
SIZE_ATTR = re.compile(r'\s(?:width|height|x|y)="[^"]*"', re.IGNORECASE)


def place(svg: str, x: int, y: int, width: int, height: int) -> str:
    """The graphic as a nested <svg> at a fixed position and size."""
    svg = re.sub(r"<\?xml[^>]*\?>", "", svg).strip()
    match = SVG_TAG.search(svg)
    if not match:
        raise SystemExit("input holds no <svg> element")
    tag = SIZE_ATTR.sub("", match.group(0))
    tag = tag[:-1].rstrip() + f' x="{x}" y="{y}" width="{width}" height="{height}">'
    return svg[: match.start()] + tag + svg[match.end() :]


def main() -> None:
    if len(sys.argv) != 5:
        raise SystemExit("usage: render-png.py <input.svg> <output.png> <width> <height>")
    source = Path(sys.argv[1])
    target = Path(sys.argv[2])
    width, height = int(sys.argv[3]), int(sys.argv[4])
    side = max(width, height)

    inner = place(source.read_text(), (side - width) // 2, (side - height) // 2, width, height)
    square = (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{side}" height="{side}" '
        f'viewBox="0 0 {side} {side}">{inner}</svg>'
    )

    with tempfile.TemporaryDirectory() as work:
        wrapper = Path(work, "square.svg")
        wrapper.write_text(square)
        subprocess.run(
            ["qlmanage", "-t", "-s", str(side), "-o", work, str(wrapper)],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        rendered = Path(work, "square.svg.png")
        if not rendered.exists():
            raise SystemExit(f"QuickLook produced nothing for {source}")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(rendered, target)

    if (width, height) != (side, side):
        subprocess.run(
            ["sips", "--cropToHeightWidth", str(height), str(width), str(target)],
            check=True,
            stdout=subprocess.DEVNULL,
        )
    print(f"wrote {target} at {width}x{height}")


if __name__ == "__main__":
    main()
