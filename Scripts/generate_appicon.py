#!/usr/bin/env python3
"""Draw Plum's app icon and write it into the asset catalogue.

The icon is generated rather than committed as an opaque binary: the palette
lives in `PlumTheme.swift`, and an icon that drifts from the brand is a bug
nobody notices until the App Store listing. Re-run it after a palette change.

Le dessin — une prune, son sillon, sa queue — est le même que celui de
`web/public/favicon.svg` et de `web/src/components/PlumMark.tsx`, exprimé dans
le même repère de 64 unités et avec les mêmes nombres. Les trois fichiers sont
trois rendus d'un seul dessin ; changer l'un sans les autres les fait diverger
sans que rien ne le signale.

Written with zlib and struct alone — no image library — so it runs anywhere
Python does, CI included.

    python3 Scripts/generate_appicon.py
"""

from __future__ import annotations

import math
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "Plum" / "Resources" / "Assets.xcassets" / "AppIcon.appiconset" / "AppIcon.png"

SIZE = 1024
SUPERSAMPLE = 3  # 3×3 samples per pixel: enough to keep the groove's ends clean.

# Le repère du dessin, partagé avec les deux SVG.
DESIGN = 64.0

# Le fruit : centre et rayons.
FRUIT_X, FRUIT_Y, FRUIT_RX, FRUIT_RY = 32.0, 35.5, 17.0, 18.0

# Le sillon, un arc de cercle. Le SVG l'écrit « M 30 21 A 36.54 36.54 0 0 0
# 30 50 » ; ce sont les mêmes bouts et le même rayon, plus le centre que la
# commande `A` laisse implicite.
GROOVE_X, GROOVE_Y, GROOVE_R = 63.54, 35.5, 36.54
GROOVE_ENDS = ((30.0, 21.0), (30.0, 50.0))
GROOVE_HALF = 1.6  # stroke-width 3.2

# La queue, un segment aux bouts arrondis. Elle part de *l'intérieur* du
# fruit : posée sur le bord, elle se détache et flotte.
STEM_A, STEM_B, STEM_HALF = (33.0, 20.0), (39.5, 11.0), 1.7

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


# Le rapport qui dit si un point fait face à l'arc ou à l'un de ses bouts.
# Dérivé de la géométrie plutôt que posé à la main : une constante recopiée
# survivrait à un changement de rayon en donnant un sillon tordu.
_GROOVE_SLOPE = abs(GROOVE_ENDS[0][1] - GROOVE_Y) / abs(GROOVE_ENDS[0][0] - GROOVE_X)


def distance_to_groove(x: float, y: float) -> float:
    """Distance au sillon, bouts arrondis compris.

    L'arc est symétrique autour de son horizontale et le fruit tient
    entièrement à gauche de son centre : savoir si l'on est en face de l'arc
    ou au-delà d'un bout se lit sur un simple rapport, sans passer par un
    angle et sans se heurter à la coupure de `atan2`.
    """
    dx = x - GROOVE_X
    dy = y - GROOVE_Y
    if dx < 0 and abs(dy) <= _GROOVE_SLOPE * (-dx):
        return abs(math.hypot(dx, dy) - GROOVE_R)

    (ax, ay), (bx, by) = GROOVE_ENDS
    return min(math.hypot(x - ax, y - ay), math.hypot(x - bx, y - by))


def distance_to_stem(x: float, y: float) -> float:
    (ax, ay), (bx, by) = STEM_A, STEM_B
    vx, vy = bx - ax, by - ay
    t = ((x - ax) * vx + (y - ay) * vy) / (vx * vx + vy * vy)
    t = 0.0 if t < 0.0 else (1.0 if t > 1.0 else t)
    return math.hypot(x - (ax + t * vx), y - (ay + t * vy))


def inside_plum(x: float, y: float) -> bool:
    """Le fruit et sa queue, moins le sillon — en unités de dessin."""
    ex = (x - FRUIT_X) / FRUIT_RX
    ey = (y - FRUIT_Y) / FRUIT_RY
    if ex * ex + ey * ey > 1.0 and distance_to_stem(x, y) > STEM_HALF:
        return False
    return distance_to_groove(x, y) > GROOVE_HALF


def render() -> bytearray:
    """One RGB row per scanline, each prefixed with PNG filter type 0."""
    rows = bytearray()
    samples = SUPERSAMPLE * SUPERSAMPLE
    to_design = DESIGN / SIZE

    # La boîte du dessin, en pixels. Neuf échantillons sur dix tombent
    # dehors : les écarter d'une comparaison plutôt que de les mesurer divise
    # le temps de rendu par trois, et ce script tourne à la main.
    left = int((min(FRUIT_X - FRUIT_RX, STEM_A[0], STEM_B[0]) - 2) / to_design)
    right = int((max(FRUIT_X + FRUIT_RX, STEM_A[0], STEM_B[0]) + 2) / to_design) + 1
    top = int((min(FRUIT_Y - FRUIT_RY, STEM_A[1], STEM_B[1]) - 2) / to_design)
    bottom = int((max(FRUIT_Y + FRUIT_RY, STEM_A[1], STEM_B[1]) + 2) / to_design) + 1

    for pixel_y in range(SIZE):
        rows.append(0)
        row = bytearray()
        near = top <= pixel_y <= bottom
        for pixel_x in range(SIZE):
            red = green = blue = 0.0
            figure_possible = near and left <= pixel_x <= right
            for sub_y in range(SUPERSAMPLE):
                sample_y = pixel_y + (sub_y + 0.5) / SUPERSAMPLE
                for sub_x in range(SUPERSAMPLE):
                    sample_x = pixel_x + (sub_x + 0.5) / SUPERSAMPLE

                    if figure_possible and inside_plum(
                        sample_x * to_design, sample_y * to_design
                    ):
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
