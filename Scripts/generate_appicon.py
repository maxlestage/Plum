#!/usr/bin/env python3
"""Draw Plum's app icon and write it into the asset catalogue.

The icon is generated rather than committed as an opaque binary: the palette
lives in `PlumTheme.swift`, and an icon that drifts from the brand is a bug
nobody notices until the App Store listing. Re-run it after a palette change.

Written with zlib and struct alone — no image library — so it runs anywhere
Python does, CI included.

Le dessin — deux disques qui se chevauchent, et au milieu une troisième forme
que ni l'un ni l'autre n'a tracée — est le même que celui de
`web/public/favicon.svg` et de `web/src/components/PlumMark.tsx`, exprimé dans
le même repère de 64 unités et avec les mêmes nombres. Les trois fichiers sont
trois rendus d'un seul dessin ; changer l'un sans les autres les fait diverger
sans que rien ne le signale.

    python3 Scripts/generate_appicon.py
"""

from __future__ import annotations

import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "Plum" / "Resources" / "Assets.xcassets" / "AppIcon.appiconset" / "AppIcon.png"

SIZE = 1024
# 3×3 échantillons par pixel : assez pour que les pointes où les deux disques
# se rejoignent restent nettes.
SUPERSAMPLE = 3

# PlumTheme.Palette, kept in sync by hand — there are only three.
#
# L'icône n'utilise que `PLUM_DEEP` et `WHITE` : le fond est un aplat, pas un
# dégradé, et c'est délibéré — toute la catégorie des applications de
# rencontres vit dans le dégradé chaud. `BLUSH` et `PLUM` restent déclarés
# parce que `check_palette.py` vérifie que les trois couleurs de marque ne
# dérivent pas de `PlumTheme.swift`. C'est le filet, pas du code mort.
BLUSH = (0xE8, 0x60, 0x8C)
PLUM = (0x6B, 0x2D, 0x5C)
PLUM_DEEP = (0x40, 0x18, 0x3A)
WHITE = (0xFF, 0xF7, 0xF4)

# Le repère du dessin, partagé avec les deux SVG.
DESIGN = 64.0

# Chaque moitié de la marque : un grand disque dont on retire un petit disque
# décalé. Les SVG l'écrivent avec `fill-rule="evenodd"` — appartenir à l'un
# *ou* à l'autre, jamais aux deux — et c'est exactement le « ou exclusif »
# qu'applique `inside_mark`. Mêmes nombres des deux côtés.
GAUCHE = ((23.5, 32.0, 17.5), (30.0, 32.0, 15.2))
DROITE = ((40.5, 32.0, 17.5), (34.0, 32.0, 15.2))


def _dans(x: float, y: float, disque: tuple[float, float, float]) -> bool:
    cx, cy, r = disque
    dx, dy = x - cx, y - cy
    return dx * dx + dy * dy <= r * r


def inside_mark(x: float, y: float) -> bool:
    """La marque, en unités de dessin."""
    for grand, petit in (GAUCHE, DROITE):
        if _dans(x, y, grand) != _dans(x, y, petit):
            return True
    return False


def render() -> bytearray:
    """One RGB row per scanline, each prefixed with PNG filter type 0."""
    rows = bytearray()
    samples = SUPERSAMPLE * SUPERSAMPLE
    to_design = DESIGN / SIZE

    # Le fond et la marque sont deux aplats : il suffit donc de compter les
    # échantillons tombés dans la marque et de mélanger une fois, au lieu de
    # cumuler neuf couleurs par pixel.
    ramp = [
        bytes(
            int(PLUM_DEEP[c] + (WHITE[c] - PLUM_DEEP[c]) * hits / samples + 0.5)
            for c in range(3)
        )
        for hits in range(samples + 1)
    ]

    for pixel_y in range(SIZE):
        rows.append(0)
        row = bytearray()
        for pixel_x in range(SIZE):
            hits = 0
            for sub_y in range(SUPERSAMPLE):
                sample_y = (pixel_y + (sub_y + 0.5) / SUPERSAMPLE) * to_design
                for sub_x in range(SUPERSAMPLE):
                    sample_x = (pixel_x + (sub_x + 0.5) / SUPERSAMPLE) * to_design
                    if inside_mark(sample_x, sample_y):
                        hits += 1
            row.extend(ramp[hits])
        rows.extend(row)
    return rows


def chunk(tag: bytes, payload: bytes) -> bytes:
    return (
        struct.pack(">I", len(payload))
        + tag
        + payload
        + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)
    )


def write_png(path: Path, raw: bytes, width: int = SIZE, height: int = SIZE) -> None:
    """Colour type 2 (truecolour, no alpha): an iOS app icon must be opaque.

    Les dimensions sont des paramètres parce que `generate_ogimage.py` écrit
    la carte de partage avec la même fonction — et avec la même marque, dont
    la géométrie vit ici et nulle part ailleurs.
    """
    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
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
