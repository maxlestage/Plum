#!/usr/bin/env python3
"""Check the generated app icon is something Xcode will actually accept.

Not a byte-for-byte comparison against a fresh render: floating-point rounding
can differ by a last bit between Python builds, and a CI job that fails on one
pixel is a CI job people learn to ignore. This checks the properties that
actually break a submission — wrong size, or an alpha channel, which the App
Store rejects outright.

    python3 Scripts/check_appicon.py
"""

from __future__ import annotations

import json
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ICON_SET = ROOT / "Plum" / "Resources" / "Assets.xcassets" / "AppIcon.appiconset"

EXPECTED_SIDE = 1024
TRUECOLOUR_NO_ALPHA = 2


def main() -> int:
    problems: list[str] = []

    contents_path = ICON_SET / "Contents.json"
    if not contents_path.exists():
        print("✗ AppIcon.appiconset/Contents.json est absent", file=sys.stderr)
        return 1

    contents = json.loads(contents_path.read_text(encoding="utf-8"))
    images = contents.get("images", [])
    filenames = [image["filename"] for image in images if image.get("filename")]

    if not filenames:
        problems.append("le jeu d'icônes ne référence aucun fichier — rien à soumettre")

    for filename in filenames:
        path = ICON_SET / filename
        if not path.exists():
            problems.append(f"{filename} est référencé mais absent du dépôt")
            continue

        data = path.read_bytes()
        if data[:8] != b"\x89PNG\r\n\x1a\n":
            problems.append(f"{filename} n'est pas un PNG")
            continue

        width, height, depth, colour_type = struct.unpack(">IIBB", data[16:26])
        if (width, height) != (EXPECTED_SIDE, EXPECTED_SIDE):
            problems.append(f"{filename} fait {width}×{height}, attendu {EXPECTED_SIDE}×{EXPECTED_SIDE}")
        if colour_type != TRUECOLOUR_NO_ALPHA:
            problems.append(
                f"{filename} a un canal alpha (type {colour_type}) — l'App Store refuse "
                "les icônes non opaques"
            )
        if depth != 8:
            problems.append(f"{filename} est en {depth} bits par canal, attendu 8")

        if not check_crcs(data):
            problems.append(f"{filename} a un CRC de chunk invalide — fichier corrompu")

    if problems:
        print("✗ Icône d'application invalide :", file=sys.stderr)
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        print("\n  Régénérez-la : python3 Scripts/generate_appicon.py", file=sys.stderr)
        return 1

    print(f"✓ Icône d'application valide — {', '.join(filenames)}")
    return 0


def check_crcs(data: bytes) -> bool:
    position = 8
    while position < len(data):
        length = struct.unpack(">I", data[position:position + 4])[0]
        tag = data[position + 4:position + 8]
        payload = data[position + 8:position + 8 + length]
        stored = struct.unpack(">I", data[position + 8 + length:position + 12 + length])[0]
        if stored != zlib.crc32(tag + payload) & 0xFFFFFFFF:
            return False
        position += 12 + length
    return True


if __name__ == "__main__":
    sys.exit(main())
