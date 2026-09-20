#!/usr/bin/env python3
"""Le nom affiché de l'application dit la même chose partout.

Le nom qui s'affiche vit dans deux mondes qui ne se lisent pas l'un l'autre :
`PlumBrand.displayName`, que le Swift utilise pour l'en-tête, l'écran
d'accueil de la montre et le chemin vers l'app Réglages ; et
`INFOPLIST_KEY_CFBundleDisplayName`, que Xcode pose sous l'icône, dans la
galerie de widgets et dans Réglages. Rien ne relie les deux — un renommage
fait d'un seul côté compile, passe tous les tests, et produit une application
dont l'icône et l'en-tête portent deux noms différents.

C'est aussi ce qui rend le message « Réactivez-la dans Réglages ▸ … » vrai ou
faux : il cite littéralement la ligne que l'app Réglages affiche, c'est-à-dire
`CFBundleDisplayName`.

    python3 Scripts/check_app_name.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BRAND = ROOT / "PlumShared" / "PlumBrand.swift"
GENERATOR = ROOT / "Scripts" / "generate_xcodeproj.py"
PBXPROJ = ROOT / "Plum.xcodeproj" / "project.pbxproj"

# Les dossiers de code d'application. Le serveur et le site portent leur propre
# nom et ne sont pas concernés : c'est l'application qu'on a renommée.
SWIFT_DIRS = ["Plum", "PlumShared", "PlumWidgets", "PlumWatch"]

# Trois cibles — application, widgets, montre — et deux configurations
# chacune. Le nombre est écrit pour qu'une cible qui perdrait son nom
# affiché ne passe pas pour une simple absence.
EXPECTED_DECLARATIONS = 6


def swift_literal(source: str) -> str:
    """Décode ce que le Swift écrit, `\\u{2023}` compris."""
    return re.sub(r"\\u\{([0-9A-Fa-f]+)\}", lambda m: chr(int(m.group(1), 16)), source)


def pbxproj_literal(source: str) -> str:
    """Décode ce que le `pbxproj` écrit : hors ASCII, `\\UXXXX`."""
    return re.sub(r"\\U([0-9A-Fa-f]{4})", lambda m: chr(int(m.group(1), 16)), source)


def echec(message: str) -> None:
    print(f"✗ {message}", file=sys.stderr)
    sys.exit(1)


def main() -> None:
    brand = BRAND.read_text(encoding="utf-8")
    trouve = re.search(r'static let displayName = "([^"]*)"', brand)
    if not trouve:
        echec(f"{BRAND.name} ne déclare plus `displayName`")
    attendu = swift_literal(trouve.group(1))

    generateur = GENERATOR.read_text(encoding="utf-8")
    trouve = re.search(r'^DISPLAY_NAME = "([^"]*)"', generateur, re.MULTILINE)
    if not trouve:
        echec("generate_xcodeproj.py ne déclare plus `DISPLAY_NAME`")
    # Le générateur écrit la valeur en littéral Python ; `\u2023` y est déjà
    # décodé à la lecture du fichier par Python lui-même, sauf ici où on lit
    # le texte source — d'où le décodage explicite.
    du_generateur = trouve.group(1).encode().decode("unicode_escape")
    if du_generateur != attendu:
        echec(
            f"le générateur dit {du_generateur!r}, "
            f"PlumBrand.displayName dit {attendu!r}"
        )

    projet = PBXPROJ.read_text(encoding="utf-8")
    declares = re.findall(
        r'INFOPLIST_KEY_CFBundleDisplayName = (?:"((?:[^"\\]|\\.)*)"|([^;\s]+));',
        projet,
    )
    if not declares:
        echec("aucune cible ne déclare de nom affiché dans le pbxproj")
    if len(declares) != EXPECTED_DECLARATIONS:
        echec(
            f"{len(declares)} déclarations de nom affiché, {EXPECTED_DECLARATIONS} "
            "attendues — une cible en a perdu ou gagné une"
        )
    for cite, nu in declares:
        valeur = pbxproj_literal(cite) if cite else nu
        if valeur != attendu:
            echec(f"le pbxproj dit {valeur!r}, PlumBrand.displayName dit {attendu!r}")

    # Et personne ne réécrit le nom à la main. C'est la dérive que la constante
    # existe pour empêcher : un `Text("Plum")` remis en place compilerait, ne
    # casserait aucun test, et laisserait un écran au nom d'avant.
    fautifs = []
    for dossier in SWIFT_DIRS:
        for fichier in sorted((ROOT / dossier).rglob("*.swift")):
            for numero, ligne in enumerate(
                fichier.read_text(encoding="utf-8").splitlines(), 1
            ):
                if re.search(r'(?<![\w.])"Plum"', ligne):
                    fautifs.append(f"{fichier.relative_to(ROOT)}:{numero}")
    if fautifs:
        echec(
            "le nom est écrit en dur au lieu de `PlumBrand.displayName` :\n  "
            + "\n  ".join(fautifs)
        )

    print(
        f"✓ nom affiché — {attendu!r}, identique dans PlumBrand, "
        f"le générateur et les {len(declares)} configurations du projet"
    )


if __name__ == "__main__":
    main()
