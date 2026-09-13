#!/usr/bin/env python3
"""Builds the macOS icon out of the one piece of source artwork.

    dev/make-macos-icon.py

macOS draws the shape of an application icon itself now. Since macOS 26 every
icon goes into the system's own rounded square, and artwork that brings its own
rounded square arrives inside a second one — which is what a thick white frame
around a shrunken amber tile in the Dock turns out to be. So the icon this
writes is **full bleed**: the gradient runs edge to edge and the corners are
the system's business.

The same source with its own rounded rectangle is still what Windows and Linux
get, because nothing masks an icon there. One drawing, two shapes, and the
difference made here rather than kept as a second file that would drift.

Only useful on a Mac: `sips` and `iconutil` are Apple's, and they are the ones
that write an icns the system is certain to read. Everything it produces is
committed, so no build ever needs to run this.
"""
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
SOURCE = ROOT / "assets" / "icon.svg"
TARGET = ROOT / "src-tauri" / "icons" / "icon.icns"

# The rounded rectangle the artwork draws for itself, and what it becomes when
# the system is the one rounding the corners.
OWN_SHAPE = re.compile(r'<rect\s+x="16"\s+y="16"\s+width="224"\s+height="224"\s+rx="56"')
FULL_BLEED = '<rect x="0" y="0" width="256" height="256"'

# What an icns needs, in the pairs `iconutil` expects.
SIZES = [
    (16, "icon_16x16"),
    (32, "icon_16x16@2x"),
    (32, "icon_32x32"),
    (64, "icon_32x32@2x"),
    (128, "icon_128x128"),
    (256, "icon_128x128@2x"),
    (256, "icon_256x256"),
    (512, "icon_256x256@2x"),
    (512, "icon_512x512"),
    (1024, "icon_512x512@2x"),
]


def main() -> int:
    if sys.platform != "darwin":
        print("this needs sips and iconutil, which only macOS has", file=sys.stderr)
        return 1

    svg = SOURCE.read_text(encoding="utf-8")
    full, replaced = OWN_SHAPE.subn(FULL_BLEED, svg)
    if replaced != 1:
        print(
            f"{SOURCE} no longer draws the rounded rectangle this knows how to "
            "take away — look at it before trusting what comes out",
            file=sys.stderr,
        )
        return 1

    with tempfile.TemporaryDirectory() as scratch:
        scratch = Path(scratch)
        edge_to_edge = scratch / "icon-full.svg"
        edge_to_edge.write_text(full, encoding="utf-8")

        # Rendered once at the largest size and scaled down from there: every
        # smaller image is then the same drawing, not ten separate renderings
        # that might differ at the edges.
        biggest = scratch / "icon-full.png"
        subprocess.run(
            [str(HERE / "render-png.py"), str(edge_to_edge), str(biggest), "1024", "1024"],
            check=True,
        )

        iconset = scratch / "AmberBeam.iconset"
        iconset.mkdir()
        for pixels, name in SIZES:
            subprocess.run(
                ["sips", "-z", str(pixels), str(pixels), str(biggest), "--out", str(iconset / f"{name}.png")],
                check=True,
                capture_output=True,
            )

        subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(TARGET)], check=True)

    print(f"wrote {TARGET.relative_to(ROOT)} ({TARGET.stat().st_size} bytes), full bleed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
