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

SWIFT_COLOUR = re.compile(r"static let (\w+)\s*=\s*Color\(hex:\s*0x([0-9A-Fa-f]{6})\)")
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


def main() -> int:
    theme = theme_colours()
    icons = icon_colours()
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

    checked = len(ASSET_EXPECTATIONS) + len(ICON_EXPECTATIONS) + len(ICON_ASSET_EXPECTATIONS)
    print(f"✓ Palette cohérente — {checked} correspondances vérifiées, {len(theme)} couleurs de thème")
    return 0


if __name__ == "__main__":
    sys.exit(main())
