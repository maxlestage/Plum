#!/usr/bin/env python3
"""Check that the brand colours agree wherever they are written down.

`PlumTheme.swift` is the source of truth, but two other places have to repeat
the values: the asset catalogue (colours SwiftUI resolves by name, with a dark
variant) and the icon generator (which has no access to Swift). Nothing stops
those from drifting apart except this script.

    python3 Scripts/check_palette.py
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
THEME = ROOT / "Plum" / "DesignSystem" / "PlumTheme.swift"
ASSETS = ROOT / "Plum" / "Resources" / "Assets.xcassets"
ICON_SCRIPT = ROOT / "Scripts" / "generate_appicon.py"
SITE_CSS = ROOT / "web" / "src" / "theme.css"
BRAND = ROOT / "PlumShared" / "PlumBrand.swift"

# Which theme colour each repetition must equal.
ASSET_EXPECTATIONS = {
    # asset colour set (light variant) -> PlumTheme.Palette member
    "AccentColor": "plum",
    "PrimaryText": "ink",
}
ICON_EXPECTATIONS = {
    # constant in generate_appicon.py -> PlumTheme.Palette member
    "BLUSH": "blush",
    "PLUM": "plum",
    "PLUM_DEEP": "plumDeep",
}
# The icon's ink is the light canvas, so the mark matches the app's paper.
ICON_ASSET_EXPECTATIONS = {"WHITE": "Canvas"}
# The presentation site repeats the palette a fourth time, in CSS. A site that
# drifts from the product it presents looks like a fake.
SITE_EXPECTATIONS = {
    "--plum": "plum",
    "--plum-deep": "plumDeep",
    "--blush": "blush",
    "--apricot": "apricot",
    "--mint": "mint",
}

SWIFT_COLOUR = re.compile(r"static let (\w+)\s*=\s*Color\(hex:\s*0x([0-9A-Fa-f]{6})\)")
CSS_COLOUR = re.compile(r"(--[a-z-]+):\s*#([0-9A-Fa-f]{6});")
PYTHON_COLOUR = re.compile(r"^(\w+)\s*=\s*\(0x([0-9A-Fa-f]{2}),\s*0x([0-9A-Fa-f]{2}),\s*0x([0-9A-Fa-f]{2})\)", re.M)


def theme_colours() -> dict[str, int]:
    return {
        name: int(value, 16)
        for name, value in SWIFT_COLOUR.findall(THEME.read_text(encoding="utf-8"))
    }


def icon_colours() -> dict[str, int]:
    found = {}
    for name, red, green, blue in PYTHON_COLOUR.findall(ICON_SCRIPT.read_text(encoding="utf-8")):
        found[name] = (int(red, 16) << 16) | (int(green, 16) << 8) | int(blue, 16)
    return found


def site_colours() -> dict[str, int]:
    if not SITE_CSS.exists():
        return {}
    # Only the light palette, declared on bare `:root`: the dark block
    # redefines the neutrals, not the brand colours.
    text = SITE_CSS.read_text(encoding="utf-8").split("@media", 1)[0]
    return {name: int(value, 16) for name, value in CSS_COLOUR.findall(text)}


def asset_colour(name: str) -> int | None:
    """The light (appearance-free) variant of a colour set."""
    path = ASSETS / f"{name}.colorset" / "Contents.json"
    if not path.exists():
        return None
    for entry in json.loads(path.read_text(encoding="utf-8"))["colors"]:
        if entry.get("appearances"):
            continue
        components = entry["color"]["components"]
        return (
            (int(components["red"], 16) << 16)
            | (int(components["green"], 16) << 8)
            | int(components["blue"], 16)
        )
    return None


# `PlumBrand` est la copie que voit l'extension de widget : une cible séparée
# ne compile pas `PlumTheme`. Elle écrit ses couleurs en composantes, d'où la
# comparaison sur le triplet plutôt que sur l'hexadécimal.
BRAND_EXPECTATIONS = {
    "plum": "plum",
    "plumDeep": "plumDeep",
    "blush": "blush",
}


def brand_colours() -> dict[str, int]:
    """Les couleurs de `PlumBrand`, recomposées depuis leurs composantes."""
    if not BRAND.exists():
        return {}
    found: dict[str, int] = {}
    motif = re.compile(
        r"static let (\w+) = Color\(\s*red: 0x([0-9A-Fa-f]{2}) / 255,\s*"
        r"green: 0x([0-9A-Fa-f]{2}) / 255,\s*blue: 0x([0-9A-Fa-f]{2}) / 255\s*\)"
    )
    for m in motif.finditer(BRAND.read_text(encoding="utf-8")):
        found[m.group(1)] = (
            int(m.group(2), 16) << 16 | int(m.group(3), 16) << 8 | int(m.group(4), 16)
        )
    return found


def main() -> int:
    theme = theme_colours()
    icons = icon_colours()
    site = site_colours()
    problems: list[str] = []

    if not theme:
        problems.append("aucune couleur trouvée dans PlumTheme.swift")

    for asset, member in ASSET_EXPECTATIONS.items():
        expected = theme.get(member)
        actual = asset_colour(asset)
        if expected is None:
            problems.append(f"PlumTheme.Palette.{member} est introuvable")
        elif actual is None:
            problems.append(f"jeu de couleurs {asset} introuvable dans le catalogue")
        elif actual != expected:
            problems.append(
                f"{asset} (#{actual:06X}) ne suit plus Palette.{member} (#{expected:06X})"
            )

    for constant, member in ICON_EXPECTATIONS.items():
        expected = theme.get(member)
        actual = icons.get(constant)
        if expected is None or actual is None:
            problems.append(f"impossible de comparer {constant} et Palette.{member}")
        elif actual != expected:
            problems.append(
                f"generate_appicon.{constant} (#{actual:06X}) ne suit plus "
                f"Palette.{member} (#{expected:06X})"
            )

    for constant, asset in ICON_ASSET_EXPECTATIONS.items():
        expected = asset_colour(asset)
        actual = icons.get(constant)
        if expected is None or actual is None:
            problems.append(f"impossible de comparer {constant} et le jeu {asset}")
        elif actual != expected:
            problems.append(
                f"generate_appicon.{constant} (#{actual:06X}) ne suit plus "
                f"{asset} (#{expected:06X})"
            )

    if site:
        for variable, member in SITE_EXPECTATIONS.items():
            expected = theme.get(member)
            actual = site.get(variable)
            if expected is None or actual is None:
                problems.append(f"impossible de comparer {variable} et Palette.{member}")
            elif actual != expected:
                problems.append(
                    f"theme.css {variable} (#{actual:06X}) ne suit plus "
                    f"Palette.{member} (#{expected:06X})"
                )

    brand = brand_colours()
    if BRAND.exists():
        if not brand:
            problems.append(
                "aucune couleur lisible dans PlumBrand.swift — le motif attendu est "
                "`Color(red: 0xAA / 255, green: 0xBB / 255, blue: 0xCC / 255)`"
            )
        for constant, member in BRAND_EXPECTATIONS.items():
            expected = theme.get(member)
            actual = brand.get(constant)
            if expected is None or actual is None:
                problems.append(f"impossible de comparer PlumBrand.{constant} et Palette.{member}")
            elif actual != expected:
                problems.append(
                    f"PlumBrand.{constant} (#{actual:06X}) ne suit plus "
                    f"Palette.{member} (#{expected:06X})"
                )

    if problems:
        print("✗ La palette a divergé :", file=sys.stderr)
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        print(
            "\n  PlumTheme.swift fait foi. Après un changement de palette, "
            "mettez à jour le catalogue puis relancez Scripts/generate_appicon.py.",
            file=sys.stderr,
        )
        return 1

    checked = (
        len(ASSET_EXPECTATIONS)
        + len(ICON_EXPECTATIONS)
        + len(ICON_ASSET_EXPECTATIONS)
        + (len(SITE_EXPECTATIONS) if site else 0)
    )
    print(f"✓ Palette cohérente — {checked} correspondances vérifiées, {len(theme)} couleurs de thème")
    return 0


if __name__ == "__main__":
    sys.exit(main())
