#!/usr/bin/env python3
"""Vérifie que le jeu d'icônes du site est là et conforme.

Comme `check_appicon.py`, ce n'est pas une comparaison octet par octet : un
dernier bit d'arrondi qui diffère entre deux versions de Python ferait échouer
la CI sans qu'aucune icône n'ait bougé, et une CI qui échoue pour rien est une
CI qu'on apprend à ignorer.

Ce qui est vérifié, ce sont les propriétés dont la rupture se voit :

- chaque fichier existe et fait la taille annoncée dans `index.html` ;
- l'icône iOS est opaque — un coin transparent y devient un coin noir ;
- les favicons ont bien un canal alpha, sinon leurs coins seraient des carrés
  prune dans un onglet clair ;
- le 16 pixels est bien la silhouette et non la marque complète. C'est le seul
  contrôle qui porte sur le dessin, parce que c'est la seule décision de dessin
  que ce script peut perdre en silence : quelqu'un qui « simplifierait » le
  générateur en supprimant le seuil obtiendrait un fichier de bonne taille, de
  bon format, et illisible.

    python3 Scripts/check_favicons.py
"""

from __future__ import annotations

import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PUBLIC = ROOT / "web" / "public"
INDEX = ROOT / "web" / "index.html"

RGB = 2
RGBA = 6

ATTENDUS = {
    "favicon-16.png": (16, RGBA),
    "favicon-32.png": (32, RGBA),
    "favicon-48.png": (48, RGBA),
    "favicon-192.png": (192, RGBA),
    "favicon-512.png": (512, RGBA),
    "apple-touch-icon.png": (180, RGB),
}


def entete(path: Path) -> tuple[int, int, int]:
    """(largeur, hauteur, type de couleur) lus dans l'IHDR."""
    donnees = path.read_bytes()
    if not donnees.startswith(b"\x89PNG\r\n\x1a\n"):
        raise ValueError("ce n'est pas un PNG")
    largeur, hauteur, _, couleur = struct.unpack(">IIBB", donnees[16:26])
    return largeur, hauteur, couleur


def pixels(path: Path) -> tuple[int, int, list]:
    donnees = path.read_bytes()
    index = 8
    idat = b""
    largeur = hauteur = couleur = 0
    while index < len(donnees):
        taille = struct.unpack(">I", donnees[index : index + 4])[0]
        tag = donnees[index + 4 : index + 8]
        charge = donnees[index + 8 : index + 8 + taille]
        if tag == b"IHDR":
            largeur, hauteur, _, couleur = struct.unpack(">IIBB", charge[:10])
        elif tag == b"IDAT":
            idat += charge
        index += 12 + taille

    brut = zlib.decompress(idat)
    octets = 4 if couleur == RGBA else 3
    lignes = []
    curseur = 0
    for _ in range(hauteur):
        filtre = brut[curseur]
        curseur += 1
        if filtre != 0:
            raise ValueError(f"filtre PNG {filtre} inattendu")
        ligne = brut[curseur : curseur + largeur * octets]
        curseur += largeur * octets
        lignes.append(
            [tuple(ligne[x * octets : (x + 1) * octets]) for x in range(largeur)]
        )
    return largeur, hauteur, lignes


def main() -> int:
    soucis: list[str] = []

    for nom, (cote, couleur) in ATTENDUS.items():
        chemin = PUBLIC / nom
        if not chemin.exists():
            soucis.append(f"{nom} manque — lancez : python3 Scripts/generate_favicons.py")
            continue
        try:
            largeur, hauteur, trouve = entete(chemin)
        except ValueError as erreur:
            soucis.append(f"{nom} : {erreur}")
            continue
        if (largeur, hauteur) != (cote, cote):
            soucis.append(f"{nom} fait {largeur}×{hauteur}, attendu {cote}×{cote}")
        if trouve != couleur:
            attendu = "opaque (type 2)" if couleur == RGB else "avec alpha (type 6)"
            soucis.append(f"{nom} devrait être {attendu}, trouvé type {trouve}")

    if not (PUBLIC / "favicon.ico").exists():
        soucis.append("favicon.ico manque")
    if not (PUBLIC / "site.webmanifest").exists():
        soucis.append("site.webmanifest manque")

    # Le contrôle de dessin, réglé sur ce qui sépare réellement les deux
    # rendus — mesuré, pas supposé. Sur la ligne médiane d'un 16 pixels :
    #
    #   silhouette       0110000000000110   deux plages claires
    #   marque complète  0011010000101100   quatre
    #
    # Les deux plages supplémentaires sont les filets intérieurs, larges d'un
    # pixel : c'est exactement ce qui vire au gris et fait se confondre les
    # deux disques. Compter les plages, et non les pixels clairs, parce que
    # le total de pixels clairs est presque le même dans les deux cas (48
    # contre 40) et ne distingue rien : la première version de ce contrôle
    # comptait les pixels et laissait passer la mutation qu'elle prétendait
    # attraper.
    petit = PUBLIC / "favicon-16.png"
    if petit.exists():
        try:
            largeur, hauteur, grille = pixels(petit)
            milieu = grille[hauteur // 2]
            plages = 0
            precedent = False
            for x in range(largeur):
                rouge, _, _, alpha = milieu[x]
                clair = rouge > 200 and alpha > 128
                if clair and not precedent:
                    plages += 1
                precedent = clair
            if plages != 2:
                soucis.append(
                    f"favicon-16.png montre {plages} plages claires sur sa ligne "
                    "médiane, attendu 2. Au-delà, ce sont les filets intérieurs "
                    "de la marque : à seize pixels ils virent au gris et les deux "
                    "disques se confondent. Le générateur doit y mettre la "
                    "silhouette (SEUIL_SILHOUETTE dans generate_favicons.py)."
                )
        except ValueError as erreur:
            soucis.append(f"favicon-16.png illisible : {erreur}")

    # Les liens déclarés doivent pointer vers des fichiers qui existent.
    if INDEX.exists():
        html = INDEX.read_text(encoding="utf-8")
        for reference in ("favicon.svg", "favicon-32.png", "favicon-16.png",
                          "apple-touch-icon.png", "site.webmanifest"):
            if f'href="/{reference}"' not in html:
                soucis.append(f"index.html ne référence pas /{reference}")

    if soucis:
        print("✗ jeu d'icônes du site :", file=sys.stderr)
        for souci in soucis:
            print(f"  - {souci}", file=sys.stderr)
        return 1

    print(f"✓ jeu d'icônes du site — {len(ATTENDUS) + 2} fichiers conformes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
