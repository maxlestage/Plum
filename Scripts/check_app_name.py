#!/usr/bin/env python3
"""Chaque cible porte le nom qui lui revient, partout où il est écrit.

Il y a deux noms. **Plum ‣** est celui de l'application iPhone : sous l'icône,
en haut de la sélection, et dans la ligne que l'app Réglages lui donne. La
marque s'arrête là — la montre et le widget s'appellent **Plum**.

Et chacun vit dans deux mondes qui ne se lisent pas l'un l'autre :
`PlumBrand.name` et `PlumBrand.displayName` côté Swift,
`INFOPLIST_KEY_CFBundleDisplayName` côté projet Xcode, une déclaration par
cible et par configuration. Rien ne relie les deux. Un renommage fait d'un seul
côté compile, passe tous les tests, et produit une application dont l'icône et
l'en-tête portent deux noms différents.

C'est aussi ce qui rend le message « Réactivez-la dans Réglages ▸ … » vrai ou
faux : il cite littéralement la ligne que l'app Réglages affiche, c'est-à-dire
`CFBundleDisplayName`.

    python3 Scripts/check_app_name.py
    python3 Scripts/check_app_name.py --paquet chemin/vers/Plum.app
"""

from __future__ import annotations

import argparse
import plistlib
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BRAND = ROOT / "PlumShared" / "PlumBrand.swift"
GENERATOR = ROOT / "Scripts" / "generate_xcodeproj.py"
PBXPROJ = ROOT / "Plum.xcodeproj" / "project.pbxproj"

# Les dossiers de code d'application. Le serveur et le site portent leur propre
# nom et ne sont pas concernés : c'est l'application iPhone qu'on a renommée.
SWIFT_DIRS = ["Plum", "PlumShared", "PlumWidgets", "PlumWatch"]

# Quelle cible porte quel nom, et combien de fois. Deux configurations chacune,
# Debug et Release : le compte est écrit pour qu'une cible qui perdrait sa
# déclaration ne passe pas pour une simple absence.
#
# `marque` vaut `PlumBrand.displayName`, `produit` vaut `PlumBrand.name`.
CIBLES = {
    "app.plum.ios": ("application iPhone", "marque", 2),
    "app.plum.ios.widgets": ("widgets", "produit", 2),
    "app.plum.ios.watchkitapp": ("montre", "produit", 2),
}


def echec(message: str) -> None:
    print(f"✗ {message}", file=sys.stderr)
    sys.exit(1)


def swift_literal(source: str) -> str:
    """Décode ce que le Swift écrit, `\\u{2023}` compris."""
    return re.sub(r"\\u\{([0-9A-Fa-f]+)\}", lambda m: chr(int(m.group(1), 16)), source)


def pbxproj_literal(source: str) -> str:
    """Décode ce que le `pbxproj` écrit : hors ASCII, `\\UXXXX`."""
    return re.sub(r"\\U([0-9A-Fa-f]{4})", lambda m: chr(int(m.group(1), 16)), source)


def noms_du_swift() -> dict[str, str]:
    """Les deux noms, lus dans `PlumBrand`."""
    brand = BRAND.read_text(encoding="utf-8")
    noms = {}
    for clef, champ in (("marque", "displayName"), ("produit", "name")):
        trouve = re.search(rf'static let {champ} = "([^"]*)"', brand)
        if not trouve:
            echec(f"{BRAND.name} ne déclare plus `{champ}`")
        noms[clef] = swift_literal(trouve.group(1))
    if noms["marque"] == noms["produit"]:
        echec(
            f"`name` et `displayName` valent tous deux {noms['marque']!r} : "
            "ce contrôle ne distinguerait plus rien"
        )
    return noms


def noms_du_generateur() -> dict[str, str]:
    """Les mêmes deux noms, lus dans le générateur de projet."""
    generateur = GENERATOR.read_text(encoding="utf-8")
    noms = {}
    for clef, champ in (("marque", "DISPLAY_NAME"), ("produit", "PROJECT_NAME")):
        trouve = re.search(rf'^{champ} = "([^"]*)"', generateur, re.MULTILINE)
        if not trouve:
            echec(f"generate_xcodeproj.py ne déclare plus `{champ}`")
        # Le fichier est lu comme texte, donc `\u2023` y est encore une
        # séquence de six caractères : Python ne l'a pas décodé pour nous.
        noms[clef] = trouve.group(1).encode().decode("unicode_escape")
    return noms


def declarations_du_projet() -> list[tuple[str, str]]:
    """Chaque configuration de build, sous la forme (identifiant, nom affiché).

    Lues par bloc et non par motif global : c'est ce qui permet de dire *quelle*
    cible porte un nom, là où un simple `findall` sur tout le fichier ne saurait
    que compter.
    """
    projet = PBXPROJ.read_text(encoding="utf-8")
    trouvees = []
    for bloc in re.finditer(r"buildSettings = \{\n(.*?)\n\t\t\t\};", projet, re.S):
        corps = bloc.group(1)
        identifiant = re.search(
            r"PRODUCT_BUNDLE_IDENTIFIER = \"?([^\";\n]+)\"?;", corps
        )
        nom = re.search(
            r'INFOPLIST_KEY_CFBundleDisplayName = (?:"((?:[^"\\]|\\.)*)"|([^;\s]+));',
            corps,
        )
        if identifiant is None or nom is None:
            continue
        cite, nu = nom.groups()
        trouvees.append(
            (identifiant.group(1), pbxproj_literal(cite) if cite else nu)
        )
    return trouvees


