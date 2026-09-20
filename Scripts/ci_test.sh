#!/usr/bin/env bash
#
# Builds and tests Plum on whatever iPhone simulator the runner happens to
# have. Picking the device at run time rather than hard-coding "iPhone 16"
# means a new Xcode image does not silently break the build.
#
# `-derivedDataPath` n'est pas de la propreté : sans lui, le paquet construit
# atterrit dans un dossier au nom haché, que rien ne peut retrouver. C'est ce
# qui permet à l'étape suivante de la CI d'ouvrir le `Info.plist` réel plutôt
# que de croire les réglages sur parole.

set -euo pipefail

SCHEME="${SCHEME:-Plum}"
PROJECT="${PROJECT:-Plum.xcodeproj}"

echo "▸ Xcode : $(xcodebuild -version | tr '\n' ' ')"

DEVICE_ID="$(
  xcrun simctl list devices available --json | python3 -c '
import json, sys

catalogue = json.load(sys.stdin)["devices"]

def rank(runtime: str) -> tuple:
    # Prefer the newest iOS runtime available.
    digits = [int(part) for part in runtime.split(".")[-1].split("-")[1:] if part.isdigit()]
    return tuple(digits)

best = None
for runtime, devices in catalogue.items():
    if "iOS" not in runtime:
        continue
    for device in devices:
        if not device.get("isAvailable"):
            continue
        if "iPhone" not in device.get("name", ""):
            continue
        candidate = (rank(runtime), device["name"], device["udid"])
        if best is None or candidate[0] > best[0]:
            best = candidate

if best is None:
    sys.exit("Aucun simulateur iPhone disponible sur ce runner.")

print(best[2])
'
)"

echo "▸ Simulateur : ${DEVICE_ID}"

set -o pipefail
xcodebuild test \
  -project "${PROJECT}" \
  -scheme "${SCHEME}" \
  -destination "id=${DEVICE_ID}" \
  -resultBundlePath "${RESULT_BUNDLE:-build/Plum.xcresult}" \
  -derivedDataPath "${DERIVED_DATA:-build/DerivedData}" \
  CODE_SIGNING_ALLOWED=NO \
  CODE_SIGNING_REQUIRED=NO \
  CODE_SIGN_IDENTITY="" \
  | tee build.log
