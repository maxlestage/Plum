#!/usr/bin/env python3
"""Parse and sanity-check Plum.xcodeproj/project.pbxproj.

Xcode is the only real authority on a project file, and it is not available on
every machine that touches this repo (CI included). This parses the OpenStep
plist by hand and checks the invariants that actually break projects: dangling
object references, missing `isa`, files listed in a build phase but absent from
the target's group, and duplicate identifiers.

    python3 Scripts/validate_pbxproj.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PBXPROJ = ROOT / "Plum.xcodeproj" / "project.pbxproj"

TOKEN = re.compile(
    r"""
      (?P<comment>/\*.*?\*/)
    | (?P<string>"(?:[^"\\]|\\.)*")
    | (?P<punct>[{}()=;,])
    | (?P<bare>[A-Za-z0-9_./$@:+\-<>\\*]+)
    | (?P<space>\s+)
    """,
    re.VERBOSE | re.DOTALL,
)

OBJECT_ID = re.compile(r"^[0-9A-F]{24}$")


class ParseError(Exception):
    pass


def tokenize(text: str) -> list[str]:
    tokens: list[str] = []
    position = 0
    while position < len(text):
        match = TOKEN.match(text, position)
        if not match:
            raise ParseError(f"Jeton inattendu à l'offset {position}: {text[position:position + 40]!r}")
        position = match.end()
        if match.lastgroup in {"comment", "space"}:
            continue
        value = match.group()
        if match.lastgroup == "string":
            value = value[1:-1].replace('\\"', '"').replace("\\\\", "\\")
        tokens.append(value)
    return tokens


class Parser:
    def __init__(self, tokens: list[str]):
        self.tokens = tokens
        self.index = 0

    def peek(self) -> str | None:
        return self.tokens[self.index] if self.index < len(self.tokens) else None

    def next(self) -> str:
        token = self.peek()
        if token is None:
            raise ParseError("Fin de fichier inattendue")
        self.index += 1
        return token

    def expect(self, expected: str) -> None:
        token = self.next()
        if token != expected:
            raise ParseError(f"Attendu {expected!r}, trouvé {token!r} (jeton #{self.index})")

    def parse_value(self):
        token = self.peek()
        if token == "{":
            return self.parse_dict()
        if token == "(":
            return self.parse_array()
        return self.next()

    def parse_dict(self) -> dict:
        self.expect("{")
        result: dict[str, object] = {}
        while True:
            token = self.peek()
            if token is None:
                raise ParseError("Dictionnaire non fermé")
            if token == "}":
                self.next()
                return result
            key = self.next()
            self.expect("=")
            value = self.parse_value()
            self.expect(";")
            if key in result:
                raise ParseError(f"Clé dupliquée : {key}")
            result[key] = value

    def parse_array(self) -> list:
        self.expect("(")
        items: list[object] = []
        while True:
            token = self.peek()
            if token is None:
                raise ParseError("Tableau non fermé")
            if token == ")":
                self.next()
                return items
            items.append(self.parse_value())
            if self.peek() == ",":
                self.next()


def collect_references(value, found: set[str]) -> None:
    if isinstance(value, dict):
        for item in value.values():
            collect_references(item, found)
    elif isinstance(value, list):
        for item in value:
            collect_references(item, found)
    elif isinstance(value, str) and OBJECT_ID.match(value):
        found.add(value)


def main() -> int:
    text = PBXPROJ.read_text(encoding="utf-8")
    if not text.startswith("// !$*UTF8*$!"):
        print("✗ En-tête UTF-8 manquant", file=sys.stderr)
        return 1

    body = text.split("\n", 1)[1]
    parser = Parser(tokenize(body))
    root = parser.parse_dict()
    if parser.peek() is not None:
        raise ParseError("Contenu après la fin du dictionnaire racine")

    problems: list[str] = []
    objects = root.get("objects")
    if not isinstance(objects, dict):
        print("✗ Section objects absente", file=sys.stderr)
        return 1

    for key in ("archiveVersion", "objectVersion", "rootObject"):
        if key not in root:
            problems.append(f"clé racine manquante : {key}")

    for object_id, obj in objects.items():
        if not OBJECT_ID.match(object_id):
            problems.append(f"identifiant mal formé : {object_id}")
        if not isinstance(obj, dict) or "isa" not in obj:
            problems.append(f"objet sans isa : {object_id}")

    root_object = root.get("rootObject")
    if root_object not in objects:
        problems.append(f"rootObject introuvable : {root_object}")

    referenced: set[str] = set()
    collect_references(objects, referenced)
    collect_references(root_object, referenced)

    for reference in sorted(referenced):
        if reference not in objects:
            problems.append(f"référence pendante : {reference}")

    defined = set(objects)
    orphans = defined - referenced - {root_object}
    for orphan in sorted(orphans):
        problems.append(f"objet jamais référencé : {orphan} ({objects[orphan].get('isa')})")

    # Every file in a build phase must also be reachable from the navigator.
    grouped_files: set[str] = set()
    for obj in objects.values():
        if isinstance(obj, dict) and obj.get("isa") == "PBXGroup":
            grouped_files.update(child for child in obj.get("children", []))

    for object_id, obj in objects.items():
        if not isinstance(obj, dict) or obj.get("isa") != "PBXBuildFile":
            continue
        file_ref = obj.get("fileRef")
        if file_ref not in grouped_files:
            problems.append(f"fichier compilé hors navigateur : {file_ref}")

    by_isa: dict[str, int] = {}
    for obj in objects.values():
        by_isa[obj.get("isa", "?")] = by_isa.get(obj.get("isa", "?"), 0) + 1

    if problems:
        print("✗ project.pbxproj invalide :", file=sys.stderr)
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        return 1

    print(f"✓ project.pbxproj valide — {len(objects)} objets")
    for isa in sorted(by_isa):
        print(f"    {isa}: {by_isa[isa]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
