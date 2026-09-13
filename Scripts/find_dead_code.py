#!/usr/bin/env python3
"""List Swift symbols that nothing outside their own declaration references.

A heuristic, deliberately not wired into CI, with two known blind spots worth
stating plainly:

- It cannot tell a protocol witness (`makeBody`, `placeSubviews`,
  `defaultValue`…) from genuinely unused code. The known ones are filtered
  out; anything else you read with judgement.
- It matches on names, not types. When two unrelated types declare the same
  member, each makes the other look used — `MatchesViewModel.apply(_:)` sat
  unwired for a while precisely because `ProfileViewModel` had an `apply` of
  its own. Common member names are exactly where it is blindest, so a clean
  run is not proof.

A check that cries wolf is a check people learn to ignore, which is why it
stays out of CI; a check that claims more than it can see is worse.

Run it after a feature lands — code that was written and never called is the
most common kind of rot in this repository, and every pass so far has found
some.

    python3 Scripts/find_dead_code.py
"""

from __future__ import annotations

import collections
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE_DIRS = ["Plum"]
ALSO_SEARCH = ["PlumTests", "PlumUITests", "Scripts"]

# Requirements the frameworks call for us; their absence from our own code is
# expected, not a finding.
PROTOCOL_WITNESSES = {
    "makeBody",       # ButtonStyle
    "defaultValue",   # EnvironmentKey
    "placeSubviews",  # Layout
    "sizeThatFits",   # Layout
    "path",           # Shape
    "body",           # View / App
    "encode",         # Encodable
    "main",           # App entry point
}

DECLARATIONS = [
    (re.compile(r"^\s*case (\w+)$"), "cas d'énumération"),
    (re.compile(r"^\s*static let (\w+)\s*[:=]"), "propriété statique"),
    (re.compile(r"^\s*(?:private |fileprivate )?func (\w+)\("), "méthode"),
    (re.compile(r"^\s*(?:private\(set\) )?var (\w+):\s*\w"), "propriété"),
    (re.compile(r"^\s*struct (\w+)\b"), "structure"),
    (re.compile(r"^\s*enum (\w+)\b"), "énumération"),
]

MINIMUM_NAME_LENGTH = 4


def DECLARATION_COUNTERS(name: str) -> list[str]:
    """Every way this name could be introduced rather than used."""
    escaped = re.escape(name)
    return [
        r"^\s*(?:private |fileprivate |public )?func " + escaped + r"\b",
        r"^\s*static let " + escaped + r"\b",
        r"^\s*(?:private\(set\) )?var " + escaped + r"\b",
        r"^\s*case " + escaped + r"$",
        r"^\s*struct " + escaped + r"\b",
        r"^\s*enum " + escaped + r"\b",
    ]


def main() -> int:
    source_files = [
        path
        for directory in SOURCE_DIRS
        for path in (ROOT / directory).rglob("*.swift")
    ]
    searched = source_files + [
        path
        for directory in ALSO_SEARCH
        for suffix in ("*.swift", "*.py")
        for path in (ROOT / directory).rglob(suffix)
    ]
    haystack = "\n".join(path.read_text(encoding="utf-8") for path in searched)

    declarations: dict[str, list[tuple[str, str]]] = collections.defaultdict(list)
    for path in source_files:
        relative = str(path.relative_to(ROOT))
        for line in path.read_text(encoding="utf-8").splitlines():
            for pattern, kind in DECLARATIONS:
                match = pattern.match(line)
                if match:
                    declarations[match.group(1)].append((kind, relative))

    findings = []
    for name, places in sorted(declarations.items()):
        if len(name) < MINIMUM_NAME_LENGTH or name in PROTOCOL_WITNESSES:
            continue
        # Count declarations across the whole corpus, not just the ones this
        # index holds: two unrelated types declaring the same member used to
        # make each other look used (`apply` was called from nowhere for a
        # while precisely because of that).
        declarations = sum(
            len(re.findall(pattern, haystack, re.M))
            for pattern in DECLARATION_COUNTERS(name)
        )
        occurrences = len(re.findall(r"\b" + re.escape(name) + r"\b", haystack))
        if occurrences <= max(declarations, len(places)):
            findings.append((places[0][1], places[0][0], name))

    if not findings:
        print("✓ Aucun symbole manifestement inutilisé")
        return 0

    print("Symboles jamais référencés hors de leur déclaration :\n")
    for path, kind, name in findings:
        print(f"  {path:50} {kind:18} {name}")
    print(
        f"\n{len(findings)} à examiner. Ce n'est pas un verdict : une exigence "
        "de protocole non listée dans PROTOCOL_WITNESSES apparaîtra ici."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
