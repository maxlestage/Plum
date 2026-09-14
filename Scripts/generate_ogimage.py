#!/usr/bin/env python3
"""Draw the card that appears when someone shares a link to the site.

Sans elle, un lien envoyé sur WhatsApp, iMessage ou Slack s'affiche en texte
nu : un titre, une ligne, et rien à regarder. Les balises `og:title` et
`og:description` étaient déjà là ; c'est l'image qui manquait.

Aucun texte dans l'image, et c'est délibéré : les aperçus affichent déjà le
titre et la description à côté, et dessiner du texte demanderait d'embarquer
une fonte et de la vectoriser — là où la marque seule se calcule.

La marque vient de `generate_appicon.py`, importée plutôt que recopiée : sa
géométrie ne doit exister qu'à un seul endroit côté Python.

    python3 Scripts/generate_ogimage.py
"""

from __future__ import annotations

from pathlib import Path

from generate_appicon import DESIGN, PLUM_DEEP, WHITE, inside_mark, write_png

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "web" / "public" / "partage.png"

# Le format qu'attendent Facebook, Slack, iMessage et les autres. Plus petit,
# l'aperçu est rogné ou flouté ; plus grand, on transporte des pixels que
# personne n'affiche.
WIDTH, HEIGHT = 1200, 630

# La part de la hauteur qu'occupe la marque — sa hauteur *réelle*, pas celle
# de la boîte de dessin de 64 unités : la marque n'en remplit que la moitié en
# hauteur, et se dimensionner sur la boîte la rendait deux fois trop petite.
MARK_HEIGHT = 0.5

# L'encombrement de la marque dans le repère de dessin, déduit de sa géométrie
# plutôt que mesuré à l'œil : deux disques de rayon 17,5 centrés en 23,5 et
# 40,5 sur la ligne y = 32.
MARK_LEFT, MARK_RIGHT = 23.5 - 17.5, 40.5 + 17.5
MARK_TOP, MARK_BOTTOM = 32.0 - 17.5, 32.0 + 17.5

SUPERSAMPLE = 3


def render() -> bytearray:
    rows = bytearray()
    samples = SUPERSAMPLE * SUPERSAMPLE

    # Le facteur qui amène la marque à la taille voulue, puis le décalage qui
    # centre son encombrement — et non la boîte — sur la carte.
    scale = (HEIGHT * MARK_HEIGHT) / (MARK_BOTTOM - MARK_TOP)
    left = (WIDTH - (MARK_RIGHT - MARK_LEFT) * scale) / 2 - MARK_LEFT * scale
    top = (HEIGHT - (MARK_BOTTOM - MARK_TOP) * scale) / 2 - MARK_TOP * scale

    ramp = [
        bytes(
            int(PLUM_DEEP[c] + (WHITE[c] - PLUM_DEEP[c]) * hits / samples + 0.5)
            for c in range(3)
        )
        for hits in range(samples + 1)
    ]

    for pixel_y in range(HEIGHT):
        rows.append(0)
        row = bytearray()
        for pixel_x in range(WIDTH):
            hits = 0
            for sub_y in range(SUPERSAMPLE):
                y = ((pixel_y + (sub_y + 0.5) / SUPERSAMPLE) - top) / scale
                # Hors de la bande où la marque vit, rien à mesurer.
                if not 0.0 <= y <= DESIGN:
                    continue
                for sub_x in range(SUPERSAMPLE):
                    x = ((pixel_x + (sub_x + 0.5) / SUPERSAMPLE) - left) / scale
                    if 0.0 <= x <= DESIGN and inside_mark(x, y):
                        hits += 1
            row.extend(ramp[hits])
        rows.extend(row)
    return rows


def main() -> None:
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    write_png(OUTPUT, render(), WIDTH, HEIGHT)
    print(f"{OUTPUT.relative_to(ROOT)} — {WIDTH}×{HEIGHT}, {OUTPUT.stat().st_size // 1024} Ko")


if __name__ == "__main__":
    main()
