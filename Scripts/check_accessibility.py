#!/usr/bin/env python3
"""Aucun bouton de l'application ne doit être muet pour VoiceOver.

Un `Button("Réessayer")` porte son texte, donc VoiceOver le lit. Un bouton
dont l'étiquette n'est qu'une icône ne dit rien : il est annoncé « bouton »,
et c'est tout. Sur une grille de six photos, cela donnait six « bouton »
identiques, dont aucun n'indiquait laquelle il allait détruire.

Ce contrôle est volontairement syntaxique — il lit le texte des fichiers, il
ne compile rien. C'est sa limite et c'est aussi ce qui le rend utilisable :
il tourne partout où Python tourne, en une seconde, et il attrape la seule
faute qui compte ici, celle qu'on commet en ajoutant un bouton-icône sans y
penser.

Ce qu'il ne prétend pas faire : juger la *qualité* d'une étiquette. « Bouton
un » passerait. Aucun outil ne sait faire cette différence ; une relecture,
si.

    python3 Scripts/check_accessibility.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# L'application, la montre, les extensions et le code qu'elles partagent. Sans les deux
# derniers, un bouton-icône ajouté dans un widget échapperait au contrôle —
# et c'est là qu'on en ajoute sans y penser, faute de l'avoir sous les yeux.
SOURCES = [
    directory
    for directory in (ROOT / "Plum", ROOT / "PlumShared", ROOT / "PlumWidgets", ROOT / "PlumWatch")
    if directory.is_dir()
]

# Combien de lignes après l'ouverture du bouton on considère comme faisant
# partie de sa déclaration. Quatorze couvre un `label:` sur plusieurs lignes
# suivi de ses modificateurs, sans mordre sur la vue suivante.
PORTEE = 14

OUVERTURE = re.compile(r"\bButton\s*[({]")
TEXTE_DIRECT = re.compile(r'Button\s*\(\s*"')
VARIABLE_DIRECTE = re.compile(r"Button\s*\(\s*[a-z]")


def muets() -> list[tuple[Path, int, str]]:
    trouves: list[tuple[Path, int, str]] = []
    for fichier in sorted(f for racine in SOURCES for f in racine.rglob("*.swift")):
        lignes = fichier.read_text(encoding="utf-8").split("\n")
        for index, ligne in enumerate(lignes):
            if not OUVERTURE.search(ligne):
                continue
            # `Button("Texte")` et `Button(titre)` portent déjà leur libellé.
            if TEXTE_DIRECT.search(ligne) or VARIABLE_DIRECTE.search(ligne):
                continue

            bloc = "\n".join(lignes[index : index + PORTEE])
            porte_du_texte = "Text(" in bloc or "Label(" in bloc
            porte_une_icone = "Image(systemName" in bloc or "systemImage" in bloc
            etiquete = "accessibilityLabel" in bloc

            if porte_une_icone and not porte_du_texte and not etiquete:
                trouves.append((fichier.relative_to(ROOT), index + 1, ligne.strip()))
    return trouves


def main() -> int:
    trouves = muets()
    if trouves:
        print("✗ boutons muets pour VoiceOver :", file=sys.stderr)
        for fichier, ligne, _ in trouves:
            print(f"  - {fichier}:{ligne}", file=sys.stderr)
        print(
            "\n  Un bouton dont l'étiquette n'est qu'une icône est annoncé"
            "\n  « bouton », sans rien d'autre. Ajoutez un"
            "\n  `.accessibilityLabel(Text(\"…\"))` qui dise ce qu'il fait — et,"
            "\n  s'il y en a plusieurs semblables, sur quoi il agit.",
            file=sys.stderr,
        )
        return 1

    boutons = sum(
        len(OUVERTURE.findall(f.read_text(encoding="utf-8")))
        for racine in SOURCES
        for f in racine.rglob("*.swift")
    )
    print(f"✓ accessibilité — {boutons} boutons, aucun muet")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
