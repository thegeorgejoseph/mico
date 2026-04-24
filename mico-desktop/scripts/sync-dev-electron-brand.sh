#!/bin/sh
set -eu

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
APP_DIR="${ROOT_DIR}/app"
SOURCE_APP="${APP_DIR}/node_modules/electron/dist/Electron.app"
DEV_APP_DIR="${ROOT_DIR}/dist/dev-electron"
SOURCE_ICON="${APP_DIR}/assets/mico-shell-icon.icns"
ICON_HASH="$(shasum -a 256 "${SOURCE_ICON}" | awk '{print substr($1, 1, 12)}')"
TARGET_APP="${DEV_APP_DIR}/mico-${ICON_HASH}.app"
INFO_PLIST="${TARGET_APP}/Contents/Info.plist"
TARGET_ICON="${TARGET_APP}/Contents/Resources/electron.icns"
SOURCE_EXECUTABLE="${TARGET_APP}/Contents/MacOS/Electron"
TARGET_EXECUTABLE="${TARGET_APP}/Contents/MacOS/mico"

if [ ! -d "${SOURCE_APP}" ]; then
  echo "missing Electron.app at ${SOURCE_APP}. Run make install first." >&2
  exit 1
fi

if [ ! -f "${SOURCE_ICON}" ]; then
  echo "missing icon source at ${SOURCE_ICON}" >&2
  exit 1
fi

mkdir -p "${DEV_APP_DIR}"
rm -rf "${DEV_APP_DIR}"/mico*.app
ditto "${SOURCE_APP}" "${TARGET_APP}"

cp "${SOURCE_ICON}" "${TARGET_ICON}"
if [ -f "${SOURCE_EXECUTABLE}" ]; then
  mv "${SOURCE_EXECUTABLE}" "${TARGET_EXECUTABLE}"
fi

plutil -replace CFBundleDisplayName -string "mico" "${INFO_PLIST}"
plutil -replace CFBundleName -string "mico" "${INFO_PLIST}"
plutil -replace CFBundleExecutable -string "mico" "${INFO_PLIST}"
plutil -replace CFBundleIdentifier -string "com.thegeorgejoseph.mico.dev.${ICON_HASH}" "${INFO_PLIST}"
plutil -replace CFBundleVersion -string "${ICON_HASH}" "${INFO_PLIST}"

echo "${TARGET_APP}"
