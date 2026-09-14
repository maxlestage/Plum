#!/usr/bin/env python3
"""Dessine le jeu d'icônes du site, à partir de la même marque que l'icône iOS.

La géométrie vit dans `generate_appicon.py` et nulle part ailleurs : ce
script l'importe plutôt que de la recopier, pour que la marque ne puisse pas
diverger d'un fichier à l'autre sans que rien ne le signale.

Une seule chose change ici, et elle est mesurée plutôt que supposée : **à
seize pixels, la marque complète ne tient pas.** Les deux anneaux ont un filet
intérieur de 2,3 unités sur 64, soit 0,57 pixel à cette taille : il disparaît
dans le gris, les deux disques se confondent, et il ne reste qu'un ovale
indistinct. Rendu et regardé, pas déduit.

La réduction retenue pour 16 pixels est la *silhouette* de la marque — le même
contour, débarrassé des filets intérieurs qu'aucun écran ne peut montrer à
cette taille. C'est la réduction la plus honnête possible : à seize pixels,
la silhouette est de toute façon tout ce que l'œil perçoit. L'autre piste
essayée, l'amande que les deux disques font naître au milieu, était plus
distinctive mais inversait le blanc et le prune de la marque — elle se lisait
comme une autre marque.

    python3 Scripts/generate_favicons.py
"""

from __future__ import annotations

import struct
import zlib
from pathlib import Path

from generate_appicon import DESIGN, PLUM_DEEP, WHITE, _dans, inside_mark

ROOT = Path(__file__).resolve().parent.parent
PUBLIC = ROOT / "web" / "public"

GAUCHE = ((23.5, 32.0, 17.5), (30.0, 32.0, 15.2))
DROITE = ((40.5, 32.0, 17.5), (34.0, 32.0, 15.2))
CENTRE = DESIGN / 2

# Au-dessous de ce seuil, on dessine la silhouette ; au-dessus, la marque
# entière. Vingt-quatre parce que le filet intérieur y vaut encore 0,86 pixel
# et commence tout juste à se tenir ; à seize il n'en reste rien.
SEUIL_SILHOUETTE = 24

# La silhouette est grossie de 15 % dans son disque.
#
# À sa taille nominale elle laisse une large bande de prune en haut et en bas —
# la marque est large et plate, le cadre est carré. À 15 % elle occupe le disque
# sans le toucher. Essayé aussi à 30 % : le blanc déborde sur le bord gauche et
# droit du disque et la silhouette se rompt. Réglé en regardant les trois
# rendus, pas en calculant une proportion.
GROSSISSEMENT = 1.15


def silhouette(x: float, y: float) -> bool:
    """La marque sans ses filets intérieurs : le grand disque moins le petit."""
    for grand, petit in (GAUCHE, DROITE):
        if _dans(x, y, grand) and not _dans(x, y, petit):
            return True
    return False


def _png(path: Path, raw: bytes, width: int, height: int, colour: int) -> None:
    head = struct.pack(">IIBBBBB", width, height, 8, colour, 0, 0, 0)

    def chunk(tag: bytes, payload: bytes) -> bytes:
        return (
            struct.pack(">I", len(payload))
            + tag
            + payload
            + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)
        )

    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", head)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def dessine(size: int, *, ronde: bool, alpha: bool, supersample: int = 4) -> bytearray:
    """Une image carrée de `size` pixels, en unités de dessin de 64.

    `ronde` découpe le fond en disque — les coins d'un onglet de navigateur
    valent mieux transparents. L'icône iOS, elle, doit être un carré opaque :
    le système lui applique son propre masque, et un coin transparent y
    devient un coin noir.
    """
    reduite = size < SEUIL_SILHOUETTE
    forme = silhouette if reduite else inside_mark
    grossi = GROSSISSEMENT if reduite else 1.0
    rows = bytearray()
    samples = supersample * supersample
    to_design = DESIGN / size

    for pixel_y in range(size):
        rows.append(0)
        for pixel_x in range(size):
            dedans = 0
            fond = 0
            for sub_y in range(supersample):
                sy = (pixel_y + (sub_y + 0.5) / supersample) * to_design
                for sub_x in range(supersample):
                    sx = (pixel_x + (sub_x + 0.5) / supersample) * to_design
                    if ronde and not _dans(sx, sy, (CENTRE, CENTRE, CENTRE)):
                        continue
                    fond += 1
                    if forme(CENTRE + (sx - CENTRE) / grossi, CENTRE + (sy - CENTRE) / grossi):
                        dedans += 1

            if fond == 0:
                rows.extend(b"\x00\x00\x00\x00" if alpha else bytes(PLUM_DEEP))
                continue

            # La couleur se mélange sur les échantillons couverts, la
            # transparence sur tous : sinon le bord du disque prendrait la
            # couleur du fond au lieu de s'effacer.
            part = dedans / fond
            pixel = bytes(
                int(PLUM_DEEP[c] + (WHITE[c] - PLUM_DEEP[c]) * part + 0.5)
                for c in range(3)
            )
            rows.extend(pixel + bytes([round(255 * fond / samples)]) if alpha else pixel)
    return rows


def ico(path: Path, tailles: list[int]) -> None:
    """Un `.ico` qui embarque plusieurs PNG.

    Le format accepte des PNG tels quels depuis Vista, et c'est ce que tout
    navigateur encore en service sait lire. Il reste utile pour la racine du
    domaine, que certains agents vont chercher sans lire le HTML.
    """
    images = []
    for taille in tailles:
        brut = dessine(taille, ronde=True, alpha=True)
        tampon = path.parent / f".ico-{taille}.png"
        _png(tampon, brut, taille, taille, 6)
        images.append(tampon.read_bytes())
        tampon.unlink()

    offset = 6 + 16 * len(images)
    entetes = b""
    for taille, donnees in zip(tailles, images):
        entetes += struct.pack(
            "<BBBBHHII",
            taille if taille < 256 else 0,
            taille if taille < 256 else 0,
            0,
            0,
            1,
            32,
            len(donnees),
            offset,
        )
        offset += len(donnees)

    path.write_bytes(
        struct.pack("<HHH", 0, 1, len(images)) + entetes + b"".join(images)
    )


def main() -> None:
    PUBLIC.mkdir(parents=True, exist_ok=True)

    # Les onglets et les favoris : coins transparents.
    for taille in (16, 32, 48, 192, 512):
        _png(
            PUBLIC / f"favicon-{taille}.png",
            dessine(taille, ronde=True, alpha=True),
            taille,
            taille,
            6,
        )

    ico(PUBLIC / "favicon.ico", [16, 32, 48])

    # « Sur l'écran d'accueil » d'iOS : carré, opaque, sans coins arrondis —
    # le système s'en charge, et les arrondir deux fois se voit.
    _png(
        PUBLIC / "apple-touch-icon.png",
        dessine(180, ronde=False, alpha=False),
        180,
        180,
        2,
    )

    for chemin in sorted(PUBLIC.glob("favicon*")) + [
        PUBLIC / "apple-touch-icon.png"
    ]:
        print(f"  {chemin.name} — {chemin.stat().st_size} octets")


if __name__ == "__main__":
    main()
