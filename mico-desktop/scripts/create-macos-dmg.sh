#!/bin/sh
set -eu

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <app_bundle> <dmg_path>" >&2
  exit 1
fi

APP_BUNDLE="$1"
DMG_PATH="$2"
APP_NAME="$(basename "${APP_BUNDLE}")"
VOLNAME="${MICO_DMG_VOLUME_NAME:-mico}"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "${TMP_DIR}"
}

trap cleanup EXIT INT TERM

if [ ! -d "${APP_BUNDLE}" ]; then
  echo "missing app bundle at ${APP_BUNDLE}" >&2
  exit 1
fi

mkdir -p "$(dirname "${DMG_PATH}")"
ditto "${APP_BUNDLE}" "${TMP_DIR}/${APP_NAME}"
ln -s /Applications "${TMP_DIR}/Applications"
rm -f "${DMG_PATH}"

hdiutil create \
  -volname "${VOLNAME}" \
  -srcfolder "${TMP_DIR}" \
  -ov \
  -format UDZO \
  "${DMG_PATH}" >/dev/null

echo "created ${DMG_PATH}"