def verifier_les_sources() -> None:
    noms = noms_du_swift()

    du_generateur = noms_du_generateur()
    for clef, valeur in noms.items():
        if du_generateur[clef] != valeur:
            echec(
                f"{clef} : le générateur dit {du_generateur[clef]!r}, "
                f"PlumBrand dit {valeur!r}"
            )

    declarees = declarations_du_projet()
    if not declarees:
        echec("aucune cible ne déclare de nom affiché dans le pbxproj")

    comptes: dict[str, int] = {}
    for identifiant, valeur in declarees:
        if identifiant not in CIBLES:
            echec(
                f"{identifiant} déclare un nom affiché ({valeur!r}) alors que "
                "rien n'est attendu de cette cible"
            )
        quoi, clef, _ = CIBLES[identifiant]
        if valeur != noms[clef]:
            echec(f"{quoi} : le pbxproj dit {valeur!r}, on attendait {noms[clef]!r}")
        comptes[identifiant] = comptes.get(identifiant, 0) + 1

    for identifiant, (quoi, _, attendu) in CIBLES.items():
        vu = comptes.get(identifiant, 0)
        if vu != attendu:
            echec(
                f"{quoi} : {vu} déclaration(s) de nom affiché, {attendu} attendues "
                "— une configuration en a perdu ou gagné une"
            )

    # Et personne ne réécrit un nom à la main. C'est la dérive que les deux
    # constantes existent pour empêcher : un `Text("Plum")` remis en place
    # compilerait, ne casserait aucun test, et laisserait un écran au nom
    # d'avant. `PlumBrand` est exempté : c'est lui qui les déclare.
    litteraux = re.compile(
        "|".join(re.escape(f'"{valeur}"') for valeur in noms.values())
    )
    fautifs = []
    for dossier in SWIFT_DIRS:
        for fichier in sorted((ROOT / dossier).rglob("*.swift")):
            if fichier == BRAND:
                continue
            for numero, ligne in enumerate(
                fichier.read_text(encoding="utf-8").splitlines(), 1
            ):
                if litteraux.search(swift_literal(ligne)):
                    fautifs.append(f"{fichier.relative_to(ROOT)}:{numero}")
    if fautifs:
        echec(
            "un nom est écrit en dur au lieu de `PlumBrand` :\n  "
            + "\n  ".join(fautifs)
        )

    print(
        f"✓ noms affichés — application iPhone {noms['marque']!r}, "
        f"montre et widgets {noms['produit']!r} ; d'accord dans PlumBrand, "
        f"le générateur et les {len(declarees)} configurations du projet"
    )


def plists_du_paquet(paquet: Path) -> list[tuple[str, str, Path]]:
    """Les `Info.plist` du paquet construit, avec le nom attendu de chacun.

    La montre et le widget ne sont pas toujours embarqués — une compilation
    visant le simulateur iOS peut se passer de la cible watchOS. Ils sont donc
    vérifiés s'ils sont là, et leur absence est dite plutôt que tue.
    """
    trouves = [("application iPhone", "marque", paquet / "Info.plist")]
    for embarque in sorted((paquet / "Watch").glob("*.app/Info.plist")):
        trouves.append((f"montre ({embarque.parent.name})", "produit", embarque))
    for embarque in sorted((paquet / "PlugIns").glob("*.appex/Info.plist")):
        trouves.append((f"extension ({embarque.parent.name})", "produit", embarque))
    return trouves


def verifier_paquet(paquet: Path) -> None:
    """Ce que le paquet construit dit vraiment.

    Le reste de ce script compare des sources entre elles : il prouve que la
    recette est cohérente, pas que le plat lui ressemble. Entre les deux il y a
    Xcode, qui doit décoder `\\U2023` en son caractère. S'il ne le faisait pas,
    l'écran d'accueil afficherait la séquence en clair — et rien, ni la
    compilation, ni les tests, ni les contrôles statiques, ne s'en plaindrait.
    """
    if not paquet.is_dir():
        echec(f"{paquet} n'est pas un paquet construit")

    noms = noms_du_swift()
    lus = []
    for quoi, clef, plist in plists_du_paquet(paquet):
        if not plist.is_file():
            if clef == "marque":
                echec(f"{plist} est absent : le paquet n'a pas été construit")
            continue
        with plist.open("rb") as flux:
            contenu = plistlib.load(flux)
        valeur = contenu.get("CFBundleDisplayName")
        if valeur is None:
            echec(f"{quoi} : le paquet construit ne porte aucun nom affiché")
        if valeur != noms[clef]:
            echec(f"{quoi} : le paquet dit {valeur!r}, on attendait {noms[clef]!r}")
        lus.append(f"{quoi} {valeur!r}")

    print(f"✓ paquet construit — {', '.join(lus)}")


def main() -> None:
    arguments = argparse.ArgumentParser(description=__doc__)
    arguments.add_argument(
        "--paquet",
        type=Path,
        help="Chemin d'un `Plum.app` construit : vérifie ses Info.plist réels "
        "plutôt que les seules sources.",
    )
    options = arguments.parse_args()

    if options.paquet is not None:
        verifier_paquet(options.paquet)
    else:
        verifier_les_sources()


if __name__ == "__main__":
    main()
