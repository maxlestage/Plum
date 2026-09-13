#!/usr/bin/env python3
"""Draw Plum's app icon and write it into the asset catalogue.

The icon is generated rather than committed as an opaque binary: the palette
lives in `PlumTheme.swift`, and an icon that drifts from the brand is a bug
nobody notices until the App Store listing. Re-run it after a palette change.

Written with zlib and struct alone — no image library — so it runs anywhere
Python does, CI included.

    python3 Scripts/generate_appicon.py
"""

from __future__ import annotations

import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "Plum" / "Resources" / "Assets.xcassets" / "AppIcon.appiconset" / "AppIcon.png"

SIZE = 1024
SUPERSAMPLE = 3  # 3×3 samples per pixel: enough to keep the heart's cusp clean.

# PlumTheme.Palette, kept in sync by hand — there are only three.
BLUSH = (0xE8, 0x60, 0x8C)
PLUM = (0x6B, 0x2D, 0x5C)
PLUM_DEEP = (0x40, 0x18, 0x3A)
WHITE = (0xFF, 0xF7, 0xF4)


def lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def background(u: float, v: float) -> tuple[float, float, float]:
    """Diagonal blush → plum → deep plum, matching `warmGradient`."""
    t = max(0.0, min(1.0, (u + v) / 2))
    if t < 0.6:
        local = t / 0.6
        return tuple(lerp(BLUSH[i], PLUM[i], local) for i in range(3))
    local = (t - 0.6) / 0.4
    return tuple(lerp(PLUM[i], PLUM_DEEP[i], local) for i in range(3))


def inside_heart(x: float, y: float) -> bool:
    """The classic implicit heart: (x² + y² − 1)³ − x²y³ ≤ 0."""
    term = x * x + y * y - 1
    return term * term * term - x * x * y * y * y <= 0


def render() -> bytearray:
    """One RGB row per scanline, each prefixed with PNG filter type 0."""
    rows = bytearray()
    span = SIZE * SUPERSAMPLE
    samples = SUPERSAMPLE * SUPERSAMPLE
    # The heart occupies the middle ~62% of the canvas, nudged up a little so
    # the lobes, not the point, sit on the optical centre.
    scale = SIZE * 0.31
    centre_x = SIZE / 2
    centre_y = SIZE * 0.47

    for pixel_y in range(SIZE):
        rows.append(0)
        row = bytearray()
        for pixel_x in range(SIZE):
            red = green = blue = 0.0
            for sub_y in range(SUPERSAMPLE):
                sample_y = pixel_y + (sub_y + 0.5) / SUPERSAMPLE
                heart_y = -(sample_y - centre_y) / scale
                for sub_x in range(SUPERSAMPLE):
                    sample_x = pixel_x + (sub_x + 0.5) / SUPERSAMPLE
                    heart_x = (sample_x - centre_x) / scale

                    if inside_heart(heart_x, heart_y):
                        colour = WHITE
                    else:
                        colour = background(sample_x / SIZE, sample_y / SIZE)
                    red += colour[0]
                    green += colour[1]
                    blue += colour[2]
            row.append(int(red / samples + 0.5))
            row.append(int(green / samples + 0.5))
            row.append(int(blue / samples + 0.5))
        rows.extend(row)
    return rows


def chunk(tag: bytes, payload: bytes) -> bytes:
    return (
        struct.pack(">I", len(payload))
        + tag
        + payload
        + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)
    )


def write_png(path: Path, raw: bytes) -> None:
    # Colour type 2 (truecolour, no alpha): an iOS app icon must be opaque.
    header = struct.pack(">IIBBBBB", SIZE, SIZE, 8, 2, 0, 0, 0)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def main() -> None:
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    write_png(OUTPUT, render())
    print(f"{OUTPUT.relative_to(ROOT)} — {SIZE}×{SIZE}, {OUTPUT.stat().st_size // 1024} Ko")


if __name__ == "__main__":
    main()
