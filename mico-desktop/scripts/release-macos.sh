#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <version>" >&2
  exit 1
fi

VERSION="$1"
ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
OUTPUT_DIR="${ROOT_DIR}/dist/release/v${VERSION}"
APP_BUNDLE="${OUTPUT_DIR}/mico.app"
VERSIONED_DMG="${OUTPUT_DIR}/mico-${VERSION}-arm64.dmg"
PUBLISHED_DMG="${OUTPUT_DIR}/mico-desktop-arm64.dmg"
PUBLISHED_SHA="${PUBLISHED_DMG}.sha256"
TAG="v${VERSION}"
NOTARY_PROFILE="${MICO_NOTARY_PROFILE:-mico-notary}"
RELEASE_REPO="${MICO_RELEASE_REPO:-thegeorgejoseph/mico}"

detect_codesign_identity() {
  security find-identity -v -p codesigning 2>/dev/null | sed -n 's/.*"\(Developer ID Application:.*\)"/\1/p' | head -n 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

require_command codesign
require_command xcrun
require_command gh
require_command shasum

CODESIGN_IDENTITY="${MICO_CODESIGN_IDENTITY:-$(detect_codesign_identity)}"

if [ -z "${CODESIGN_IDENTITY}" ]; then
  cat >&2 <<EOF
missing Developer ID Application identity.

Install a Developer ID Application certificate into Keychain Access, then rerun:
  security find-identity -v -p codesigning

You can also override the identity explicitly:
  MICO_CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" make release-macos VERSION=${VERSION}
EOF
  exit 1
fi

echo "==> running verification"
make test
make prod-local-build

echo "==> staging release inputs"
make release-stage VERSION="${VERSION}"

echo "==> signing ${APP_BUNDLE}"
"${ROOT_DIR}/scripts/sign-macos-app.sh" "${APP_BUNDLE}" "${CODESIGN_IDENTITY}"

echo "==> packaging DMG"
"${ROOT_DIR}/scripts/create-macos-dmg.sh" "${APP_BUNDLE}" "${VERSIONED_DMG}"
cp "${VERSIONED_DMG}" "${PUBLISHED_DMG}"

echo "==> notarizing ${PUBLISHED_DMG}"
xcrun notarytool submit "${PUBLISHED_DMG}" --keychain-profile "${NOTARY_PROFILE}" --wait
xcrun stapler staple "${PUBLISHED_DMG}"
xcrun stapler validate "${PUBLISHED_DMG}"

"${ROOT_DIR}/scripts/write-sha256.sh" "${PUBLISHED_DMG}"

echo "==> publishing GitHub release assets"
if gh release view "${TAG}" --repo "${RELEASE_REPO}" >/dev/null 2>&1; then
  gh release upload "${TAG}" "${PUBLISHED_DMG}" "${PUBLISHED_SHA}" --repo "${RELEASE_REPO}" --clobber
else
  gh release create "${TAG}" "${PUBLISHED_DMG}" "${PUBLISHED_SHA}" --repo "${RELEASE_REPO}" --target "$(git -C "${ROOT_DIR}" rev-parse HEAD)" --title "${TAG}"
fi

echo "published ${PUBLISHED_DMG}"
echo "homebrew URL: https://github.com/${RELEASE_REPO}/releases/latest/download/$(basename "${PUBLISHED_DMG}")"
