#!/usr/bin/env python3
"""Aucun type d'une cible ne doit porter le nom d'un type partagé.

`PlumShared` est compilé *dans* chaque cible, pas lié comme un cadriciel : le
module reste donc le même, et deux types du même nom ne cohabitent pas. Un
`private struct Message` dans une vue entre en collision avec le modèle
`Message` — et `private` n'y change rien, la portée d'un fichier ne protège
pas d'une redéclaration dans le module.

Ce contrôle existe parce que la faute est invisible à l'écriture : la vue
compilait parfaitement tant que les modèles vivaient dans la cible de
l'application. Elle n'a cassé qu'au moment où ils sont devenus partagés, dans
une cible que je ne peux pas compiler ici.

Il est syntaxique, comme ses voisins, et ne prétend pas remplacer le
compilateur — seulement rendre cette faute-là visible en une seconde plutôt
qu'en un aller-retour de quinze minutes vers la CI.

    python3 Scripts/check_name_clashes.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SHARED = ROOT / "PlumShared"
TARGETS = [ROOT / "Plum", ROOT / "PlumWatch", ROOT / "PlumWidgets"]

DECLARATION = re.compile(
    r"^\s*(?:public |private |internal |fileprivate )?"
    r"(?:final )?(?:struct|enum|class|actor|protocol)\s+([A-Za-z_]\w*)",
    re.M,
)


def declared(racine: Path) -> dict[str, list[str]]:
    trouves: dict[str, list[str]] = {}
    for fichier in sorted(racine.rglob("*.swift")):
        for nom in DECLARATION.findall(fichier.read_text(encoding="utf-8")):
            trouves.setdefault(nom, []).append(str(fichier.relative_to(ROOT)))
    return trouves


def main() -> int:
    if not SHARED.is_dir():
        print("✓ pas de code partagé, rien à vérifier")
        return 0

    partages = declared(SHARED)
    soucis: list[str] = []

    for cible in TARGETS:
        if not cible.is_dir():
            continue
        for nom, fichiers in declared(cible).items():
            if nom in partages:
                soucis.append(
                    f"{nom} — déclaré dans {', '.join(fichiers)} "
                    f"et dans {', '.join(partages[nom])}"
                )

    if soucis:
        print("✗ noms de types en collision avec le code partagé :", file=sys.stderr)
        for souci in sorted(set(soucis)):
            print(f"  - {souci}", file=sys.stderr)
        print(
            "\n  `PlumShared` est compilé dans chaque cible, donc le module est le"
            "\n  même : deux types du même nom ne cohabitent pas, `private` compris."
            "\n  Renommez celui de la cible.",
            file=sys.stderr,
        )
        return 1

    print(f"✓ noms de types — {len(partages)} types partagés, aucune collision")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
