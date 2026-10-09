#!/usr/bin/env python3
"""Vérifie que la marque est dessinée de la même façon partout où elle l'est.

    python3 Scripts/check_mark.py

Deux disques qui se chevauchent : chaque moitié est un grand disque dont on
retire un petit disque décalé. La géométrie est écrite à sept endroits, dans
cinq langages, faute de pouvoir la partager :

- `Scripts/generate_appicon.py`  — l'icône d'application, qui fait foi ;
- `Scripts/generate_favicons.py`, qui la recopie (`generate_ogimage.py`,
  lui, l'importe : il n'a rien à garder d'accord) ;
- `web/public/favicon.svg` et `web/src/components/marque.rs`, en tracés SVG ;
- `Plum/App/RootView.swift`, la marque dans l'application ;
- `web/src/animations.js`, la poussière qui la forme sur le site, et
  `web/src/theme.css`, le rideau qui s'ouvre sur elle.

Rien ne gardait leur accord. Celle de l'application avait dérivé jusqu'à ne
plus être la marque du tout : un cœur sur un dégradé, c'est-à-dire le
« j'aime » que le site dit ne pas exister. Ce script est le contrôle qui
manquait.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

Disque = tuple[float, float, float]  # centre x, centre y, rayon
Marque = tuple[Disque, Disque, Disque, Disque]  # gauche grand, petit, droite grand, petit

NOMBRE = r"(-?\d+(?:\.\d+)?)"


def depuis_python(chemin: Path) -> Marque:
    """`GAUCHE = ((23.5, 32.0, 17.5), (30.0, 32.0, 15.2))`, et `DROITE`."""
    texte = chemin.read_text(encoding="utf-8")
    moities = []
    for nom in ("GAUCHE", "DROITE"):
        motif = (
            rf"^{nom}\s*=\s*\(\s*\(\s*{NOMBRE},\s*{NOMBRE},\s*{NOMBRE}\s*\)\s*,"
            rf"\s*\(\s*{NOMBRE},\s*{NOMBRE},\s*{NOMBRE}\s*\)\s*\)"
        )
        trouve = re.search(motif, texte, re.M)
        if trouve is None:
            raise ValueError(f"{nom} introuvable")
        v = [float(x) for x in trouve.groups()]
        moities += [(v[0], v[1], v[2]), (v[3], v[4], v[5])]
    return tuple(moities)  # type: ignore[return-value]


def depuis_svg(chemin: Path) -> Marque:
    """Quatre sous-tracés `M x y a r r …` : un cercle commencé à son sommet,
    donc de centre (x, y + r)."""
    texte = re.sub(r"<!--.*?-->|//[^\n]*", " ", chemin.read_text(encoding="utf-8"), flags=re.S)
    cercles = [
        (float(x), float(y) + float(r), float(r))
        for x, y, r in re.findall(rf"M\s*{NOMBRE}\s+{NOMBRE}\s+a\s*{NOMBRE}", texte)
    ]
    if len(cercles) != 4:
        raise ValueError(f"attendu 4 sous-tracés, trouvé {len(cercles)}")
    return tuple(cercles)  # type: ignore[return-value]


def depuis_js(chemin: Path) -> Marque:
    """`const GAUCHE = [[23.5, 32.0, 17.5], [30.0, 32.0, 15.2]];`, et `DROITE`."""
    texte = chemin.read_text(encoding="utf-8")
    moities = []
    for nom in ("GAUCHE", "DROITE"):
        motif = (
            rf"const {nom}\s*=\s*\[\s*\[\s*{NOMBRE},\s*{NOMBRE},\s*{NOMBRE}\s*\]\s*,"
            rf"\s*\[\s*{NOMBRE},\s*{NOMBRE},\s*{NOMBRE}\s*\]\s*\]"
        )
        trouve = re.search(motif, texte)
        if trouve is None:
            raise ValueError(f"{nom} introuvable")
        v = [float(x) for x in trouve.groups()]
        moities += [(v[0], v[1], v[2]), (v[3], v[4], v[5])]
    return tuple(moities)  # type: ignore[return-value]


def depuis_css(chemin: Path) -> Marque:
    """Le SVG du rideau, en URI de données : les mêmes sous-tracés que le
    favicon. Les commentaires `/* */` partent d'abord ; pas les `//`, que
    l'adresse `http://www.w3.org` contient sur la même ligne que les tracés."""
    texte = re.sub(r"/\*.*?\*/", " ", chemin.read_text(encoding="utf-8"), flags=re.S)
    cercles = [
        (float(x), float(y) + float(r), float(r))
        for x, y, r in re.findall(rf"M\s*{NOMBRE}\s+{NOMBRE}\s+a\s*{NOMBRE}", texte)
    ]
    if len(cercles) != 4:
        raise ValueError(f"attendu 4 sous-tracés, trouvé {len(cercles)}")
    return tuple(cercles)  # type: ignore[return-value]


def depuis_swift(chemin: Path) -> Marque:
    """`static let gaucheGrand = Disque(x: 23.5, y: 32.0, rayon: 17.5)`, etc."""
    texte = chemin.read_text(encoding="utf-8")
    cercles = []
    for nom in ("gaucheGrand", "gauchePetit", "droiteGrand", "droitePetit"):
        trouve = re.search(
            rf"static let {nom}\s*=\s*Disque\(x:\s*{NOMBRE},\s*y:\s*{NOMBRE},\s*rayon:\s*{NOMBRE}\)",
            texte,
        )
        if trouve is None:
            raise ValueError(f"{nom} introuvable")
        cercles.append(tuple(float(x) for x in trouve.groups()))
    return tuple(cercles)  # type: ignore[return-value]


SOURCES = {
    "Scripts/generate_appicon.py": depuis_python,
    "Scripts/generate_favicons.py": depuis_python,
    "web/public/favicon.svg": depuis_svg,
    "web/src/components/marque.rs": depuis_svg,
    "Plum/App/RootView.swift": depuis_swift,
    # La poussière de l'ouverture, et le rideau qui la précède.
    "web/src/animations.js": depuis_js,
    "web/src/theme.css": depuis_css,
}
REFERENCE = "Scripts/generate_appicon.py"


def main() -> int:
    lues: dict[str, Marque] = {}
    problemes: list[str] = []
    for chemin, lire in SOURCES.items():
        fichier = ROOT / chemin
        if not fichier.exists():
            problemes.append(f"{chemin} est introuvable")
            continue
        try:
            lues[chemin] = lire(fichier)
        except ValueError as erreur:
            problemes.append(f"{chemin} : {erreur}")

    reference = lues.get(REFERENCE)
    if reference is None:
        problemes.append(f"la référence {REFERENCE} est illisible")
    else:
        noms = ("gauche, grand", "gauche, petit", "droite, grand", "droite, petit")
        for chemin, marque in lues.items():
            for nom, attendu, obtenu in zip(noms, reference, marque):
                if any(abs(a - b) > 1e-9 for a, b in zip(attendu, obtenu)):
                    problemes.append(
                        f"{chemin} — disque {nom} : {obtenu} au lieu de {attendu}"
                    )

    if problemes:
        print("✗ La marque n'est plus la même partout :", file=sys.stderr)
        for probleme in problemes:
            print(f"  - {probleme}", file=sys.stderr)
        return 1
    print(f"✓ La marque est la même dans les {len(lues)} endroits où elle est dessinée")
    return 0


if __name__ == "__main__":
    sys.exit(main())
