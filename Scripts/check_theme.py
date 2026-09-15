#!/usr/bin/env python3
"""Vérifie que les trois moitiés du thème clair/sombre disent la même chose.

Le site suit le système par défaut et accepte un choix explicite. Ça tient sur
trois pièces qui ne se parlent pas :

1. `web/src/theme.css` déclare le jeu sombre **deux fois** — une pour la
   requête média, une pour `[data-theme="dark"]`. Deux copies dérivent, et la
   dérive ne se voit que dans celui des deux cas qu'on ne teste pas.
2. `web/index.html` porte un script qui relit le choix avant le premier rendu,
   avec la clé de stockage recopiée à la main depuis `web/src/theme.ts`.
3. Les balises `theme-color`, qui teintent la barre du navigateur.

Une pièce qui décroche donne une bascule qui a l'air de marcher. C'est le mode
de panne qu'on ne voit pas : le bouton s'enfonce, la page ne bouge pas, ou
elle bouge partout sauf à un endroit.

    python3 Scripts/check_theme.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

RACINE = Path(__file__).resolve().parent.parent
CSS = RACINE / "web" / "src" / "theme.css"
HTML = RACINE / "web" / "index.html"
TS = RACINE / "web" / "src" / "theme.ts"

SELECTEUR_AUTO = ':root:not([data-theme="light"])'
SELECTEUR_CHOISI = ':root[data-theme="dark"]'


def sans_commentaires(css: str) -> str:
    return re.sub(r"/\*.*?\*/", "", css, flags=re.S)


def declarations(css: str, selecteur: str) -> list[tuple[str, str]] | None:
    """Les paires propriété/valeur du premier bloc portant ce sélecteur.

    Un analyseur au caractère plutôt qu'une expression rationnelle : les
    valeurs contiennent des accolades (`color-mix(...)` n'en a pas, mais
    `@media` imbriqué oui) et une expression finirait par s'arrêter au mauvais
    endroit sans le dire.
    """
    depart = css.find(selecteur)
    while depart != -1:
        suite = css[depart + len(selecteur) :]
        # Le sélecteur doit être suivi de l'accolade, pas d'un descendant :
        # `:root[data-theme="dark"] a {` est un autre bloc.
        if suite.lstrip().startswith("{"):
            corps = suite[suite.index("{") + 1 :]
            profondeur = 0
            for i, caractere in enumerate(corps):
                if caractere == "{":
                    profondeur += 1
                elif caractere == "}":
                    if profondeur == 0:
                        corps = corps[:i]
                        break
                    profondeur -= 1
            return [
                (nom.strip(), valeur.strip())
                for ligne in corps.split(";")
                if ":" in ligne
                for nom, valeur in [ligne.split(":", 1)]
            ]
        depart = css.find(selecteur, depart + 1)
    return None


def blocs_media_sombre(css: str) -> list[str]:
    """Les sélecteurs déclarés sous `prefers-color-scheme: dark`."""
    selecteurs: list[str] = []
    for m in re.finditer(r"@media\s*\(prefers-color-scheme:\s*dark\)\s*\{", css):
        corps = css[m.end() :]
        profondeur = 0
        for i, caractere in enumerate(corps):
            if caractere == "{":
                profondeur += 1
            elif caractere == "}":
                if profondeur == 0:
                    corps = corps[:i]
                    break
                profondeur -= 1
        selecteurs += [
            bout.split("{")[0].strip()
            for bout in corps.split("}")
            if "{" in bout
        ]
    return selecteurs


def main() -> int:
    soucis: list[str] = []
    css = sans_commentaires(CSS.read_text(encoding="utf-8"))
    html = HTML.read_text(encoding="utf-8")
    ts = TS.read_text(encoding="utf-8")

    # ---- 1. Les deux jeux sombres, déclaration par déclaration -------------
    auto = declarations(css, SELECTEUR_AUTO)
    choisi = declarations(css, SELECTEUR_CHOISI)
    clair = declarations(css, ":root")

    if auto is None:
        soucis.append(f"aucun bloc `{SELECTEUR_AUTO}` — le thème système n'a plus de jeu sombre")
    if choisi is None:
        soucis.append(f"aucun bloc `{SELECTEUR_CHOISI}` — le choix explicite « sombre » ne peindrait rien")
    if clair is None:
        soucis.append("aucun bloc `:root` — la palette claire a disparu")

    if auto is not None and choisi is not None:
        if auto != choisi:
            seulement_auto = [d for d in auto if d not in choisi]
            seulement_choisi = [d for d in choisi if d not in auto]
            soucis.append(
                "les deux jeux sombres ont divergé :\n"
                + "".join(f"        seulement sous @media : {n}: {v}\n" for n, v in seulement_auto)
                + "".join(f"        seulement sous [data-theme] : {n}: {v}\n" for n, v in seulement_choisi)
                + ("        (même contenu, ordre différent)\n" if not seulement_auto and not seulement_choisi else "")
            )

    # ---- 2. Rien de sombre qui n'existe pas en clair -----------------------
    if auto is not None and clair is not None:
        connus = {nom for nom, _ in clair}
        for nom, _ in auto:
            if nom not in connus:
                soucis.append(
                    f"`{nom}` n'est défini qu'en sombre : en thème clair il ne vaudrait rien"
                )

    # ---- 3. Aucune règle sombre qui ignore un choix « clair » --------------
    for selecteur in blocs_media_sombre(css):
        if '[data-theme="light"]' not in selecteur:
            soucis.append(
                f"la règle `{selecteur}` sous @media sombre ne laisse pas passer un choix "
                f'« clair » — il lui manque `:not([data-theme="light"])`'
            )

    # ---- 4. Le script d'avant-rendu, et sa clé ----------------------------
    cle = re.search(r'themeStorageKey\s*=\s*"([^"]+)"', ts)
    if cle is None:
        soucis.append("`themeStorageKey` est introuvable dans web/src/theme.ts")
    else:
        avant_tete = html.split("</head>", 1)[0]
        script = re.search(r"<script>(.*?)</script>", avant_tete, re.S)
        if script is None:
            soucis.append(
                "web/index.html n'a plus de script en ligne dans <head> : une page "
                "choisie en sombre s'afficherait en clair le temps que React démarre"
            )
        elif cle.group(1) not in script.group(1):
            soucis.append(
                f'le script de web/index.html ne lit pas la clé « {cle.group(1)}» '
                "de theme.ts — le choix mémorisé ne serait jamais relu"
            )
        elif "dataset.theme" not in script.group(1):
            soucis.append("le script d'avant-rendu ne pose plus `dataset.theme`")

    # ---- 5. Les deux teintes de barre de navigateur -----------------------
    teintes = re.findall(
        r"<meta\s+name=\"theme-color\"[^>]*?content=\"([^\"]+)\"[^>]*?media=\"\(prefers-color-scheme:\s*(\w+)\)\"",
        html,
        re.S,
    )
    schemas = {schema for _, schema in teintes}
    if schemas != {"light", "dark"}:
        soucis.append(
            "web/index.html doit porter une balise `theme-color` par thème "
            f"(trouvé : {sorted(schemas) or 'aucune'}) — sans JavaScript, c'est "
            "la seule chose qui teinte la barre du navigateur"
        )

    if soucis:
        print("✗ Le thème clair/sombre s'est désaccordé :", file=sys.stderr)
        for souci in soucis:
            print(f"  - {souci}", file=sys.stderr)
        print(
            "\n  Les deux jeux sombres de web/src/theme.css doivent rester identiques "
            "au mot près.\n  Pour éprouver la bascule dans un vrai navigateur :\n"
            "    cd web && npm run build && node scripts/verifier-theme.mjs",
            file=sys.stderr,
        )
        return 1

    print(
        f"✓ Thème cohérent — {len(auto)} déclarations sombres identiques des deux côtés, "
        f"{len(teintes)} teintes de barre, script d'avant-rendu en place"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
